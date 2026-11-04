use rust_cache_inspect::inspect;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rust-cache-inspect-test-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(fs::canonicalize(path).unwrap())
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn counts_nested_files_and_deduplicates_hardlinks_without_mutation() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.path().join("nested")).unwrap();
    let original = fixture.path().join("original");
    fs::write(&original, b"12345").unwrap();
    fs::hard_link(&original, fixture.path().join("alias")).unwrap();
    fs::write(fixture.path().join("nested/second"), b"678").unwrap();
    let before = fs::metadata(&original).unwrap().modified().unwrap();
    let result = inspect(fixture.path(), 100);
    assert!(result.complete_scan, "{:?}", result.errors);
    assert_eq!(result.logical_file_bytes, 8);
    assert_eq!(result.regular_files, 2);
    assert_eq!(result.duplicate_hard_links_skipped, 1);
    assert_eq!(result.entries_seen, 4);
    assert!(result.filesystem_bytes.unwrap().total > 0);
    assert_eq!(fs::read(original).unwrap(), b"12345");
    assert_eq!(fs::read_dir(fixture.path()).unwrap().count(), 3);
    assert_eq!(
        fs::metadata(fixture.path().join("alias"))
            .unwrap()
            .modified()
            .unwrap(),
        before
    );
}

#[test]
fn reports_missing_and_regular_file_roots_as_errors() {
    let fixture = Fixture::new();
    fs::write(fixture.path().join("file"), b"data").unwrap();
    for path in [fixture.path().join("missing"), fixture.path().join("file")] {
        let result = inspect(&path, 100);
        assert!(!result.complete_scan);
        assert!(result.errors_total > 0);
        assert_eq!(result.entries_seen, 0);
    }
}

#[test]
fn bounded_scan_is_explicitly_partial() {
    let fixture = Fixture::new();
    for n in 0..20 {
        fs::create_dir(fixture.path().join(format!("directory-{n}"))).unwrap();
    }
    let result = inspect(fixture.path(), 3);
    assert!(!result.complete_scan);
    assert!(result.entry_limit_reached);
    assert_eq!(result.entries_seen, 3);
    assert_eq!(result.errors_total, 0);
}

#[test]
fn exact_budget_can_complete_and_empty_directory_is_complete() {
    let fixture = Fixture::new();
    assert!(inspect(fixture.path(), 1).complete_scan);
    fs::write(fixture.path().join("one"), b"1").unwrap();
    assert!(inspect(fixture.path(), 1).complete_scan);
}

#[test]
fn cli_reports_multiple_paths_and_failure_exit_status() {
    let fixture = Fixture::new();
    fs::write(fixture.path().join("file"), b"123").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rust-cache-inspect"))
        .arg("--path")
        .arg(fixture.path())
        .arg("--path")
        .arg(fixture.path().join("missing"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["mode"], "read-only");
    assert_eq!(json["cargo_invoked"], false);
    assert_eq!(json["paths"][0]["logical_file_bytes"], 3);
    assert_eq!(json["paths"][0]["complete_scan"], true);
    assert_eq!(json["paths"][1]["complete_scan"], false);
}

#[test]
fn cli_rejects_invalid_or_missing_arguments() {
    for args in [
        vec![],
        vec!["--path"],
        vec!["--path", ""],
        vec!["--path", ".", "--max-entries", "0"],
        vec!["--path", ".", "--max-entries", "10000001"],
        vec!["--path", ".", "--max-entries", "1", "--max-entries", "2"],
        vec!["--delete"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rust-cache-inspect"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let version = Command::new(env!("CARGO_BIN_EXE_rust-cache-inspect"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        "rust-cache-inspect 0.2.0"
    );
}

#[test]
fn cli_limit_is_a_non_successful_partial_result() {
    let fixture = Fixture::new();
    fs::write(fixture.path().join("one"), b"1").unwrap();
    fs::write(fixture.path().join("two"), b"2").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rust-cache-inspect"))
        .arg("--path")
        .arg(fixture.path())
        .args(["--max-entries", "1"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["paths"][0]["entry_limit_reached"], true);
    assert_eq!(json["paths"][0]["entries_seen"], 1);
}

#[cfg(unix)]
#[test]
fn rejects_link_roots_ancestors_and_link_parent_traversal() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.path().join("real/child")).unwrap();
    symlink(fixture.path().join("real"), fixture.path().join("link")).unwrap();
    for path in [
        fixture.path().join("link"),
        fixture.path().join("link/child"),
        fixture.path().join("link/../real/child"),
    ] {
        let result = inspect(&path, 100);
        assert!(!result.complete_scan);
        assert!(
            result.errors.iter().any(|e| e.contains("symlink/reparse")),
            "{:?}",
            result.errors
        );
        assert_eq!(result.entries_seen, 0);
    }
    assert!(inspect(&fixture.path().join("real/../real/child"), 100).complete_scan);
}

#[cfg(unix)]
#[test]
fn skips_descendant_links_and_special_files() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.path().join("secret"), b"not counted").unwrap();
    fs::write(fixture.path().join("keep"), b"123").unwrap();
    symlink(outside.path(), fixture.path().join("linked-dir")).unwrap();
    symlink(
        outside.path().join("secret"),
        fixture.path().join("linked-file"),
    )
    .unwrap();
    symlink("missing", fixture.path().join("broken")).unwrap();
    assert!(Command::new("mkfifo")
        .arg(fixture.path().join("fifo"))
        .status()
        .unwrap()
        .success());
    let result = inspect(fixture.path(), 100);
    assert!(result.complete_scan, "{:?}", result.errors);
    assert_eq!(result.logical_file_bytes, 3);
    assert_eq!(result.skipped_links_or_reparse_points, 3);
    assert_eq!(result.skipped_other_entries, 1);
}

