//! Bounded, read-only inspection of explicitly selected directories.
//!
//! This is a diagnostic, not a snapshot or protection against hostile path races.

use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_MAX_ENTRIES: u64 = 100_000;
pub const MAX_ENTRIES: u64 = 10_000_000;
const MAX_RECORDED_ERRORS: usize = 128;

#[derive(Debug, Serialize)]
pub struct FilesystemBytes {
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub available: u64,
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub requested_path: String,
    pub absolute_path: String,
    pub path_encoding_lossy: bool,
    pub canonical_path_at_start: Option<String>,
    pub effective_cargo_path_verified: bool,
    pub ownership_and_concurrency_verified: bool,
    pub filesystem_bytes: Option<FilesystemBytes>,
    pub logical_file_bytes: u64,
    pub regular_files: u64,
    pub entries_seen: u64,
    pub skipped_links_or_reparse_points: u64,
    pub skipped_other_entries: u64,
    pub duplicate_hard_links_skipped: u64,
    pub duplicate_directories_skipped: u64,
    pub errors: Vec<String>,
    pub errors_total: u64,
    pub errors_truncated: bool,
    pub complete_scan: bool,
    pub entry_limit: u64,
    pub entry_limit_reached: bool,
}

impl Inspection {
    fn error(&mut self, error: impl ToString) {
        self.errors_total += 1;
        if self.errors.len() < MAX_RECORDED_ERRORS {
            self.errors.push(error.to_string());
        } else {
            self.errors_truncated = true;
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub mode: &'static str,
    pub cargo_invoked: bool,
    pub version: &'static str,
    pub notes: &'static [&'static str],
    pub paths: Vec<Inspection>,
}

pub fn report(paths: &[PathBuf], max_entries: u64) -> Report {
    Report {
        mode: "read-only",
        cargo_invoked: false,
        version: env!("CARGO_PKG_VERSION"),
        notes: &[
            "Only explicitly supplied paths are inspected. Cargo configuration is not resolved; tilde and environment variables are not expanded by the binary.",
            "Directory sizes are logical regular-file bytes, not allocated or reclaimable space. Free and available filesystem bytes have different meanings.",
            "Hard links are deduplicated within each supplied path. Do not sum overlapping paths.",
            "Symlinks/reparse points, other-device entries, and duplicate directory identities are skipped. Same-device bind mounts cannot be reliably identified.",
            "complete_scan describes completion within the stated skip policy; skipped entries remain outside coverage.",
            "This is not a snapshot or security sandbox. Concurrent path replacement can invalidate results; use stable, trusted directories.",
            "No cleanup eligibility, ownership, or idle-build guarantee is inferred. The helper never invokes Cargo, deletes files, or changes settings.",
        ],
        paths: paths.iter().map(|p| inspect(p, max_entries)).collect(),
    }
}

fn is_link_or_reparse(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
struct Identity {
    device: u64,
    file: u64,
}

#[cfg(unix)]
fn identity(_path: &Path, metadata: &Metadata) -> io::Result<Identity> {
    use std::os::unix::fs::MetadataExt;
    Ok(Identity {
        device: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn identity(path: &Path, _metadata: &Metadata) -> io::Result<Identity> {
    use std::fs::OpenOptions;
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT,
    };
    // Attribute-only handle, with sharing enabled by std. OPEN_REPARSE_POINT
    // prevents following a replacement reparse point during this identity probe.
    let file = OpenOptions::new()
        .access_mode(0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    // SAFETY: the handle is live and info points to writable storage of the
    // exact Win32 structure; initialization is read only after success.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    if info.dwFileAttributes & 0x400 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Entry became a reparse point during identity inspection.",
        ));
    }
    Ok(Identity {
        device: u64::from(info.dwVolumeSerialNumber),
        file: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    })
}

#[cfg(not(any(unix, windows)))]
compile_error!("rust-cache-inspect supports Unix and Windows targets");

fn validated_root(path: &Path) -> io::Result<(PathBuf, Metadata, Identity)> {
    if !path.is_absolute() && matches!(path.components().next(), Some(Component::Prefix(_))) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Drive-relative Windows paths are ambiguous; supply a fully qualified path.",
        ));
    }
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Do not canonicalize before checking every spelled component. In
    // particular, link/../target must not hide a symlink ancestor.
    let mut prefix = PathBuf::new();
    for component in absolute.components() {
        prefix.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&prefix)?;
        if is_link_or_reparse(&metadata) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput,
                "Path or ancestor is a symlink/reparse point; verify its owner and supply the actual path explicitly."));
        }
        if !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expected an existing directory and directory ancestors.",
            ));
        }
    }
    let canonical = fs::canonicalize(&absolute)?;
    let metadata = fs::symlink_metadata(&canonical)?;
    if is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Root changed during inspection.",
        ));
    }
    let identity = identity(&canonical, &metadata)?;
    Ok((canonical, metadata, identity))
}

