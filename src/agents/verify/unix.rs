//! Opt-in provider execution. No prompts, model turns, logins or configuration edits.
use super::*;
use serde_json::{json, Value};
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

struct Temporary(PathBuf);
impl Temporary {
    fn new() -> io::Result<Self> {
        let base = fs::canonicalize(std::env::temp_dir())?;
        for attempt in 0..100 {
            let path = base.join(format!(
                "rcm-agent-verify-{}-{}-{attempt}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::other(
            "Cannot allocate private verification directory",
        ))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Probe {
    child: Child,
    stdout: ChildStdout,
    stderr: ChildStderr,
    out: Vec<u8>,
    err: Vec<u8>,
    cursor: usize,
    deadline: Instant,
}
impl Probe {
    fn start(mut command: Command, timeout: Duration) -> io::Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("Missing stdout pipe"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("Missing stderr pipe"))?;
        let probe = Self {
            child,
            stdout,
            stderr,
            out: Vec::new(),
            err: Vec::new(),
            cursor: 0,
            deadline: Instant::now() + timeout,
        };
        #[cfg(unix)]
        {
            nonblocking(
                probe
                    .child
                    .stdin
                    .as_ref()
                    .ok_or_else(|| io::Error::other("Missing stdin pipe"))?,
            )?;
            nonblocking(&probe.stdout)?;
            nonblocking(&probe.stderr)?;
        }
        Ok(probe)
    }
    fn pump(&mut self) -> Result<bool, String> {
        let before = self.out.len() + self.err.len();
        read_available(&mut self.stdout, &mut self.out)?;
        read_available(&mut self.stderr, &mut self.err)?;
        if Instant::now() >= self.deadline {
            return Err("Provider probe timed out".into());
        }
        Ok(before != self.out.len() + self.err.len())
    }
    fn send(&mut self, message: Value) -> Result<(), String> {
        let mut bytes = serde_json::to_vec(&message).map_err(|e| e.to_string())?;
        if bytes.len() > OUTPUT_LIMIT {
            return Err("Provider request exceeded the 4 MiB budget".into());
        }
        bytes.push(b'\n');
        let mut offset = 0;
        while offset < bytes.len() {
            self.pump()?;
            let result = self
                .child
                .stdin
                .as_mut()
                .ok_or("Provider input is closed")?
                .write(&bytes[offset..]);
            match result {
                Ok(0) => return Err("Provider input closed during request".into()),
                Ok(n) => offset += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10))
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }
    fn response(&mut self, matches: impl Fn(&Value) -> bool) -> Result<Value, String> {
        loop {
            let progressed = self.pump()?;
            while let Some(length) = self.out[self.cursor..].iter().position(|b| *b == b'\n') {
                let end = self.cursor + length;
                let line = &self.out[self.cursor..end];
                self.cursor = end + 1;
                if let Ok(value) = serde_json::from_slice(line) {
                    if matches(&value) {
                        return Ok(value);
                    }
                }
            }
            if self.child.try_wait().map_err(|e| e.to_string())?.is_some() {
                return Err("Provider exited before the expected catalog response".into());
            }
            if !progressed {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
    fn output(mut self) -> Result<Vec<u8>, String> {
        self.child.stdin.take();
        loop {
            let progressed = self.pump()?;
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                self.pump()?;
                return if status.success() {
                    Ok(std::mem::take(&mut self.out))
                } else {
                    Err(format!("Provider exited unsuccessfully ({status})"))
                };
            }
            if !progressed {
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}
impl Drop for Probe {
    fn drop(&mut self) {
        self.child.stdin.take();
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.child.id() as i32), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(unix)]
fn nonblocking(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let descriptor = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn read_available(pipe: &mut impl Read, output: &mut Vec<u8>) -> Result<(), String> {
    let mut buffer = [0; 8192];
    loop {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(()),
            Ok(n) => {
                if output.len() + n > OUTPUT_LIMIT {
                    return Err("Provider output exceeded the 4 MiB budget".into());
                }
                output.extend_from_slice(&buffer[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn command(ctx: &Context, executable: &Path, temporary: &Path) -> Command {
    let mut command = Command::new(executable);
    command.env_clear().current_dir(temporary);
    for key in ["PATH", "SystemRoot", "WINDIR", "ComSpec", "PATHEXT"] {
        if let Some(value) = ctx.env.get(key) {
            command.env(key, value);
        }
    }
    for key in [
        "HOME",
        "USERPROFILE",
        "TMPDIR",
        "TMP",
        "TEMP",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        command.env(key, temporary);
    }
    command
        .env("TERM", "dumb")
        .env("NO_COLOR", "1")
        .env("DISABLE_AUTOUPDATER", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1");
    for (key, suffix) in [
        ("XDG_CONFIG_HOME", ".config"),
        ("XDG_DATA_HOME", ".local/share"),
        ("XDG_CACHE_HOME", ".cache"),
        ("XDG_STATE_HOME", ".local/state"),
    ] {
        command.env(key, temporary.join(suffix));
    }
    let mut paths = vec![executable.parent().unwrap_or(temporary).to_owned()];
    if let Some(path) = ctx.env.get("PATH") {
        paths.extend(std::env::split_paths(path).take(128));
    }
    if let Ok(path) = std::env::join_paths(paths) {
        command.env("PATH", path);
    }
    command
}

fn version(id: &str, bytes: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    text.lines().take(64).find_map(|line| {
        let line = line.trim();
        let candidate = match id {
            "codex" => line.strip_prefix("codex-cli ")?.split_whitespace().next()?,
            "claude" => line.strip_suffix(" (Claude Code)")?,
            "hermes" => line
                .strip_prefix("Hermes Agent v")?
                .split_whitespace()
                .next()?,
            "opencode" => line
                .strip_prefix("opencode v")
                .map(|s| s.split_whitespace().next().unwrap_or(""))
                .unwrap_or(line.strip_prefix('v').unwrap_or(line)),
            "gemini" => line.strip_prefix('v').unwrap_or(line),
            _ => return None,
        };
        if candidate.len() >= 128 {
            return None;
        }
        let (release, build) = candidate
            .split_once('+')
            .map_or((candidate, None), |(a, b)| (a, Some(b)));
        let (core, prerelease) = release
            .split_once('-')
            .map_or((release, None), |(a, b)| (a, Some(b)));
        let core: Vec<_> = core.split('.').collect();
        if core.len() != 3
            || core.iter().any(|s| {
                s.is_empty()
                    || (s.len() > 1 && s.starts_with('0'))
                    || !s.bytes().all(|b| b.is_ascii_digit())
            })
        {
            return None;
        }
        let identifiers = |value: &str, numeric_zeroes: bool| {
            value.split('.').all(|s| {
                !s.is_empty()
                    && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    && (numeric_zeroes
                        || !s.bytes().all(|b| b.is_ascii_digit())
                        || s.len() == 1
                        || !s.starts_with('0'))
            })
        };
        if build.is_some_and(|s| !identifiers(s, true))
            || prerelease.is_some_and(|s| !identifiers(s, false))
        {
            return None;
        }
        Some(candidate.to_string())
    })
}

fn codex(
    ctx: &Context,
    exe: &Path,
    temp: &Path,
    roots: &[PathBuf],
    expected: &[PathBuf],
) -> Result<usize, String> {
    let state = temp.join("codex-state");
    fs::create_dir(&state).map_err(|e| e.to_string())?;
    if roots.is_empty() || roots.len() > 2 {
        return Err("Codex catalog mapping requires one or two known skill roots".into());
    }
    fs::create_dir(temp.join(".agents")).map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(&roots[0], temp.join(".agents/skills"))
        .map_err(|e| e.to_string())?;
    if let Some(legacy) = roots.get(1) {
        std::os::unix::fs::symlink(legacy, state.join("skills")).map_err(|e| e.to_string())?;
    }
    let mut cmd = command(ctx, exe, temp);
    cmd.env("CODEX_HOME", &state).args([
        "app-server",
        "--stdio",
        "-c",
        "analytics.enabled=false",
        "-c",
        "feedback.enabled=false",
    ]);
    let mut probe = Probe::start(cmd, PROBE_TIMEOUT).map_err(|e| e.to_string())?;
    probe.send(json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"rcm-catalog-check","version":env!("CARGO_PKG_VERSION")}}}))?;
    let initialized = probe.response(|v| v["id"] == 1)?;
    if initialized.get("result").is_none() {
        return Err("Codex initialize failed".into());
    }
    probe.send(json!({"method":"initialized","params":{}}))?;
    probe
        .send(json!({"id":2,"method":"skills/list","params":{"cwds":[temp],"forceReload":true}}))?;
    let result = probe.response(|v| v["id"] == 2)?;
    codex_catalog(&result, expected)
}
fn mapped_metadata(actual: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| {
        if actual.file_name() == Some(std::ffi::OsStr::new("SKILL.md"))
            && fs::canonicalize(root)
                .ok()
                .is_some_and(|root| actual.starts_with(root))
        {
            return true;
        }
        fs::read_dir(root).ok().is_some_and(|entries| {
            entries.take(MAX_SCAN).filter_map(Result::ok).any(|entry| {
                fs::canonicalize(entry.path().join("SKILL.md"))
                    .ok()
                    .as_deref()
                    == Some(actual)
            })
        })
    })
}
fn codex_catalog(value: &Value, expected: &[PathBuf]) -> Result<usize, String> {
    let data = value["result"]["data"]
        .as_array()
        .ok_or("Malformed Codex catalog response")?;
    let mut matches = Vec::new();
    for entry in data {
        let skills = entry["skills"]
            .as_array()
            .ok_or("Malformed Codex skill list")?;
        for skill in skills {
            if skill["name"] == NAME {
                matches.push(skill);
            }
        }
    }
    if matches.is_empty() {
        return Err("not_registered: Codex catalog does not list the target skill".into());
    }
    if matches.len() != 1 {
        return Err("ambiguous: Codex catalog lists duplicate target skills".into());
    }
    let skill = matches[0];
    if skill["enabled"] != true {
        return Err("disabled: Codex catalog target is disabled or has unknown enablement".into());
    }
    let path = skill["path"]
        .as_str()
        .ok_or("Codex catalog omitted the target path")?;
    let actual =
        fs::canonicalize(path).map_err(|e| format!("Cannot resolve Codex catalog path: {e}"))?;
    if !mapped_metadata(&actual, expected) {
        return Err("Codex catalog name matched a different source path".into());
    }
    Ok(data
        .iter()
        .filter_map(|entry| entry["errors"].as_array())
        .map(Vec::len)
        .sum())
}

fn claude(
    ctx: &Context,
    exe: &Path,
    temp: &Path,
    roots: &[PathBuf],
) -> Result<&'static str, String> {
    let config = temp.join("claude-state");
    fs::create_dir(&config).map_err(|e| e.to_string())?;
    if roots.len() != 1 {
        return Err("Claude catalog mapping requires one unambiguous skill root".into());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&roots[0], config.join("skills")).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    return Err(
        "Read-only Claude skill-root mapping is not available on Windows; version evidence only"
            .into(),
    );
    #[cfg(unix)]
    {
        let mut cmd = command(ctx, exe, temp);
        cmd.env("CLAUDE_CONFIG_DIR", &config).args([
            "--print",
            "--no-session-persistence",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--setting-sources",
            "user",
            "--settings",
            "{\"disableAllHooks\":true}",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
        ]);
        let mut probe = Probe::start(cmd, PROBE_TIMEOUT).map_err(|e| e.to_string())?;
        probe.send(json!({"type":"control_request","request_id":"rcm-catalog-init","request":{"subtype":"initialize"}}))?;
        let result = probe.response(|v| {
            v["type"] == "control_response" && v["response"]["request_id"] == "rcm-catalog-init"
        })?;
        claude_catalog(&result)?;
        Ok("listed")
    }
}
fn claude_catalog(value: &Value) -> Result<(), String> {
    let response = &value["response"];
    if response["subtype"] != "success" {
        return Err("Claude initialize failed".into());
    }
    let commands = response["response"]["commands"]
        .as_array()
        .ok_or("Malformed Claude command catalog")?;
    match commands.iter().filter(|c| c["name"] == NAME).count() {
        0 => {
            return Err(
                "not_registered: Claude skill command is absent from the isolated catalog".into(),
            )
        }
        1 => {}
        _ => {
            return Err(
                "ambiguous: Claude target command is duplicated in the isolated catalog".into(),
            )
        }
    }
    Ok(())
}

fn gemini(
    ctx: &Context,
    exe: &Path,
    temp: &Path,
    roots: &[PathBuf],
) -> Result<&'static str, String> {
    if roots.is_empty() || roots.len() > 2 {
        return Err("Gemini catalog mapping requires one or two known skill roots".into());
    }
    for (source, folder) in roots.iter().zip([".agents", ".gemini"]) {
        fs::create_dir_all(temp.join(folder)).map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(source, temp.join(folder).join("skills"))
            .map_err(|e| e.to_string())?;
    }
    let mut cmd = command(ctx, exe, temp);
    cmd.env("GEMINI_CLI_HOME", temp)
        .env("GEMINI_TELEMETRY_ENABLED", "false")
        .args(["skills", "list"]);
    let bytes = Probe::start(cmd, PROBE_TIMEOUT)
        .map_err(|e| e.to_string())?
        .output()?;
    gemini_catalog(&bytes, roots)?;
    Ok("enabled")
}
fn gemini_catalog(bytes: &[u8], roots: &[PathBuf]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Non-UTF-8 Gemini catalog")?;
    let lines: Vec<_> = text.lines().collect();
    let enabled = format!("{NAME} [Enabled]");
    let disabled = format!("{NAME} [Disabled]");
    let entries: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == enabled || line.trim() == disabled)
        .collect();
    if entries.is_empty() {
        return Err("not_registered: Gemini does not list the target skill".into());
    }
    if entries.len() != 1 {
        return Err("ambiguous: Gemini lists duplicate target entries".into());
    }
    let (index, line) = entries[0];
    if line.trim() == disabled {
        return Err("disabled: Gemini target skill is disabled".into());
    }
    let path = lines
        .iter()
        .skip(index + 1)
        .take_while(|line| line.trim().is_empty() || line.starts_with(char::is_whitespace))
        .find_map(|line| line.trim().strip_prefix("Location:").map(str::trim))
        .ok_or("Gemini catalog omitted the target location")?;
    let actual =
        fs::canonicalize(path).map_err(|e| format!("Cannot resolve Gemini catalog path: {e}"))?;
    if !mapped_metadata(&actual, roots) {
        return Err("Gemini target location does not belong to a mapped source root".into());
    }
    Ok(())
}

fn skill_snapshot(source: &Path, destination: &Path) -> Result<(), String> {
    const TOTAL_BYTES: usize = 16 * 1024 * 1024;
    let mut nodes = MAX_SCAN;
    let mut bytes_left = TOTAL_BYTES;
    let mut stack = vec![(source.to_owned(), PathBuf::new(), 0usize, BTreeSet::new())];
    fs::create_dir(destination).map_err(|e| e.to_string())?;
    while let Some((logical, relative, depth, mut ancestors)) = stack.pop() {
        let canonical = fs::canonicalize(&logical)
            .map_err(|e| format!("Cannot read snapshot directory {}: {e}", logical.display()))?;
        safe(&canonical).map_err(|e| e.to_string())?;
        if !ancestors.insert(canonical.clone()) {
            return Err("Skill snapshot directory cycle requires review".into());
        }
        let metadata = canonical.join("SKILL.md");
        if exists(&metadata).map_err(|e| e.to_string())? {
            let metadata = fs::canonicalize(&metadata).map_err(|e| {
                format!(
                    "Cannot resolve snapshot metadata {}: {e}",
                    metadata.display()
                )
            })?;
            let bytes = bounded(&metadata, MAX_BYTES.min(bytes_left as u64)).map_err(|e| {
                format!("Cannot copy snapshot metadata {}: {e}", metadata.display())
            })?;
            bytes_left = bytes_left
                .checked_sub(bytes.len())
                .ok_or("Skill snapshot byte budget exhausted")?;
            let folder = destination.join(&relative);
            mkdir(&folder).map_err(|e| e.to_string())?;
            exclusive_file(&folder.join("SKILL.md"))
                .and_then(|mut file| file.write_all(&bytes))
                .map_err(|e| e.to_string())?;
            continue;
        }
        for entry in fs::read_dir(&canonical).map_err(|e| e.to_string())? {
            if nodes == 0 {
                return Err("Skill snapshot entry budget exhausted".into());
            }
            nodes -= 1;
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_name() == ".hub" {
                continue;
            }
            let resolved = match fs::canonicalize(entry.path()) {
                Ok(path) => path,
                Err(e) if e.kind() == io::ErrorKind::NotFound && entry.file_name() != NAME => {
                    continue
                }
                Err(e) => {
                    return Err(format!(
                        "Cannot resolve snapshot entry {}: {e}",
                        entry.path().display()
                    ))
                }
            };
            if fs::metadata(&resolved).map_err(|e| e.to_string())?.is_dir() {
                if depth >= 3 {
                    return Err("Skill snapshot depth budget exhausted".into());
                }
                stack.push((
                    resolved,
                    relative.join(entry.file_name()),
                    depth + 1,
                    ancestors.clone(),
                ));
            }
        }
    }
    Ok(())
}

fn hermes(
    ctx: &Context,
    exe: &Path,
    temp: &Path,
    roots: &[PathBuf],
) -> Result<&'static str, String> {
    if roots.len() != 1 {
        return Err("Hermes catalog mapping requires one unambiguous local skill root".into());
    }
    let state = temp.join("hermes-state");
    fs::create_dir(&state).map_err(|e| e.to_string())?;
    skill_snapshot(&roots[0], &state.join("skills"))?;
    let mut cmd = command(ctx, exe, temp);
    cmd.env("HERMES_HOME", state).env("COLUMNS", "200").args([
        "skills",
        "list",
        "--source",
        "local",
        "--enabled-only",
    ]);
    let bytes = Probe::start(cmd, PROBE_TIMEOUT)
        .map_err(|e| e.to_string())?
        .output()?;
    hermes_catalog(&bytes)?;
    Ok("enabled")
}
fn hermes_catalog(bytes: &[u8]) -> Result<(), String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Non-UTF-8 Hermes catalog")?;
    let mut matches = 0;
    for line in text.lines() {
        let Some(row) = line
            .trim()
            .strip_prefix('│')
            .and_then(|line| line.strip_suffix('│'))
        else {
            continue;
        };
        let cells: Vec<_> = row.split('│').map(str::trim).collect();
        if cells.len() == 5 && cells[0] == NAME && cells[2] == "local" && cells[4] == "enabled" {
            matches += 1;
        }
    }
    match matches {
        0 => Err(
            "not_registered: Hermes enabled local catalog does not list the target skill".into(),
        ),
        1 => Ok(()),
        _ => Err("ambiguous: Hermes lists duplicate enabled target skills".into()),
    }
}

fn opencode(
    ctx: &Context,
    exe: &Path,
    temp: &Path,
    roots: &[PathBuf],
) -> Result<&'static str, String> {
    for source in roots {
        let parent = source
            .parent()
            .and_then(Path::file_name)
            .and_then(|s| s.to_str())
            .ok_or("Unknown OpenCode discovery root")?;
        let relative = match parent {
            ".agents" => ".agents/skills",
            ".claude" => ".claude/skills",
            ".opencode" => ".opencode/skills",
            "opencode" => ".config/opencode/skills",
            _ => return Err("Unknown OpenCode discovery root mapping".into()),
        };
        let destination = temp.join(relative);
        fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(source, destination).map_err(|e| e.to_string())?;
    }
    let mut cmd = command(ctx, exe, temp);
    cmd.env("OPENCODE_DISABLE_AUTOUPDATE", "1")
        .env("OPENCODE_DISABLE_SHARE", "1")
        .args(["api", "--standalone", "skill.list"]);
    let bytes = Probe::start(cmd, PROBE_TIMEOUT)
        .map_err(|e| e.to_string())?
        .output()?;
    opencode_catalog(&bytes, roots)?;
    Ok("listed")
}
fn opencode_catalog(bytes: &[u8], roots: &[PathBuf]) -> Result<(), String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| "Malformed OpenCode catalog JSON")?;
    let skills = value["data"]
        .as_array()
        .ok_or("Malformed OpenCode skill list")?;
    let found: Vec<_> = skills.iter().filter(|s| s["id"] == NAME).collect();
    if found.is_empty() {
        return Err("not_registered: OpenCode catalog does not list the target skill ID".into());
    }
    if found.len() != 1 {
        return Err("ambiguous: OpenCode catalog lists duplicate target IDs".into());
    }
    let path = found[0]["path"]
        .as_str()
        .ok_or("OpenCode catalog omitted target path")?;
    let actual =
        fs::canonicalize(path).map_err(|e| format!("Cannot resolve OpenCode catalog path: {e}"))?;
    if !mapped_metadata(&actual, roots)
        && !roots.iter().any(|root| {
            fs::canonicalize(root.join(format!("{NAME}.md")))
                .ok()
                .as_deref()
                == Some(&actual)
        })
    {
        return Err("OpenCode target ID matched another source path".into());
    }
    Ok(())
}

pub(super) fn run(ctx: &Context, opt: &Options, roots: &[Root], response: &mut Response) -> i32 {
    if opt.selected.is_empty() {
        response.status = "complete".into();
        response
            .warnings
            .push("No agents selected or detected; no verification performed".into());
        return 0;
    }
    for agent in &mut response.agents {
        if !opt.selected.contains(&agent.id) {
            continue;
        }
        agent.compatibility = "unverified";
        agent.warnings.clear();
        let candidates: Vec<_> = roots
            .iter()
            .filter(|r| r.scope == opt.scope && r.agents.contains(&agent.id) && r.path.is_dir())
            .map(|r| r.path.clone())
            .collect();
        let invalid_source = candidates.iter().find(|path| {
            !path.is_absolute()
                || path.to_str().is_none()
                || path.components().any(|c| matches!(c, Component::ParentDir))
        });
        agent.source_paths = candidates
            .iter()
            .filter(|path| {
                path.is_absolute()
                    && path.to_str().is_some()
                    && !path.components().any(|c| matches!(c, Component::ParentDir))
            })
            .cloned()
            .collect();
        let Some(exe) = command_present(ctx, &agent.id).and_then(|p| fs::canonicalize(p).ok())
        else {
            agent.version_status = "not_available";
            agent.catalog_status = "not_checked";
            agent.reason =
                "No executable was found; configuration-directory presence alone is insufficient"
                    .into();
            continue;
        };
        let temporary = match Temporary::new() {
            Ok(temp) => temp,
            Err(error) => {
                agent.reason = error.to_string();
                agent.version_status = "failed";
                continue;
            }
        };
        let mut cmd = command(ctx, &exe, &temporary.0);
        cmd.arg("--version");
        match Probe::start(cmd, PROBE_TIMEOUT)
            .map_err(|e| e.to_string())
            .and_then(Probe::output)
        {
            Ok(bytes) => {
                agent.version = version(&agent.id, &bytes);
                agent.version_status = if agent.version.is_some() {
                    "reported"
                } else {
                    "unrecognized"
                };
            }
            Err(error) => {
                agent.version_status = "failed";
                agent.reason = error;
                continue;
            }
        }
        if let Some(path) = invalid_source {
            agent.catalog_status = "unsupported_configuration";
            agent.reason = format!(
                "Discovery root must be an absolute Unicode path without parent traversal: {}",
                path.display()
            );
            continue;
        }
        if agent.source_paths.is_empty() {
            agent.catalog_status = "not_registered";
            agent.reason = "No target skill registration was found in the requested scope".into();
            continue;
        }
        match custom_problem(ctx, opt, &agent.id) {
            Ok(None) => {}
            Ok(Some(problem)) => {
                agent.catalog_status = "unsupported_configuration";
                agent.reason = problem;
                continue;
            }
            Err(error) => {
                agent.catalog_status = "failed";
                agent.reason = error.to_string();
                continue;
            }
        }
        let mapped = agent.source_paths.clone();
        let catalog = match agent.id.as_str() {
            "codex" => {
                codex(ctx, &exe, &temporary.0, &mapped, &mapped).map(|count| ("enabled", count))
            }
            "claude" => claude(ctx, &exe, &temporary.0, &mapped).map(|status| (status, 0)),
            "gemini" => gemini(ctx, &exe, &temporary.0, &mapped).map(|status| (status, 0)),
            "hermes" => hermes(ctx, &exe, &temporary.0, &mapped).map(|status| (status, 0)),
            "opencode" => opencode(ctx, &exe, &temporary.0, &mapped).map(|status| (status, 0)),
            _ => {
                agent.catalog_status = "unsupported";
                agent.reason = "No supported no-model native catalog adapter is available; version evidence does not establish compatibility".into();
                continue;
            }
        };
        agent.runtime_scope = if agent.id == "hermes" {
            "isolated_skill_snapshot"
        } else {
            "isolated_native_catalog"
        };
        match catalog {
            Ok((status, errors)) => {
                agent.catalog_status = status;
                agent.compatibility = if agent.id == "hermes" {
                    "parser_verified"
                } else {
                    "verified"
                };
                if errors != 0 {
                    agent.warnings.push(format!("Native catalog reported {errors} unrelated discovery errors; target identity, path and enablement were checked separately"));
                }
                agent.reason = if agent.id == "hermes" {
                    "Target skill metadata appears in the native catalog of a bounded private snapshot; original source discovery, ordinary-session settings and instruction execution were not verified".into()
                } else {
                    "Target skill appears in the native catalog using isolated configuration and mapped source roots; ordinary-session overrides, authentication, trust and instruction execution were not checked".into()
                };
            }
            Err(error) => {
                agent.catalog_status = if error.starts_with("not_registered:") {
                    "not_registered"
                } else if error.starts_with("disabled:") {
                    "disabled"
                } else if error.starts_with("ambiguous:") {
                    "ambiguous"
                } else {
                    "failed"
                };
                agent.reason = error;
            }
        }
    }
    let verified = response
        .agents
        .iter()
        .filter(|a| opt.selected.contains(&a.id))
        .all(|a| a.compatibility == "verified");
    response.status = if verified { "verified" } else { "partial" }.into();
    if verified {
        0
    } else {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_is_bounded_semver_evidence_only() {
        assert_eq!(
            version("codex", b"codex-cli 0.166.0\n").as_deref(),
            Some("0.166.0")
        );
        assert_eq!(
            version("claude", b"2.1.318 (Claude Code)\n").as_deref(),
            Some("2.1.318")
        );
        assert_eq!(version("codex", b"command present"), None);
        assert_eq!(
            version("codex", b"warning dependency 1.2.3\n127.0.0.1\n"),
            None
        );
        assert_eq!(
            version("gemini", b"127.0.0.1\nwarning dependency 1.2.3\n"),
            None
        );
        assert_eq!(version("claude", b"1.2.3garbage (Claude Code)\n"), None);
        assert_eq!(
            version("codex", b"codex-cli 1.2.3-beta.1+build.4\n").as_deref(),
            Some("1.2.3-beta.1+build.4")
        );
        assert_eq!(
            version("hermes", b"Hermes Agent v0.21.3\n").as_deref(),
            Some("0.21.3")
        );
        assert_eq!(version("hermes", b"Hermes Agent v0.21.3 (2026.9.14) upstream hash\nPython: 3.11.13\nOpenAI SDK: 2.24.0\n").as_deref(), Some("0.21.3"));
        assert_eq!(
            version("opencode", b"opencode v2.2.5\n").as_deref(),
            Some("2.2.5")
        );
        assert_eq!(version("gemini", b"1.2.3+build+extra\n"), None);
        assert_eq!(version("gemini", b"1.2.3-01\n"), None);
        assert_eq!(version("gemini", b"01.2.3\n"), None);
    }
    #[test]
    fn codex_catalog_checks_exact_identity_path_and_enablement() {
        let temp = Temporary::new().unwrap();
        let skill = temp.0.join("renamed-target");
        fs::create_dir(&skill).unwrap();
        let path = skill.join("SKILL.md");
        fs::write(&path, "---\nname: rust-cache-management\n---\n").unwrap();
        let catalog = |name, enabled, path| json!({"result":{"data":[{"skills":[{"name":name,"enabled":enabled,"path":path}],"errors":[{"message":"unrelated invalid metadata"}]}]}});
        let roots = [temp.0.clone()];
        assert_eq!(
            codex_catalog(&catalog(NAME, true, &path), &roots).unwrap(),
            1
        );
        assert!(codex_catalog(&catalog(NAME, false, &path), &roots)
            .unwrap_err()
            .starts_with("disabled:"));
        assert!(
            codex_catalog(&catalog("rust-cache-management-extra", true, &path), &roots)
                .unwrap_err()
                .starts_with("not_registered:")
        );
        assert!(
            codex_catalog(&catalog(NAME, true, &path), &[temp.0.join("another-root")]).is_err()
        );
        assert!(codex_catalog(&json!({"result":{}}), &roots).is_err());
    }
    #[test]
    fn exact_claude_catalog_match_is_required() {
        let response = |name| json!({"response":{"subtype":"success","response":{"commands":[{"name":name}]}}});
        assert!(claude_catalog(&response(NAME)).is_ok());
        assert!(claude_catalog(&response("rust-cache-management-extra")).is_err());
        assert!(claude_catalog(&json!({"response":{"subtype":"success"}})).is_err());
    }
    #[test]
    fn text_catalogs_require_exact_enabled_entries_and_source_locations() {
        let temp = Temporary::new().unwrap();
        let path = temp.0.join("SKILL.md");
        fs::write(&path, "---\nname: rust-cache-management\n---\n").unwrap();
        let listing = format!(
            "rust-cache-management [Enabled]\n  Location: {}\n",
            path.display()
        );
        assert!(gemini_catalog(listing.as_bytes(), std::slice::from_ref(&temp.0)).is_ok());
        assert!(gemini_catalog(
            listing
                .replace("rust-cache-management [", "rust-cache-management-extra [")
                .as_bytes(),
            std::slice::from_ref(&temp.0)
        )
        .is_err());
        assert!(gemini_catalog(
            listing.replace("[Enabled]", "[Disabled]").as_bytes(),
            std::slice::from_ref(&temp.0)
        )
        .is_err());
        assert!(gemini_catalog(listing.as_bytes(), &[temp.0.join("wrong-root")]).is_err());
        assert!(hermes_catalog(
            "│ rust-cache-management │ Rust caches │ local │ local │ enabled │\n".as_bytes()
        )
        .is_ok());
        assert!(hermes_catalog(
            "│ rust-cache-management-extra │ Rust caches │ local │ local │ enabled │\n".as_bytes()
        )
        .is_err());
        assert!(hermes_catalog(
            "│ rust-cache-management │ Rust caches │ local │ local │ disabled │\n".as_bytes()
        )
        .is_err());
        assert!(hermes_catalog(
            "│ │ rust-cache-management │ local │ local │ enabled │\n".as_bytes()
        )
        .is_err());
        assert!(hermes_catalog(
            "│ rust-cache-management │ │ local │ local │ enabled │\n".as_bytes()
        )
        .is_ok());
    }
    #[test]
    fn opencode_catalog_uses_exact_ids_and_mapped_metadata_or_flat_files() {
        let temp = Temporary::new().unwrap();
        let path = temp.0.join("SKILL.md");
        fs::write(&path, "---\nname: rust-cache-management\n---\n").unwrap();
        let catalog = |id, path| {
            json!({"data":[{"id":id,"name":"Human display title","path":path,"content":"must never be emitted"}]}).to_string()
        };
        assert!(opencode_catalog(
            catalog(NAME, &path).as_bytes(),
            std::slice::from_ref(&temp.0)
        )
        .is_ok());
        assert!(opencode_catalog(
            catalog("rust-cache-management-extra", &path).as_bytes(),
            std::slice::from_ref(&temp.0)
        )
        .is_err());
        assert!(opencode_catalog(
            catalog(NAME, &path).as_bytes(),
            &[temp.0.join("wrong-root")]
        )
        .is_err());
        let flat = temp.0.join("rust-cache-management.md");
        fs::write(&flat, "Native flat skill").unwrap();
        assert!(opencode_catalog(
            catalog(NAME, &flat).as_bytes(),
            std::slice::from_ref(&temp.0)
        )
        .is_ok());
    }
    #[test]
    fn native_catalog_paths_can_resolve_through_read_only_skill_aliases() {
        let temp = Temporary::new().unwrap();
        let root = temp.0.join("skills");
        let outside = temp.0.join("outside");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(
            outside.join("SKILL.md"),
            "---\nname: rust-cache-management\n---\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside, root.join("renamed-alias")).unwrap();
        let path = root.join("renamed-alias/SKILL.md");
        let catalog = json!({"result":{"data":[{"skills":[{"name":NAME,"path":path,"enabled":true}],"errors":[]}]}});
        assert!(codex_catalog(&catalog, std::slice::from_ref(&root)).is_ok());
        assert!(codex_catalog(&catalog, &[temp.0.join("other-root")]).is_err());
        assert_eq!(fs::read_link(root.join("renamed-alias")).unwrap(), outside);
    }
    #[test]
    fn hermes_snapshot_materializes_aliases_and_enforces_file_and_depth_budgets() {
        let temp = Temporary::new().unwrap();
        let source = temp.0.join("source");
        let external = temp.0.join("external");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&external).unwrap();
        fs::write(
            external.join("SKILL.md"),
            "---\nname: rust-cache-management\n---\n",
        )
        .unwrap();
        std::os::unix::fs::symlink(&external, source.join("alias")).unwrap();
        let destination = temp.0.join("snapshot");
        skill_snapshot(&source, &destination).unwrap();
        assert!(destination.join("alias/SKILL.md").is_file());
        assert!(!fs::symlink_metadata(destination.join("alias"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!fs::symlink_metadata(destination.join("alias/SKILL.md"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(source.join("alias")).unwrap(), external);
        fs::write(
            external.join("SKILL.md"),
            vec![b'x'; MAX_BYTES as usize + 1],
        )
        .unwrap();
        assert!(skill_snapshot(&source, &temp.0.join("oversized"))
            .unwrap_err()
            .contains("oversized"));
        fs::remove_file(source.join("alias")).unwrap();
        fs::create_dir_all(source.join("one/two/three/four")).unwrap();
        assert!(skill_snapshot(&source, &temp.0.join("deep"))
            .unwrap_err()
            .contains("depth budget"));
    }
    #[cfg(unix)]
    #[test]
    fn probe_time_and_output_are_bounded() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30"]);
        let start = Instant::now();
        let result = Probe::start(command, Duration::from_millis(100))
            .unwrap()
            .output();
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(3));
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            &format!(
                "chunk='{}'; while :; do printf '%s' \"$chunk\"; done",
                "x".repeat(32768)
            ),
        ]);
        let result = Probe::start(command, Duration::from_secs(5))
            .unwrap()
            .output();
        let error = result.unwrap_err();
        assert!(error.contains("output exceeded"), "{error}");
    }
    #[test]
    fn stalled_provider_stdin_obeys_deadline() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30"]);
        let start = Instant::now();
        let mut probe = Probe::start(command, Duration::from_millis(100)).unwrap();
        let result = probe.send(json!({"large_request":"x".repeat(1024 * 1024)}));
        assert!(result.unwrap_err().contains("timed out"));
        drop(probe);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn timeout_stops_provider_descendants() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "sleep 30 & echo $!; wait"]);
        let mut probe = Probe::start(command, Duration::from_millis(200)).unwrap();
        while !probe.out.contains(&b'\n') {
            probe.pump().unwrap();
            thread::sleep(Duration::from_millis(5));
        }
        let pid: i32 = std::str::from_utf8(&probe.out)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(probe.output().unwrap_err().contains("timed out"));
        let running = || {
            #[cfg(target_os = "linux")]
            {
                fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| {
                    !s.split_once(") ")
                        .is_some_and(|(_, tail)| tail.starts_with('Z'))
                })
            }
            #[cfg(not(target_os = "linux"))]
            {
                unsafe { libc::kill(pid, 0) == 0 }
            }
        };
        for _ in 0..20 {
            if !running() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(
            !running(),
            "Provider descendant {pid} remained running after timeout"
        );
    }
}