#[cfg(unix)]
#[test]
fn inaccessible_directory_reports_partial_scan() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let denied = fixture.path().join("denied");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("file"), b"data").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
    let result = inspect(fixture.path(), 100);
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
    // Root can bypass Unix mode bits. Fail explicitly there instead of
    // presenting an unexecuted permission test as evidence.
    assert!(
        !result.complete_scan,
        "Run permission-denial coverage as a non-root user"
    );
    assert!(result.errors_total > 0);
    assert_eq!(result.logical_file_bytes, 0);
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_report_filesystem_result_and_lossy_display() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    let path = fixture.path().join(OsString::from_vec(vec![b'd', 0xff]));
    match fs::create_dir(&path) {
        Ok(()) => {
            fs::write(path.join("file"), b"data").unwrap();
            let result = inspect(&path, 100);
            assert!(result.complete_scan, "{:?}", result.errors);
            assert!(result.path_encoding_lossy);
            assert_eq!(result.logical_file_bytes, 4);
        }
        // Darwin EILSEQ is 92. Native macOS CI rejects this filename at
        // creation; validate the helper's filesystem error instead of skipping
        // coverage or assuming every Unix filesystem accepts arbitrary bytes.
        #[cfg(target_os = "macos")]
        Err(error) if error.raw_os_error() == Some(92) => {
            let expected_error = fs::symlink_metadata(&path).unwrap_err().to_string();
            let result = inspect(&path, 100);
            assert!(!result.complete_scan);
            assert!(result.path_encoding_lossy);
            assert_eq!(result.errors_total, 1);
            assert_eq!(result.errors, vec![expected_error]);
            assert_eq!(result.entries_seen, 0);
            assert_eq!(result.logical_file_bytes, 0);
            assert!(result.canonical_path_at_start.is_none());
        }
        Err(error) => panic!("Unexpected non-UTF-8 fixture creation error: {error}"),
    }
}

#[cfg(windows)]
#[test]
fn rejects_junction_roots_and_skips_junction_descendants() {
    let fixture = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.path().join("uncounted"), b"outside").unwrap();
    let junction = fixture.path().join("junction");
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let root = inspect(&junction, 100);
    assert!(!root.complete_scan);
    assert!(root.errors.iter().any(|e| e.contains("symlink/reparse")));
    let parent = inspect(fixture.path(), 100);
    assert!(parent.complete_scan, "{:?}", parent.errors);
    assert_eq!(parent.skipped_links_or_reparse_points, 1);
    assert_eq!(parent.logical_file_bytes, 0);
    fs::remove_dir(junction).unwrap();
}

#[cfg(windows)]
#[test]
fn rejects_ambiguous_drive_relative_paths() {
    let result = inspect(Path::new("C:relative"), 100);
    assert!(!result.complete_scan);
    assert!(result.errors.iter().any(|e| e.contains("Drive-relative")));
}