pub fn inspect(path: &Path, max_entries: u64) -> Inspection {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()
            .map(|p| p.join(path))
            .unwrap_or_else(|_| path.to_owned())
    };
    let mut result = Inspection {
        requested_path: path.to_string_lossy().into_owned(),
        absolute_path: absolute.to_string_lossy().into_owned(),
        path_encoding_lossy: path.to_str().is_none() || absolute.to_str().is_none(),
        canonical_path_at_start: None,
        effective_cargo_path_verified: false,
        ownership_and_concurrency_verified: false,
        filesystem_bytes: None,
        logical_file_bytes: 0,
        regular_files: 0,
        entries_seen: 0,
        skipped_links_or_reparse_points: 0,
        skipped_other_entries: 0,
        duplicate_hard_links_skipped: 0,
        duplicate_directories_skipped: 0,
        errors: Vec::new(),
        errors_total: 0,
        errors_truncated: false,
        complete_scan: false,
        entry_limit: max_entries,
        entry_limit_reached: false,
    };
    if !(1..=MAX_ENTRIES).contains(&max_entries) {
        result.error(format!("max-entries must be between 1 and {MAX_ENTRIES}"));
        return result;
    }
    let (root, _, root_identity) = match validated_root(path) {
        Ok(root) => root,
        Err(error) => {
            result.error(error);
            return result;
        }
    };
    result.canonical_path_at_start = Some(root.to_string_lossy().into_owned());
    match fs2::statvfs(&root) {
        Ok(stats) => {
            result.filesystem_bytes = Some(FilesystemBytes {
                total: stats.total_space(),
                used: stats.total_space().saturating_sub(stats.free_space()),
                free: stats.free_space(),
                available: stats.available_space(),
            })
        }
        Err(error) => result.error(format!("Filesystem capacity: {error}")),
    }
    let mut stack = vec![(root, root_identity)];
    let mut seen_directories = HashSet::from([root_identity]);
    let mut seen_files = HashSet::new();
    'scan: while let Some((directory, expected_identity)) = stack.pop() {
        let metadata = match fs::symlink_metadata(&directory) {
            Ok(info) => info,
            Err(error) => {
                result.error(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        if is_link_or_reparse(&metadata) {
            result.skipped_links_or_reparse_points += 1;
            result.error(format!(
                "{}: queued directory became a link/reparse point",
                directory.display()
            ));
            continue;
        }
        match identity(&directory, &metadata) {
            Ok(current) if current == expected_identity && metadata.is_dir() => {}
            Ok(_) => {
                result.error(format!(
                    "{}: queued directory identity changed",
                    directory.display()
                ));
                continue;
            }
            Err(error) => {
                result.error(format!("{}: {error}", directory.display()));
                continue;
            }
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                result.error(format!("{}: {error}", directory.display()));
                continue;
            }
        };
        for entry in entries {
            if result.entries_seen >= max_entries {
                result.entry_limit_reached = true;
                break 'scan;
            }
            result.entries_seen += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    result.error(format!("{}: {error}", directory.display()));
                    continue;
                }
            };
            let entry_path = entry.path();
            let metadata = match fs::symlink_metadata(&entry_path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    result.error(format!("{}: {error}", entry_path.display()));
                    continue;
                }
            };
            if is_link_or_reparse(&metadata) {
                result.skipped_links_or_reparse_points += 1;
                continue;
            }
            if !metadata.is_dir() && !metadata.is_file() {
                result.skipped_other_entries += 1;
                continue;
            }
            let entry_identity = match identity(&entry_path, &metadata) {
                Ok(identity) => identity,
                Err(error) => {
                    result.error(format!("{}: {error}", entry_path.display()));
                    continue;
                }
            };
            if entry_identity.device != root_identity.device {
                result.skipped_other_entries += 1;
                continue;
            }
            if metadata.is_dir() {
                if seen_directories.insert(entry_identity) {
                    // Stack and identity sets are bounded by max_entries.
                    stack.push((entry_path, entry_identity));
                } else {
                    result.duplicate_directories_skipped += 1;
                }
            } else if entry_identity.file != 0 && !seen_files.insert(entry_identity) {
                result.duplicate_hard_links_skipped += 1;
            } else if let Some(bytes) = result.logical_file_bytes.checked_add(metadata.len()) {
                result.logical_file_bytes = bytes;
                result.regular_files += 1;
            } else {
                result.error("Logical file-byte total overflowed u64; scan stopped.");
                break 'scan;
            }
        }
    }
    result.complete_scan = !result.entry_limit_reached && result.errors_total == 0;
    result
}
