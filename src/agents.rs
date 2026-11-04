//! Explicit, create/reuse-only skill registration. Never executes provider CLIs.
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const NAME: &str = "rust-cache-management";
const IDS: [&str; 5] = ["codex", "claude", "gemini", "opencode", "hermes"];
const TEXT: [&str; 12] = [
    "AGENTS-snippet.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.md",
    "SKILL.md",
    "THIRD-PARTY-NOTICES",
    "VERSION",
    "references/defaults.md",
    "references/distribution.md",
    "references/inspection.md",
    "references/policy.md",
];
const MAX_SCAN: usize = 8192;
const MAX_BYTES: u64 = 4 * 1024 * 1024;
#[cfg(windows)]
const BINARY: &str = "bin/rust-cache-inspect.exe";
#[cfg(not(windows))]
const BINARY: &str = "bin/rust-cache-inspect";

#[derive(Serialize)]
struct Agent {
    id: String,
    detected: bool,
    version: Option<String>,
    compatibility: &'static str,
    evidence: Vec<String>,
    warnings: Vec<String>,
}
#[derive(Clone, Serialize)]
struct Action {
    agents: Vec<String>,
    target: PathBuf,
    status: String,
    detail: String,
}
#[derive(Serialize)]
struct Response {
    schema_version: u32,
    command: String,
    scope: String,
    dry_run: bool,
    status: String,
    agents: Vec<Agent>,
    actions: Vec<Action>,
    errors: Vec<String>,
    warnings: Vec<String>,
}
#[derive(Default)]
struct Options {
    command: String,
    scope: String,
    project: Option<PathBuf>,
    selected: BTreeSet<String>,
    all: bool,
    dry: bool,
    yes: bool,
}
struct Context {
    home: PathBuf,
    exe: PathBuf,
    env: BTreeMap<String, OsString>,
}
#[derive(Clone)]
struct Root {
    path: PathBuf,
    scope: &'static str,
    agents: BTreeSet<String>,
}
struct Existing {
    path: PathBuf,
    scope: &'static str,
    agents: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
struct Record {
    schema_version: u32,
    target: PathBuf,
    version: String,
    source_manifest_sha256: String,
    files: BTreeMap<String, String>,
}
struct Payload {
    bytes: BTreeMap<String, Vec<u8>>,
    hashes: BTreeMap<String, String>,
    manifest: String,
}

fn linked(info: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        info.file_type().is_symlink() || info.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        info.file_type().is_symlink()
    }
}
fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e),
    }
}
fn safe(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.to_str().is_none()
        || path.components().any(|c| matches!(c, Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Expected an absolute, unlinked Unicode path without parent traversal",
        ));
    }
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        match fs::symlink_metadata(&prefix) {
            Ok(info) if linked(&info) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Linked/reparse path preserved: {}", prefix.display()),
                ))
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    safe(path)?;
    let info = fs::symlink_metadata(path)?;
    if !info.is_file() || info.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Missing, non-regular, or oversized file",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File exceeded read budget",
        ));
    }
    Ok(bytes)
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn mkdir(path: &Path) -> io::Result<()> {
    safe(path)?;
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) {
            continue;
        }
        if !exists(&prefix)? {
            match fs::create_dir(&prefix) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        safe(&prefix)?;
        if !fs::symlink_metadata(&prefix)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Parent is not a directory",
            ));
        }
    }
    Ok(())
}
fn exclusive_file(path: &Path) -> io::Result<File> {
    safe(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}
fn publish(source: &Path, destination: &Path) -> io::Result<()> {
    safe(source)?;
    safe(destination)?;
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(source.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
        // SAFETY: live NUL-terminated paths; atomic no-replace kernel operation.
        if unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(source.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
        // SAFETY: live NUL-terminated paths; Darwin exclusive rename refuses collisions.
        if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileW;
        let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // SAFETY: valid terminated UTF-16 buffers; MoveFileW does not replace a destination.
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    return Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Atomic no-replace setup is unsupported on this OS",
    ));
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    Ok(())
}
fn skill_name(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Non-UTF-8 skill metadata")?;
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return Err("Missing skill frontmatter".into());
    }
    let mut name = None;
    for line in lines {
        if line == "---" {
            return name.ok_or_else(|| "Missing skill name".into());
        }
        if let Some(value) = line.strip_prefix("name:") {
            if name.is_some() {
                return Err("Duplicate skill name field".into());
            }
            let value = value.trim();
            let scalar = if value.len() >= 2
                && ((value.starts_with('"') && value.ends_with('"'))
                    || (value.starts_with('\'') && value.ends_with('\'')))
            {
                &value[1..value.len() - 1]
            } else {
                value
            };
            if scalar.is_empty()
                || !scalar
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err("Ambiguous skill name metadata".into());
            }
            name = Some(scalar.to_string());
        }
    }
    Err("Unclosed skill frontmatter".into())
}
fn discovery_frontmatter(path: &Path) -> io::Result<Vec<u8>> {
    const MAX_HEADER: u64 = 65536;
    safe(path)?;
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Non-regular skill metadata file",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let file = options.open(path)?;
    let mut reader = BufReader::new(file.take(MAX_HEADER + 1));
    let mut bytes = Vec::new();
    loop {
        let start = bytes.len();
        if reader.read_until(b'\n', &mut bytes)? == 0 {
            return Ok(bytes);
        }
        if bytes.len() as u64 > MAX_HEADER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Skill frontmatter exceeds 65536-byte discovery budget",
            ));
        }
        let line = &bytes[start..];
        let line = line.strip_suffix(b"\n").unwrap_or(line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if (start == 0 && line != b"---") || (start != 0 && line == b"---") {
            return Ok(bytes);
        }
    }
}
fn source(exe: &Path) -> io::Result<Payload> {
    safe(exe)?;
    let root = exe.parent().and_then(Path::parent).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Executable is not in a release payload",
        )
    })?;
    let manifest = bounded(&root.join("MANIFEST.sha256"), 65536)?;
    let mut expected: BTreeSet<String> = TEXT.iter().map(|v| v.to_string()).collect();
    expected.extend([
        BINARY.to_string(),
        "install.sh".into(),
        "install.ps1".into(),
    ]);
    let mut hashes = BTreeMap::new();
    for line in std::str::from_utf8(&manifest)
        .map_err(io::Error::other)?
        .lines()
    {
        let (hash, name) = line
            .split_once("  ")
            .ok_or_else(|| io::Error::other("Invalid release manifest"))?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !expected.contains(name)
            || hashes.insert(name.to_string(), hash.to_string()).is_some()
        {
            return Err(io::Error::other(
                "Unexpected or duplicate release manifest member",
            ));
        }
    }
    if hashes.keys().cloned().collect::<BTreeSet<_>>() != expected {
        return Err(io::Error::other("Incomplete fixed release manifest"));
    }
    let mut stack = vec![root.to_path_buf()];
    let mut actual = BTreeSet::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            safe(&path)?;
            let info = fs::symlink_metadata(&path)?;
            let rel = path
                .strip_prefix(root)
                .map_err(io::Error::other)?
                .to_str()
                .ok_or_else(|| io::Error::other("Invalid payload path"))?
                .replace('\\', "/");
            if info.is_dir() && (rel == "bin" || rel == "references") {
                stack.push(path);
            } else if info.is_file() && (expected.contains(&rel) || rel == "MANIFEST.sha256") {
                actual.insert(rel);
            } else {
                return Err(io::Error::other("Unexpected release payload member"));
            }
            if actual.len() > 16 {
                return Err(io::Error::other("Release member limit exceeded"));
            }
        }
    }
    let mut all = expected.clone();
    all.insert("MANIFEST.sha256".into());
    if actual != all {
        return Err(io::Error::other("Incomplete release tree"));
    }
    let mut bytes = BTreeMap::new();
    for (name, hash) in &hashes {
        let data = bounded(
            &root.join(name),
            if name == BINARY {
                64 * 1024 * 1024
            } else {
                MAX_BYTES
            },
        )?;
        if digest(&data) != *hash {
            return Err(io::Error::other(format!(
                "Modified source payload preserved: {name}"
            )));
        }
        if TEXT.contains(&name.as_str()) {
            bytes.insert(name.clone(), data);
        }
    }
    if bytes["VERSION"] != format!("v{}\n", env!("CARGO_PKG_VERSION")).as_bytes() {
        return Err(io::Error::other(
            "Release VERSION does not match this binary",
        ));
    }
    if skill_name(&bytes["SKILL.md"]).map_err(io::Error::other)? != NAME {
        return Err(io::Error::other("Unexpected source skill name"));
    }
    hashes.retain(|name, _| TEXT.contains(&name.as_str()));
    Ok(Payload {
        bytes,
        hashes,
        manifest: digest(&manifest),
    })
}
fn context() -> io::Result<Context> {
    let mut env = BTreeMap::new();
    for name in [
        "HOME",
        "USERPROFILE",
        "PATH",
        "CODEX_HOME",
        "GEMINI_CLI_HOME",
        "CLAUDE_CONFIG_DIR",
        "HERMES_HOME",
        "XDG_CONFIG_HOME",
        "OPENCODE_CONFIG_DIR",
        "OPENCODE_CONFIG",
        "OPENCODE_CONFIG_CONTENT",
        "OPENCODE_DISABLE_EXTERNAL_SKILLS",
        "OPENCODE_DISABLE_CLAUDE_CODE_SKILLS",
    ] {
        if let Some(value) = std::env::var_os(name).filter(|v| !v.is_empty()) {
            env.insert(name.into(), value);
        }
    }
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    let home = PathBuf::from(
        env.get(key)
            .ok_or_else(|| io::Error::other("OS user home is unavailable"))?,
    );
    safe(&home)?;
    Ok(Context {
        home,
        exe: std::env::current_exe()?,
        env,
    })
}
fn env_path(ctx: &Context, key: &str, default: PathBuf) -> PathBuf {
    ctx.env.get(key).map(PathBuf::from).unwrap_or(default)
}
fn add_root(roots: &mut Vec<Root>, path: PathBuf, scope: &'static str, ids: &[&str]) {
    if let Some(root) = roots.iter_mut().find(|r| r.path == path) {
        root.agents.extend(ids.iter().map(|v| v.to_string()));
    } else {
        roots.push(Root {
            path,
            scope,
            agents: ids.iter().map(|v| v.to_string()).collect(),
        });
    }
}
fn roots(ctx: &Context, project: Option<&Path>) -> Vec<Root> {
    let mut roots = Vec::new();
    let g = env_path(ctx, "GEMINI_CLI_HOME", ctx.home.clone());
    let c = env_path(ctx, "CLAUDE_CONFIG_DIR", ctx.home.join(".claude"));
    let h = env_path(ctx, "HERMES_HOME", ctx.home.join(".hermes"));
    add_root(
        &mut roots,
        ctx.home.join(".agents/skills"),
        "user",
        &["codex", "opencode"],
    );
    add_root(&mut roots, g.join(".agents/skills"), "user", &["gemini"]);
    add_root(&mut roots, g.join(".gemini/skills"), "user", &["gemini"]);
    add_root(
        &mut roots,
        env_path(ctx, "CODEX_HOME", ctx.home.join(".codex")).join("skills"),
        "user",
        &["codex"],
    );
    add_root(&mut roots, c.join("skills"), "user", &["claude"]);
    add_root(
        &mut roots,
        ctx.home.join(".claude/skills"),
        "user",
        &["opencode"],
    );
    add_root(
        &mut roots,
        ctx.home.join(".config/opencode/skills"),
        "user",
        &["opencode"],
    );
    add_root(&mut roots, h.join("skills"), "user", &["hermes"]);
    if let Some(project) = project {
        for ancestor in project.ancestors().take(32) {
            let scope = if ancestor == project {
                "project"
            } else {
                "ancestor"
            };
            add_root(
                &mut roots,
                ancestor.join(".agents/skills"),
                scope,
                &["codex", "gemini", "opencode"],
            );
            add_root(
                &mut roots,
                ancestor.join(".claude/skills"),
                scope,
                &["claude", "opencode"],
            );
            add_root(
                &mut roots,
                ancestor.join(".gemini/skills"),
                scope,
                &["gemini"],
            );
            add_root(
                &mut roots,
                ancestor.join(".opencode/skills"),
                scope,
                &["opencode"],
            );
            if ancestor.join(".git").exists() {
                add_root(
                    &mut roots,
                    ancestor.join(".hermes/skills"),
                    scope,
                    &["hermes"],
                );
                break;
            }
        }
    }
    roots
}
fn command_present(ctx: &Context, id: &str) -> Option<PathBuf> {
    let path = ctx.env.get("PATH")?;
    for dir in std::env::split_paths(path).take(128) {
        #[cfg(windows)]
        let names = vec![
            format!("{id}.exe"),
            format!("{id}.cmd"),
            format!("{id}.bat"),
        ];
        #[cfg(not(windows))]
        let names = vec![id.to_string()];
        for name in names {
            let p = dir.join(name);
            if let Ok(info) = fs::metadata(&p) {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if info.is_file() && info.permissions().mode() & 0o111 != 0 {
                        return Some(p);
                    }
                }
                #[cfg(windows)]
                if info.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}
fn detect(ctx: &Context, _roots: &[Root]) -> Vec<Agent> {
    IDS.iter().map(|id| {
        let mut evidence = Vec::new();
        if let Some(path) = command_present(ctx, id) { evidence.push(format!("PATH command present: {}", path.display())); }
        let config = match *id {
            "codex" => env_path(ctx, "CODEX_HOME", ctx.home.join(".codex")),
            "claude" => env_path(ctx, "CLAUDE_CONFIG_DIR", ctx.home.join(".claude")),
            "gemini" => env_path(ctx, "GEMINI_CLI_HOME", ctx.home.clone()).join(".gemini"),
            "opencode" => ctx.home.join(".config/opencode"),
            _ => env_path(ctx, "HERMES_HOME", ctx.home.join(".hermes")),
        };
        if config.is_dir() { evidence.push(format!("Configuration directory present: {}", config.display())); }
        Agent { id: id.to_string(), detected: !evidence.is_empty(), version: None, compatibility: "unverified", evidence,
            warnings: vec![match *id {
                "gemini" => "Shared .agents support documented from 0.28.0; catalog tested at 0.66.0 only. Installed version is unknown; --yes accepts this documented-path assumption.",
                "hermes" => "NousResearch identity/version and profile sources are not certified by a matching command name. Project trust is separate.",
                _ => "PATH/config presence is installation evidence only; installed version, current usage, catalog activation, and permissions remain unverified.",
            }.into()] }
    }).collect()
}
fn custom_problem(ctx: &Context, opt: &Options, id: &str) -> io::Result<Option<String>> {
    if id == "opencode" && ctx.env.keys().any(|v| v.starts_with("OPENCODE_")) {
        return Ok(Some(
            "Custom OpenCode environment/source configuration requires manual review".into(),
        ));
    }
    if id == "opencode" {
        let default = ctx.home.join(".config");
        if env_path(ctx, "XDG_CONFIG_HOME", default.clone()) != default {
            return Ok(Some(
                "Custom XDG_CONFIG_HOME leaves OpenCode sources unverified; manual review required"
                    .into(),
            ));
        }
        let mut configs = vec![
            default.join("opencode/opencode.json"),
            default.join("opencode/opencode.jsonc"),
        ];
        if let Some(project) = &opt.project {
            for ancestor in project.ancestors().take(32) {
                for name in ["opencode.json", "opencode.jsonc"] {
                    configs.push(ancestor.join(name));
                    configs.push(ancestor.join(".opencode").join(name));
                }
                if ancestor.join(".git").exists() {
                    break;
                }
            }
        }
        for config in configs {
            if exists(&config)? {
                return Ok(Some(format!(
                    "OpenCode configuration may add custom skill sources; manual review required: {}",
                    config.display()
                )));
            }
        }
    }
    if id == "hermes" {
        let h = env_path(ctx, "HERMES_HOME", ctx.home.join(".hermes"));
        safe(&h)?;
        if !ctx.env.contains_key("HERMES_HOME")
            && (h.join("profiles").exists()
                || h.join("active_profile").exists()
                || h.join(".active_profile").exists())
        {
            return Ok(Some(
                "Hermes profile selection is uncertain; explicitly select HERMES_HOME".into(),
            ));
        }
        let config = h.join("config.yaml");
        if exists(&config)? {
            let data = bounded(&config, 65536)?;
            let text = String::from_utf8(data).map_err(io::Error::other)?;
            if ["external_dirs", "create_dir", "profile"]
                .iter()
                .any(|key| text.contains(key))
            {
                return Ok(Some("Hermes custom skill sources/profile configuration is not interpreted; manual review required".into()));
            }
        }
    }
    Ok(None)
}
fn scan(roots: &[Root], warnings: &mut Vec<String>) -> Result<Vec<Existing>, String> {
    let mut found = Vec::new();
    let mut budget = MAX_SCAN;
    for root in roots {
        if !exists(&root.path).map_err(|e| e.to_string())? {
            continue;
        }
        let mut stack = vec![(root.path.clone(), 0usize, BTreeSet::new())];
        while let Some((logical, depth, mut ancestors)) = stack.pop() {
            // Discovery may read supported provider symlinks. Writes never use
            // these canonical targets; preserve each lexical registration so
            // two Gemini aliases to one directory still count as duplicates.
            let canonical = fs::canonicalize(&logical).map_err(|e| {
                format!(
                    "Cannot resolve discovery directory {}: {e}",
                    logical.display()
                )
            })?;
            safe(&canonical).map_err(|e| e.to_string())?;
            if !ancestors.insert(canonical.clone()) {
                return Err("Discovery link cycle requires manual review".into());
            }
            let skill = canonical.join("SKILL.md");
            let mut leaf = false;
            if exists(&skill).map_err(|e| e.to_string())? {
                let readable = fs::canonicalize(&skill).map_err(|e| {
                    format!(
                        "Cannot resolve skill metadata {}: {e}",
                        logical.join("SKILL.md").display()
                    )
                })?;
                let header = discovery_frontmatter(&readable).map_err(|e| {
                    format!(
                        "Cannot read skill metadata {}: {e}",
                        logical.join("SKILL.md").display()
                    )
                })?;
                let name = skill_name(&header).map_err(|e| {
                    format!(
                        "Invalid skill metadata {}: {e}",
                        logical.join("SKILL.md").display()
                    )
                })?;
                if logical.file_name().is_some_and(|n| n == NAME) && name != NAME {
                    return Err(format!(
                        "Canonical skill folder declares a different skill name: {}",
                        logical.display()
                    ));
                }
                if name == NAME {
                    found.push(Existing {
                        path: logical.clone(),
                        scope: root.scope,
                        agents: root.agents.clone(),
                    });
                }
                // A portable skill directory is a discovery leaf. Supporting
                // assets are not additional discovery roots or setup targets.
                leaf = depth > 0;
            } else if logical.file_name().is_some_and(|n| n == NAME) {
                return Err(format!(
                    "Existing named directory has no readable SKILL.md: {}",
                    logical.display()
                ));
            }
            if leaf {
                continue;
            }
            for entry in fs::read_dir(&canonical).map_err(|e| e.to_string())? {
                if budget == 0 {
                    return Err("Skill discovery entry budget exhausted; no writes".into());
                }
                budget -= 1;
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                // OpenCode V2 also derives a skill ID from a source-root
                // Markdown filename. Other providers do not discover this form.
                if depth == 0
                    && root.agents.contains("opencode")
                    && entry.file_name() == format!("{NAME}.md").as_str()
                {
                    let readable = fs::canonicalize(&path).map_err(|e| {
                        format!("Cannot resolve skill registration {}: {e}", path.display())
                    })?;
                    bounded(&readable, 65536).map_err(|e| e.to_string())?;
                    found.push(Existing {
                        path: logical.join(entry.file_name()),
                        scope: root.scope,
                        agents: BTreeSet::from(["opencode".into()]),
                    });
                    continue;
                }
                let info = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                let directory = if linked(&info) {
                    let logical_path = logical.join(entry.file_name());
                    let resolved = match fs::canonicalize(&path) {
                        Ok(resolved) => resolved,
                        Err(e)
                            if e.kind() == io::ErrorKind::NotFound
                                && entry.file_name() != NAME
                                && entry.file_name() != format!("{NAME}.md").as_str() =>
                        {
                            let target = fs::read_link(&path).map_err(|e| {
                                format!(
                                    "Cannot read discovery link {}: {e}",
                                    logical_path.display()
                                )
                            })?;
                            let warning = format!(
                                "Dangling discovery link preserved and skipped: {} -> {}",
                                logical_path.display(),
                                target.display()
                            );
                            if !warnings.contains(&warning) {
                                warnings.push(warning);
                            }
                            continue;
                        }
                        Err(e) => {
                            return Err(format!(
                                "Cannot resolve discovery link {}: {e}",
                                logical_path.display()
                            ));
                        }
                    };
                    safe(&resolved).map_err(|e| e.to_string())?;
                    fs::metadata(resolved).map_err(|e| e.to_string())?.is_dir()
                } else {
                    info.is_dir()
                };
                if directory {
                    if depth >= 3 {
                        return Err("Skill discovery depth budget exhausted; no writes".into());
                    }
                    stack.push((
                        logical.join(entry.file_name()),
                        depth + 1,
                        ancestors.clone(),
                    ));
                }
            }
        }
    }
    Ok(found)
}
fn target(ctx: &Context, opt: &Options, id: &str) -> Result<PathBuf, String> {
    let base = if opt.scope == "project" {
        opt.project.clone().ok_or("Project scope needs --project")?
    } else {
        ctx.home.clone()
    };
    let root = match id {
        "opencode"
            if opt.selected.contains("claude")
                && !opt.selected.contains("codex")
                && !opt.selected.contains("gemini")
                && (opt.scope == "project"
                    || env_path(ctx, "CLAUDE_CONFIG_DIR", ctx.home.join(".claude"))
                        == ctx.home.join(".claude")) =>
        {
            base.join(".claude/skills")
        }
        "codex" | "opencode" => base.join(".agents/skills"),
        "gemini" => {
            if opt.scope == "user" {
                env_path(ctx, "GEMINI_CLI_HOME", ctx.home.clone()).join(".agents/skills")
            } else {
                base.join(".agents/skills")
            }
        }
        "claude" => {
            if opt.scope == "user" {
                env_path(ctx, "CLAUDE_CONFIG_DIR", ctx.home.join(".claude")).join("skills")
            } else {
                base.join(".claude/skills")
            }
        }
        "hermes" => {
            if opt.scope == "project" {
                return Err("Hermes project registration is unsupported; Git-root/profile/trust resolution requires manual review".into());
            }
            env_path(ctx, "HERMES_HOME", ctx.home.join(".hermes")).join("skills")
        }
        _ => return Err("Unknown agent".into()),
    };
    safe(&root).map_err(|e| e.to_string())?;
    Ok(root.join(NAME))
}
fn record_path(ctx: &Context, target: &Path) -> PathBuf {
    ctx.home.join(".rust-cache-management-setup").join(format!(
        "{}.json",
        digest(target.to_string_lossy().as_bytes())
    ))
}
fn check_record(ctx: &Context, path: &Path, payload: &Payload) -> Result<bool, String> {
    let record = record_path(ctx, path);
    if !exists(&record).map_err(|e| e.to_string())? {
        return Ok(false);
    }
    let record: Record =
        serde_json::from_slice(&bounded(&record, 65536).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    if record.schema_version != 1
        || record.target != path
        || record.version != env!("CARGO_PKG_VERSION")
        || record.files != payload.hashes
    {
        return Err("Managed version/record differs; automatic setup upgrades are not implemented, existing files preserved".into());
    }
    let mut expected: BTreeSet<String> = TEXT.iter().map(|v| v.to_string()).collect();
    let mut stack = vec![path.to_owned()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            safe(&p).map_err(|e| e.to_string())?;
            let rel = p
                .strip_prefix(path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let info = fs::symlink_metadata(&p).map_err(|e| e.to_string())?;
            if info.is_dir() && rel == "references" {
                stack.push(p);
            } else if info.is_file() && expected.remove(&rel) {
                if digest(&bounded(&p, MAX_BYTES).map_err(|e| e.to_string())?)
                    != payload.hashes[&rel]
                {
                    return Err("Modified managed file preserved".into());
                }
            } else {
                return Err("Unexpected managed destination member preserved".into());
            }
        }
    }
    if !expected.is_empty() {
        return Err("Managed destination is incomplete; preserved".into());
    }
    Ok(true)
}
fn plan(
    ctx: &Context,
    opt: &Options,
    roots: &[Root],
    payload: &Payload,
    warnings: &mut Vec<String>,
) -> Result<Vec<Action>, String> {
    let existing = scan(roots, warnings)?;
    let mut groups: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut result = Vec::new();
    for id in &opt.selected {
        if let Some(problem) = custom_problem(ctx, opt, id).map_err(|e| e.to_string())? {
            result.push(Action {
                agents: vec![id.clone()],
                target: ctx.home.clone(),
                status: "conflict".into(),
                detail: problem,
            });
            continue;
        }
        match target(ctx, opt, id) {
            Ok(path) => groups.entry(path).or_default().push(id.clone()),
            Err(detail) => result.push(Action {
                agents: vec![id.clone()],
                target: ctx.home.clone(),
                status: "conflict".into(),
                detail,
            }),
        }
    }
    let planned_paths: Vec<_> = groups.keys().cloned().collect();
    for (path, agents) in groups {
        let consumers = roots
            .iter()
            .find(|r| path.parent() == Some(r.path.as_path()))
            .map(|r| r.agents.clone())
            .unwrap_or_else(|| agents.iter().cloned().collect());
        let gemini_detected = detect(ctx, roots)
            .iter()
            .any(|a| a.id == "gemini" && a.detected);
        let visible: Vec<_> = existing
            .iter()
            .filter(|e| {
                if e.agents.is_disjoint(&consumers) {
                    return false;
                }
                // OpenCode resolves identical owned compatibility aliases as
                // one catalog ID. Do not extend this exception to Gemini or
                // uncertified manual, modified, or old-version copies.
                if e.path != path
                    && e.scope == opt.scope
                    && opencode_only_overlap(&e.agents, &consumers)
                    && matches!(check_record(ctx, &e.path, payload), Ok(true))
                {
                    return false;
                }
                // Ordinary Codex user/project layering is supported. If Gemini is
                // present, its cross-tier duplicate warning still requires review.
                !(agents.len() == 1
                    && agents[0] == "codex"
                    && !gemini_detected
                    && e.scope != opt.scope
                    && e.agents.contains("codex"))
            })
            .collect();
        let mut action = Action {
            agents: agents.clone(),
            target: path.clone(),
            status: "planned".into(),
            detail:
                "Create one fixed skill copy; no config, trust, login, or instruction-file edits"
                    .into(),
        };
        if visible.len() > 1 {
            action.status = "conflict".into();
            action.detail =
                "Multiple visible same-name registrations preserved; resolve duplicates manually"
                    .into();
        } else if let Some(item) = visible.first() {
            if (item.scope != opt.scope && item.path != path)
                || !agents.iter().all(|id| item.agents.contains(id))
            {
                action.status = "conflict".into();
                action.detail="Existing registration overlaps another provider or scope; no duplicate or migration created".into();
            } else {
                action.target = item.path.clone();
                match check_record(ctx, &item.path, payload) {
                    Ok(true) => {
                        action.status = "managed_reuse".into();
                        action.detail = "Matching managed copy verified; no files changed".into();
                    }
                    Ok(false) => {
                        action.status = "manual_present".into();
                        action.detail="Existing manual skill preserved and reported; version/content not adopted or certified".into();
                    }
                    Err(detail) => {
                        action.status = "conflict".into();
                        action.detail = detail;
                    }
                }
            }
        } else if exists(&path).map_err(|e| e.to_string())?
            || exists(&record_path(ctx, &path)).map_err(|e| e.to_string())?
        {
            action.status = "conflict".into();
            action.detail = "Existing target or incomplete ownership record preserved".into();
        }
        if action.status == "planned"
            && planned_paths
                .iter()
                .filter(|other| **other != path)
                .any(|other| {
                    roots
                        .iter()
                        .find(|r| other.parent() == Some(r.path.as_path()))
                        .is_some_and(|r| {
                            let overlap = r
                                .agents
                                .intersection(&consumers)
                                .any(|id| opt.selected.contains(id));
                            overlap
                                && !(opencode_only_overlap(&r.agents, &consumers)
                                    && managed_or_new(ctx, &path, payload)
                                    && managed_or_new(ctx, other, payload))
                        })
                })
        {
            action.status = "conflict".into();
            action.detail = "Selected provider aliases would see multiple planned copies; choose compatible groups separately and review existing registrations".into();
        }
        result.push(action);
    }
    Ok(result)
}
fn opencode_only_overlap(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    let mut common = a.intersection(b);
    common.next().is_some_and(|id| id == "opencode") && common.next().is_none()
}
fn managed_or_new(ctx: &Context, path: &Path, payload: &Payload) -> bool {
    if matches!(exists(path), Ok(false)) && matches!(exists(&record_path(ctx, path)), Ok(false)) {
        return true;
    }
    matches!(check_record(ctx, path, payload), Ok(true))
}
fn install(ctx: &Context, action: &Action, payload: &Payload) -> io::Result<()> {
    let root = action
        .target
        .parent()
        .ok_or_else(|| io::Error::other("Target parent missing"))?;
    let outside = root
        .parent()
        .ok_or_else(|| io::Error::other("Staging parent missing"))?;
    mkdir(root)?;
    safe(root)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let stage = outside.join(format!(
        ".rust-cache-management-stage-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&stage)?;
    let record_path = record_path(ctx, &action.target);
    let mut record_created = false;
    let result = (|| {
        fs::create_dir(stage.join("references"))?;
        for (name, bytes) in &payload.bytes {
            let mut f = exclusive_file(&stage.join(name))?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        let record = Record {
            schema_version: 1,
            target: action.target.clone(),
            version: env!("CARGO_PKG_VERSION").into(),
            source_manifest_sha256: payload.manifest.clone(),
            files: payload.hashes.clone(),
        };
        let mut f = exclusive_file(&record_path)?;
        record_created = true;
        serde_json::to_writer(&mut f, &record).map_err(io::Error::other)?;
        f.sync_all()?;
        publish(&stage, &action.target)
    })();
    if result.is_err() {
        if record_created {
            let _ = fs::remove_file(record_path);
        }
        let _ = fs::remove_dir_all(stage);
    }
    result
}
fn parse(args: Vec<OsString>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut o = Options {
        command: args
            .next()
            .and_then(|v| v.into_string().ok())
            .ok_or("Expected detect or setup")?,
        scope: "user".into(),
        ..Default::default()
    };
    if !["detect", "setup"].contains(&o.command.as_str()) {
        return Err("Expected agents detect or agents setup".into());
    }
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--json") => {}
            Some("--dry-run") => o.dry = true,
            Some("--yes") => o.yes = true,
            Some("--all-detected") => o.all = true,
            Some("--agent") => {
                let value = args
                    .next()
                    .and_then(|v| v.into_string().ok())
                    .ok_or("--agent needs names")?;
                for id in value.split(',') {
                    if !IDS.contains(&id) {
                        return Err(format!("Unsupported agent: {id}"));
                    }
                    o.selected.insert(id.to_string());
                }
            }
            Some("--scope") => {
                o.scope = args
                    .next()
                    .and_then(|v| v.into_string().ok())
                    .ok_or("--scope needs user or project")?;
                if !["user", "project"].contains(&o.scope.as_str()) {
                    return Err("Unsupported scope".into());
                }
            }
            Some("--project") => {
                if o.project.is_some() {
                    return Err("Repeated --project".into());
                }
                let path = PathBuf::from(args.next().ok_or("--project needs a path")?);
                if !path.is_absolute() || !path.is_dir() {
                    return Err("--project needs an absolute existing directory".into());
                }
                safe(&path).map_err(|e| e.to_string())?;
                o.project = Some(path);
            }
            _ => return Err("Unknown agents option".into()),
        }
    }
    if o.command == "detect"
        && (!o.selected.is_empty() || o.all || o.yes || o.dry || o.scope != "user")
    {
        return Err("detect accepts only --project and --json".into());
    }
    if o.command == "setup" {
        if o.selected.is_empty() && !o.all {
            return Err("Select --agent or --all-detected".into());
        }
        if !o.selected.is_empty() && o.all {
            return Err("Do not combine --agent and --all-detected".into());
        }
        if o.scope == "project" && o.project.is_none() {
            return Err("Project scope requires --project".into());
        }
    }
    Ok(o)
}
pub fn run(args: Vec<OsString>) -> i32 {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        let help = serde_json::json!({
            "schema_version": 1, "command": "help", "scope": "user", "dry_run": true,
            "status": "help", "agents": [], "actions": [], "errors": [], "warnings": [],
            "usage": "agents detect [--project ABS] [--json]\nagents setup (--agent codex,claude,gemini,opencode,hermes | --all-detected) [--scope user|project] [--project ABS] [--dry-run] [--yes] [--json]\nRepeat --agent or use comma-separated names. Project scope requires --project. Setup is create/reuse-only; manual or modified files are preserved. Writes require --yes and create skill copies plus HOME/.rust-cache-management-setup records/lock. Detection and dry-run never write or execute providers. Unknown versions remain unverified.\nExit: 0 success/reuse/preview/help, 1 IO, 2 invalid, 3 conflict, 4 consent required."
        });
        let mut out = io::stdout().lock();
        return if serde_json::to_writer_pretty(&mut out, &help).is_ok() && writeln!(out).is_ok() {
            0
        } else {
            1
        };
    }
    let parsed = parse(args);
    let mut response=Response{schema_version:1,command:"agents".into(),scope:"user".into(),dry_run:false,status:"invalid".into(),agents:Vec::new(),actions:Vec::new(),errors:Vec::new(),warnings:vec!["Discovery covers bounded mapped roots and treats valid child skill directories as leaves; custom sources, nested skills inside existing skills, plugins, enterprise rules and future versions may need manual review. Existing files are never overwritten. Use stable trusted directories: path checks are not a security sandbox against concurrent hostile replacements. No update/rollback is implemented for agent copies.".into()]};
    response.warnings.push("OpenCode may see identical current managed aliases as one catalog ID, with documented .agents over .claude precedence. Manual, modified, or uncertain overrides still require review; Gemini duplicate registrations remain conflicts.".into());
    let code = match parsed {
        Err(e) => {
            response.errors.push(e);
            2
        }
        Ok(mut opt) => {
            response.command = opt.command.clone();
            response.scope = opt.scope.clone();
            response.dry_run = opt.dry;
            match context() {
                Err(e) => {
                    response.status = "io_error".into();
                    response.errors.push(e.to_string());
                    1
                }
                Ok(ctx) => {
                    let roots = roots(&ctx, opt.project.as_deref());
                    response.agents = detect(&ctx, &roots);
                    if opt.command == "detect" {
                        response.status = "detected".into();
                        0
                    } else {
                        if opt.all {
                            opt.selected = response
                                .agents
                                .iter()
                                .filter(|a| a.detected)
                                .map(|a| a.id.clone())
                                .collect();
                        }
                        execute(&ctx, &opt, &roots, &mut response)
                    }
                }
            }
        }
    };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    if serde_json::to_writer_pretty(&mut out, &response).is_err() || writeln!(out).is_err() {
        return 1;
    }
    code
}
fn execute(ctx: &Context, opt: &Options, roots: &[Root], response: &mut Response) -> i32 {
    if opt.selected.is_empty() {
        response.status = "complete".into();
        response
            .warnings
            .push("No agents detected; no setup performed".into());
        return 0;
    }
    let payload = match source(&ctx.exe) {
        Ok(p) => p,
        Err(e) => {
            response.status = "io_error".into();
            response.errors.push(format!(
                "Verified executable release payload unavailable: {e}"
            ));
            return 1;
        }
    };
    response.actions = match plan(ctx, opt, roots, &payload, &mut response.warnings) {
        Ok(p) => p,
        Err(e) => {
            response.status = "conflict".into();
            response.errors.push(e);
            return 3;
        }
    };
    if response.actions.iter().any(|a| a.status == "conflict") {
        response.status = "conflict".into();
        return 3;
    }
    if opt.dry {
        response.status = "planned".into();
        return 0;
    }
    if !response.actions.iter().any(|a| a.status == "planned") {
        response.status = "complete".into();
        return 0;
    }
    if !opt.yes {
        response.status = "consent_required".into();
        response.errors.push(
            "Explicit --yes is required before setup writes; use --dry-run to preview".into(),
        );
        return 4;
    }
    let state = ctx.home.join(".rust-cache-management-setup");
    let result = (|| -> io::Result<()> {
        mkdir(&state)?;
        let lock_path = state.join("setup.lock");
        safe(&lock_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let lock = options.open(&lock_path)?;
        if linked(&lock.metadata()?) {
            return Err(io::Error::other("Linked lock file preserved"));
        }
        lock.try_lock_exclusive()?;
        let refreshed =
            plan(ctx, opt, roots, &payload, &mut response.warnings).map_err(io::Error::other)?;
        if refreshed.iter().any(|a| a.status == "conflict") {
            response.actions = refreshed;
            return Err(io::Error::other("Setup preconditions changed under lock"));
        }
        response.actions = refreshed;
        for action in &mut response.actions {
            if action.status == "planned" {
                install(ctx, action, &payload)?;
                action.status = "created".into();
                action.detail="Created a fixed skill copy with baseline outside discovery roots; agent activation/trust remains separate".into();
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => {
            response.status = "complete".into();
            0
        }
        Err(e) => {
            response.status = "io_error".into();
            response.errors.push(e.to_string());
            1
        }
    }
}
