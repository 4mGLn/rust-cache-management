use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
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
struct Fixture {
    root: PathBuf,
    home: PathBuf,
    exe: PathBuf,
    project: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rcm-agents-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let home = root.join("private home 한글");
        fs::create_dir(&home).unwrap();
        let project = root.join("project");
        fs::create_dir(&project).unwrap();
        fs::create_dir(project.join(".git")).unwrap();
        let release = root.join("release");
        fs::create_dir_all(release.join("bin")).unwrap();
        fs::create_dir(release.join("references")).unwrap();
        #[cfg(windows)]
        let binary = "bin/rust-cache-inspect.exe";
        #[cfg(not(windows))]
        let binary = "bin/rust-cache-inspect";
        let exe = release.join(binary);
        fs::copy(env!("CARGO_BIN_EXE_rust-cache-inspect"), &exe).unwrap();
        let mut names = TEXT.to_vec();
        names.extend(["install.sh", "install.ps1"]);
        for name in &names {
            let data = if *name == "VERSION" {
                format!("v{}\n", env!("CARGO_PKG_VERSION")).into_bytes()
            } else {
                fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(name)).unwrap()
            };
            fs::write(release.join(name), data).unwrap();
        }
        names.push(binary);
        names.sort();
        let mut manifest = String::new();
        for name in names {
            let bytes = fs::read(release.join(name)).unwrap();
            manifest.push_str(&format!("{:x}  {name}\n", Sha256::digest(bytes)));
        }
        fs::write(release.join("MANIFEST.sha256"), manifest).unwrap();
        Self {
            root,
            home,
            exe,
            project,
        }
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(&self.exe);
        c.env_clear()
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("PATH", "")
            .args(["agents"])
            .args(args);
        c
    }
    fn run(&self, args: &[&str]) -> (i32, Value) {
        let o = self.cmd(args).output().unwrap();
        Self::output(o)
    }
    fn output(o: Output) -> (i32, Value) {
        let json = serde_json::from_slice(&o.stdout).unwrap_or_else(|e| {
            panic!(
                "{e}; stdout={} stderr={}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
        });
        (o.status.code().unwrap(), json)
    }
    fn skill(&self) -> PathBuf {
        self.home.join(".agents/skills/rust-cache-management")
    }
    fn manual(&self, path: &Path) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("SKILL.md"),
            "---\nname: rust-cache-management\ndescription: manual fixture\n---\nPreserve me\n",
        )
        .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn detect_needs_no_payload_and_performs_no_provider_execution() {
    let f = Fixture::new();
    let provider = f.root.join("providers");
    fs::create_dir(&provider).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let p = provider.join("gemini");
        fs::write(&p, "#!/bin/sh\nexit 99\n").unwrap();
        fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
    }
    #[cfg(windows)]
    fs::write(provider.join("gemini.cmd"), "@exit /b 99\n").unwrap();
    fs::remove_file(
        f.exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MANIFEST.sha256"),
    )
    .unwrap();
    let o = f
        .cmd(&["detect", "--json"])
        .env("PATH", provider)
        .output()
        .unwrap();
    let (code, j) = Fixture::output(o);
    assert_eq!(code, 0);
    assert_eq!(j["status"], "detected");
    assert!(j["agents"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["id"] == "gemini" && a["detected"] == true));
    assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
}
#[test]
fn dry_run_and_consent_create_no_home_files() {
    let f = Fixture::new();
    for (args, code, status) in [
        (
            vec!["setup", "--agent", "codex", "--dry-run", "--json"],
            0,
            "planned",
        ),
        (
            vec!["setup", "--agent", "codex", "--json"],
            4,
            "consent_required",
        ),
    ] {
        let (c, j) = f.run(&args);
        assert_eq!(c, code, "{j}");
        assert_eq!(j["status"], status);
        assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
    }
}
#[test]
fn shared_setup_rerun_and_modified_managed_copy() {
    let f = Fixture::new();
    let args = ["setup", "--agent", "codex,gemini", "--yes", "--json"];
    let (c, j) = f.run(&args);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["actions"].as_array().unwrap().len(), 1);
    assert_eq!(j["actions"][0]["status"], "created");
    assert!(f.skill().join("references/policy.md").is_file());
    assert!(!f.home.join(".gemini/skills").exists());
    assert!(!f.skill().join(".baseline").exists());
    let (c, j) = f.run(&args);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["actions"][0]["status"], "managed_reuse");
    fs::write(f.skill().join("references/policy.md"), "user modification").unwrap();
    let (c, j) = f.run(&args);
    assert_eq!(c, 3, "{j}");
    assert_eq!(
        fs::read_to_string(f.skill().join("references/policy.md")).unwrap(),
        "user modification"
    );
}
#[test]
fn renamed_manual_copy_is_preserved_without_adoption() {
    let f = Fixture::new();
    let manual = f.home.join(".agents/skills/manual-folder");
    f.manual(&manual);
    let before = fs::read(manual.join("SKILL.md")).unwrap();
    let (c, j) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["actions"][0]["status"], "manual_present");
    assert_eq!(fs::read(manual.join("SKILL.md")).unwrap(), before);
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}
#[test]
fn duplicate_and_gemini_native_collisions_preserve_files() {
    let f = Fixture::new();
    let manual = f.home.join(".gemini/skills/other-name");
    f.manual(&manual);
    let (c, j) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(c, 3, "{j}");
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
    f.manual(&f.skill());
    let (c, j) = f.run(&["setup", "--agent", "gemini", "--yes", "--json"]);
    assert_eq!(c, 3, "{j}");
    assert!(manual.join("SKILL.md").is_file());
}
#[test]
fn project_scope_and_codex_layering_are_explicit() {
    let f = Fixture::new();
    let (c, j) = f.run(&["setup", "--agent", "codex", "--yes"]);
    assert_eq!(c, 0, "{j}");
    let (c, j) = f.run(&[
        "setup",
        "--agent",
        "codex",
        "--scope",
        "project",
        "--project",
        f.project.to_str().unwrap(),
        "--yes",
        "--json",
    ]);
    assert_eq!(c, 0, "{j}");
    assert!(f
        .project
        .join(".agents/skills/rust-cache-management/SKILL.md")
        .is_file());
}
#[test]
fn different_gemini_home_means_separate_shared_roots() {
    let f = Fixture::new();
    let g = f.root.join("gemini home");
    fs::create_dir(&g).unwrap();
    let o = f
        .cmd(&["setup", "--agent", "codex,gemini", "--yes", "--json"])
        .env("GEMINI_CLI_HOME", &g)
        .output()
        .unwrap();
    let (c, j) = Fixture::output(o);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["actions"].as_array().unwrap().len(), 2);
    assert!(f.skill().exists());
    assert!(g
        .join(".agents/skills/rust-cache-management/SKILL.md")
        .exists());
    assert!(!g.join(".gemini/skills").exists());
}
#[test]
fn custom_claude_and_explicit_hermes_homes_are_used() {
    let f = Fixture::new();
    let c = f.root.join("claude config");
    let h = f.root.join("hermes profile");
    fs::create_dir(&c).unwrap();
    fs::create_dir(&h).unwrap();
    let o = f
        .cmd(&["setup", "--agent", "claude,hermes", "--yes", "--json"])
        .env("CLAUDE_CONFIG_DIR", &c)
        .env("HERMES_HOME", &h)
        .output()
        .unwrap();
    let (code, j) = Fixture::output(o);
    assert_eq!(code, 0, "{j}");
    assert!(c.join("skills/rust-cache-management/SKILL.md").exists());
    assert!(h.join("skills/rust-cache-management/SKILL.md").exists());
}
#[test]
fn source_manifest_tampering_and_extra_members_are_refused() {
    for extra in [false, true] {
        let f = Fixture::new();
        let release = f.exe.parent().unwrap().parent().unwrap();
        if extra {
            fs::write(release.join("unexpected"), "extra").unwrap();
        } else {
            fs::write(release.join("README.md"), "tampered").unwrap();
        }
        let (c, j) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
        assert_eq!(c, 1, "{j}");
        assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
    }
}
#[test]
fn invalid_options_have_json_and_no_writes() {
    let f = Fixture::new();
    for args in [
        vec!["setup", "--agent", "unknown", "--yes", "--json"],
        vec!["setup", "--agent", "codex", "--scope", "system", "--json"],
        vec!["setup", "--agent", "codex", "--scope", "project", "--json"],
    ] {
        let (c, j) = f.run(&args);
        assert_eq!(c, 2, "{j}");
        assert_eq!(j["status"], "invalid");
    }
    assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
}
#[test]
fn unsupported_custom_sources_and_sticky_profile_fail_closed() {
    let f = Fixture::new();
    fs::create_dir_all(f.home.join(".hermes/profiles")).unwrap();
    let (c, j) = f.run(&["setup", "--agent", "hermes", "--yes", "--json"]);
    assert_eq!(c, 3, "{j}");
    let o = f
        .cmd(&["setup", "--agent", "opencode", "--yes", "--json"])
        .env("OPENCODE_CONFIG_CONTENT", "unknown")
        .output()
        .unwrap();
    let (c, j) = Fixture::output(o);
    assert_eq!(c, 3, "{j}");
    assert!(!f.skill().exists());
}
#[test]
fn concurrent_setup_preserves_exactly_one_complete_copy() {
    let f = Fixture::new();
    let args = ["setup", "--agent", "codex", "--yes", "--json"];
    let a = f
        .cmd(&args)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let b = f
        .cmd(&args)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let (ca, ja) = Fixture::output(a.wait_with_output().unwrap());
    let (cb, jb) = Fixture::output(b.wait_with_output().unwrap());
    assert!(ca == 0 || cb == 0, "{ja} {jb}");
    // A competing read-only preflight can observe the exclusive ownership
    // record before atomic directory publication and conservatively refuse.
    assert!(
        [0, 1, 3].contains(&ca) && [0, 1, 3].contains(&cb),
        "{ja} {jb}"
    );
    for (code, json) in [(ca, &ja), (cb, &jb)] {
        if code == 3 {
            assert_eq!(json["status"], "conflict", "{json}");
        }
    }
    let (c, j) = f.run(&args);
    assert_eq!(c, 0, "{j}");
    assert_eq!(j["actions"][0]["status"], "managed_reuse");
}
#[cfg(unix)]
#[test]
fn linked_ancestors_and_discovery_links_are_preserved() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = f.root.join("outside");
    fs::create_dir(&outside).unwrap();
    symlink(&outside, f.home.join(".agents")).unwrap();
    let (c, j) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(c, 3, "{j}");
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn help_works_without_payload_or_home_mutation() {
    let f = Fixture::new();
    fs::remove_file(
        f.exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("MANIFEST.sha256"),
    )
    .unwrap();
    for args in [vec!["--help"], vec!["setup", "--help", "--json"]] {
        let (code, json) = f.run(&args);
        assert_eq!(code, 0, "{json}");
        assert_eq!(json["status"], "help");
        assert!(json["usage"].as_str().unwrap().contains("--all-detected"));
    }
    assert_eq!(fs::read_dir(&f.home).unwrap().count(), 0);
}
#[test]
fn canonical_folder_with_different_name_conflicts() {
    let f = Fixture::new();
    f.manual(&f.skill());
    fs::write(
        f.skill().join("SKILL.md"),
        "---\nname: another-skill\ndescription: existing\n---\n",
    )
    .unwrap();
    let (code, json) = f.run(&["setup", "--agent", "gemini", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(json["errors"][0]
        .as_str()
        .unwrap()
        .contains("different skill name"));
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}
#[test]
fn fresh_claude_and_opencode_share_one_compatible_copy() {
    let f = Fixture::new();
    let (code, json) = f.run(&["setup", "--agent", "claude,opencode", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["actions"].as_array().unwrap().len(), 1);
    assert!(f
        .home
        .join(".claude/skills/rust-cache-management/SKILL.md")
        .exists());
    assert!(!f.skill().exists());
}

#[cfg(unix)]
#[test]
fn unrelated_discovery_symlink_is_read_only_and_does_not_block_setup() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let unrelated = f.root.join("unrelated-skill");
    fs::create_dir(&unrelated).unwrap();
    fs::write(
        unrelated.join("SKILL.md"),
        "---\nname: unrelated-skill\ndescription: unrelated\n---\n",
    )
    .unwrap();
    fs::create_dir_all(f.home.join(".agents/skills")).unwrap();
    symlink(&unrelated, f.home.join(".agents/skills/unrelated-link")).unwrap();
    symlink("absent", unrelated.join("unrelated-asset-link")).unwrap();
    let before = fs::read(unrelated.join("SKILL.md")).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert!(f.skill().is_dir());
    assert_eq!(fs::read(unrelated.join("SKILL.md")).unwrap(), before);
    assert!(unrelated.join("unrelated-asset-link").is_symlink());
}
#[cfg(unix)]
#[test]
fn dangling_unrelated_discovery_links_are_preserved_with_warnings() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let root = f.home.join(".agents/skills");
    fs::create_dir_all(&root).unwrap();
    let link = root.join("paseo-progress");
    let missing = f.root.join("missing-plugin/skills/paseo-progress");
    symlink(&missing, &link).unwrap();
    let warning = |json: &Value| {
        let matches: Vec<_> = json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .filter(|w| w.contains(&*link.to_string_lossy()))
            .collect();
        assert_eq!(matches.len(), 1, "{json}");
        assert!(matches[0].contains(&*missing.to_string_lossy()), "{json}");
    };
    for (extra, expected, status) in [
        (Some("--dry-run"), 0, "planned"),
        (None, 4, "consent_required"),
    ] {
        let mut args = vec!["setup", "--agent", "codex,claude", "--json"];
        args.extend(extra);
        let (code, json) = f.run(&args);
        assert_eq!(code, expected, "{json}");
        assert_eq!(json["status"], status);
        warning(&json);
        assert!(!f.skill().exists());
        assert!(!f.home.join(".claude").exists());
        assert!(!f.home.join(".rust-cache-management-setup").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(fs::read_link(&link).unwrap(), missing);
    }
    for expected_action in ["created", "managed_reuse"] {
        let (code, json) = f.run(&["setup", "--agent", "codex,claude", "--yes", "--json"]);
        assert_eq!(code, 0, "{json}");
        assert_eq!(json["status"], "complete");
        assert!(json["actions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|action| action["status"] == expected_action));
        warning(&json);
        assert_eq!(fs::read_link(&link).unwrap(), missing);
        assert!(!missing.exists());
    }
    assert!(f.skill().join("SKILL.md").is_file());
    assert!(f
        .home
        .join(".claude/skills/rust-cache-management/SKILL.md")
        .is_file());
}

#[test]
fn large_skill_bodies_do_not_exhaust_discovery_metadata_budget() {
    let f = Fixture::new();
    let other = f.home.join(".agents/skills/large-unrelated");
    fs::create_dir_all(&other).unwrap();
    let bytes = format!(
        "---\nname: large-unrelated\ndescription: Valid large skill\n---\n{}",
        "한글 body\n".repeat(20000)
    );
    fs::write(other.join("SKILL.md"), &bytes).unwrap();
    let boundary = f.home.join(".agents/skills/boundary-header");
    fs::create_dir(&boundary).unwrap();
    let prefix = "---\r\nname: boundary-header\r\n# ";
    let suffix = "\r\n---\r\n";
    let boundary_bytes = format!(
        "{prefix}{}{suffix}{}",
        "x".repeat(65536 - prefix.len() - suffix.len()),
        "한글 body\n".repeat(20000)
    );
    fs::write(boundary.join("SKILL.md"), &boundary_bytes).unwrap();
    for extra in ["--dry-run", "--yes"] {
        let (code, json) = f.run(&["setup", "--agent", "codex", extra, "--json"]);
        assert_eq!(code, 0, "{json}");
        assert_eq!(fs::read(other.join("SKILL.md")).unwrap(), bytes.as_bytes());
        assert_eq!(
            fs::read(boundary.join("SKILL.md")).unwrap(),
            boundary_bytes.as_bytes()
        );
        if extra == "--dry-run" {
            assert!(!f.skill().exists());
            assert!(!f.home.join(".rust-cache-management-setup").exists());
        }
    }
    assert!(f.skill().join("SKILL.md").is_file());

    let f = Fixture::new();
    let manual = f.home.join(".agents/skills/renamed-large-skill");
    f.manual(&manual);
    let mut bytes = fs::read(manual.join("SKILL.md")).unwrap();
    bytes.extend("Large manual body\n".repeat(10000).as_bytes());
    fs::write(manual.join("SKILL.md"), &bytes).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["actions"][0]["status"], "manual_present");
    assert_eq!(json["actions"][0]["target"], manual.to_str().unwrap());
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
    let second = f.home.join(".agents/skills/second-large-skill");
    fs::create_dir(&second).unwrap();
    fs::write(second.join("SKILL.md"), &bytes).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert_eq!(fs::read(manual.join("SKILL.md")).unwrap(), bytes);
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}

#[test]
fn invalid_or_oversized_discovery_headers_still_block_setup() {
    let oversized = format!("---\nname: unrelated\n# {}\n---\n", "x".repeat(65536));
    for bytes in [
        oversized.as_bytes(),
        b"---\nname: unrelated\n# Unclosed header\n",
        b"---\nname: unrelated\nname: rust-cache-management\n---\n",
        b"---\nname: unrelated\ndescription: \xff\n---\n",
    ] {
        let f = Fixture::new();
        let other = f.home.join(".agents/skills/unrelated");
        fs::create_dir_all(&other).unwrap();
        let skill = other.join("SKILL.md");
        fs::write(&skill, bytes).unwrap();
        let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
        assert_eq!(code, 3, "{json}");
        assert!(
            json["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e.as_str().unwrap().contains(&*skill.to_string_lossy())),
            "{json}"
        );
        assert_eq!(fs::read(&skill).unwrap(), bytes);
        assert!(!f.skill().exists());
        assert!(!f.home.join(".rust-cache-management-setup").exists());
    }
    let f = Fixture::new();
    fs::create_dir_all(f.home.join(".agents/skills/unrelated/SKILL.md")).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}

#[cfg(unix)]
#[test]
fn dangling_reserved_discovery_links_and_roots_still_conflict() {
    use std::os::unix::fs::symlink;
    for relative in [
        ".agents/skills/rust-cache-management",
        ".agents/skills/rust-cache-management.md",
        ".agents/skills",
        ".agents/skills/other-skill/SKILL.md",
    ] {
        let f = Fixture::new();
        let link = f.home.join(relative);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        let missing = f.root.join("missing-registration");
        symlink(&missing, &link).unwrap();
        let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
        assert_eq!(code, 3, "{json}");
        assert_eq!(json["status"], "conflict");
        assert!(
            json["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e.as_str().unwrap().contains(&*link.to_string_lossy())),
            "{json}"
        );
        assert!(!f.home.join(".rust-cache-management-setup").exists());
        assert_eq!(fs::read_link(&link).unwrap(), missing);
        assert!(!missing.exists());
    }
}

#[cfg(unix)]
#[test]
fn dangling_discovery_warning_survives_other_conflicts_and_cycles_are_refused() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let root = f.home.join(".agents/skills");
    fs::create_dir_all(&root).unwrap();
    let link = root.join("unrelated");
    let missing = f.root.join("missing-plugin");
    symlink(&missing, &link).unwrap();
    fs::create_dir_all(f.home.join(".config/opencode")).unwrap();
    fs::write(f.home.join(".config/opencode/opencode.json"), "{}").unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex,opencode", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(json["actions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|action| action["detail"].as_str().unwrap().contains("manual review")));
    assert!(json["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains(&*link.to_string_lossy())));
    assert!(!f.home.join(".rust-cache-management-setup").exists());
    assert!(!f.skill().exists());
    assert_eq!(fs::read_link(&link).unwrap(), missing);

    let cycle = root.join("loop");
    symlink(&cycle, &cycle).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e.as_str().unwrap().contains(&*cycle.to_string_lossy())));
    assert_eq!(fs::read_link(&cycle).unwrap(), cycle);
    assert!(!f.home.join(".rust-cache-management-setup").exists());
    assert!(!f.skill().exists());
}

#[cfg(unix)]
#[test]
fn two_discovery_aliases_to_same_rust_skill_are_still_duplicates() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let manual = f.root.join("manual-rust");
    f.manual(&manual);
    fs::create_dir_all(f.home.join(".agents/skills")).unwrap();
    fs::create_dir_all(f.home.join(".gemini/skills")).unwrap();
    symlink(&manual, f.home.join(".agents/skills/alias-a")).unwrap();
    symlink(&manual, f.home.join(".gemini/skills/alias-b")).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "gemini", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}

#[test]
fn same_lexical_root_from_user_and_non_git_ancestor_is_not_a_duplicate() {
    let f = Fixture::new();
    f.manual(&f.skill());
    let project = f.home.join("plain-project");
    fs::create_dir(&project).unwrap();
    let (code, json) = f.run(&[
        "setup",
        "--agent",
        "gemini",
        "--scope",
        "user",
        "--project",
        project.to_str().unwrap(),
        "--yes",
        "--json",
    ]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["actions"][0]["status"], "manual_present");
    assert!(!project.join(".agents").exists());
}

#[test]
fn opencode_flat_markdown_registrations_are_preserved_without_duplicates() {
    for (root, selected, expected) in [
        (".config/opencode/skills", "opencode", 0),
        (".agents/skills", "codex", 3),
    ] {
        let f = Fixture::new();
        let dir = f.home.join(root);
        fs::create_dir_all(&dir).unwrap();
        let flat = dir.join("rust-cache-management.md");
        let bytes = b"# Existing manual OpenCode skill\nPreserve this filename ID.\n";
        fs::write(&flat, bytes).unwrap();
        let before = fs::metadata(&flat).unwrap().modified().unwrap();
        let (code, json) = f.run(&["setup", "--agent", selected, "--yes", "--json"]);
        assert_eq!(code, expected, "{json}");
        if expected == 0 {
            assert_eq!(json["actions"][0]["status"], "manual_present");
            assert_eq!(json["actions"][0]["target"], flat.to_str().unwrap());
        }
        assert_eq!(fs::read(&flat).unwrap(), bytes);
        assert_eq!(fs::metadata(&flat).unwrap().modified().unwrap(), before);
        assert!(!f.skill().exists());
        assert!(!f.home.join(".rust-cache-management-setup").exists());
    }
    let f = Fixture::new();
    for root in [".config/opencode/skills", ".agents/skills"] {
        let dir = f.home.join(root);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("rust-cache-management.md"), "manual\n").unwrap();
    }
    let (code, json) = f.run(&["setup", "--agent", "opencode", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert!(!f.skill().exists());
    assert!(!f.home.join(".rust-cache-management-setup").exists());
}

#[test]
fn opencode_file_configuration_and_custom_xdg_require_manual_review() {
    for relative in [
        ".config/opencode/opencode.json",
        ".config/opencode/opencode.jsonc",
        "workspace/opencode.json",
        "workspace/opencode.jsonc",
        "workspace/.opencode/opencode.json",
        "workspace/.opencode/opencode.jsonc",
    ] {
        let f = Fixture::new();
        let config = f.home.join(relative);
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        let bytes = b"{\"skills\":[\"/unresolved/external-skills\"]}\n";
        fs::write(&config, bytes).unwrap();
        let project = f.home.join("workspace/nested");
        fs::create_dir_all(&project).unwrap();
        // Both the project root and a parent source are checked read-only.
        for path in [config.parent().unwrap(), project.as_path()] {
            let (code, json) = f.run(&[
                "setup",
                "--agent",
                "opencode",
                "--project",
                path.to_str().unwrap(),
                "--yes",
                "--json",
            ]);
            assert_eq!(code, 3, "{relative}: {json}");
            assert!(json["actions"][0]["detail"]
                .as_str()
                .unwrap()
                .contains("manual review"));
            assert_eq!(fs::read(&config).unwrap(), bytes);
            assert!(!f.skill().exists());
            assert!(!f.home.join(".rust-cache-management-setup").exists());
        }
    }
    let f = Fixture::new();
    let output = f
        .cmd(&["setup", "--agent", "opencode", "--yes", "--json"])
        .env("XDG_CONFIG_HOME", f.root.join("other-xdg"))
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 3, "{json}");
    assert!(!f.skill().exists());
    let output = f
        .cmd(&["setup", "--agent", "opencode", "--dry-run", "--json"])
        .env("XDG_CONFIG_HOME", f.home.join(".config"))
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 0, "{json}");
}

#[test]
fn all_five_agents_share_only_compatible_identical_managed_aliases() {
    let f = Fixture::new();
    let args = [
        "setup",
        "--agent",
        "codex,claude,gemini,opencode,hermes",
        "--yes",
        "--json",
    ];
    let (code, json) = f.run(&args);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["actions"].as_array().unwrap().len(), 3);
    for root in [".agents", ".claude", ".hermes"] {
        let skill = f.home.join(root).join("skills/rust-cache-management");
        assert_eq!(
            fs::read(skill.join("SKILL.md")).unwrap(),
            fs::read(f.skill().join("SKILL.md")).unwrap()
        );
    }
    assert!(json["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains(".agents over .claude")));
    let (code, json) = f.run(&args);
    assert_eq!(code, 0, "{json}");
    assert!(json["actions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["status"] == "managed_reuse"));
}

#[test]
fn sequential_codex_and_claude_owned_copies_coexist_in_either_order() {
    for agents in [["codex", "claude"], ["claude", "codex"]] {
        let f = Fixture::new();
        for id in agents {
            let (code, json) = f.run(&["setup", "--agent", id, "--yes", "--json"]);
            assert_eq!(code, 0, "{id}: {json}");
        }
        for id in agents {
            let (code, json) = f.run(&["setup", "--agent", id, "--yes", "--json"]);
            assert_eq!(code, 0, "{id}: {json}");
            assert_eq!(json["actions"][0]["status"], "managed_reuse");
        }
    }
}

#[test]
fn manual_or_modified_opencode_aliases_still_block_new_provider_copy() {
    for (manual, selected) in [(".claude", "codex"), (".agents", "claude")] {
        let f = Fixture::new();
        let path = f.home.join(manual).join("skills/rust-cache-management");
        f.manual(&path);
        let before = fs::read(path.join("SKILL.md")).unwrap();
        let (code, json) = f.run(&["setup", "--agent", selected, "--yes", "--json"]);
        assert_eq!(code, 3, "{json}");
        assert_eq!(fs::read(path.join("SKILL.md")).unwrap(), before);
        assert!(!f.home.join(".rust-cache-management-setup").exists());
    }
    let f = Fixture::new();
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    fs::write(f.skill().join("README.md"), "user modifications\n").unwrap();
    let (code, json) = f.run(&["setup", "--agent", "claude", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert_eq!(
        fs::read(f.skill().join("README.md")).unwrap(),
        b"user modifications\n"
    );
    assert!(!f.home.join(".claude").exists());
}

#[test]
fn identical_owned_gemini_aliases_remain_a_conflict() {
    let f = Fixture::new();
    let (code, json) = f.run(&["setup", "--agent", "gemini", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    let native = f.home.join(".gemini/skills/rust-cache-management");
    fs::create_dir_all(native.join("references")).unwrap();
    for name in TEXT {
        fs::copy(f.skill().join(name), native.join(name)).unwrap();
    }
    let record_path = |path: &Path| {
        f.home.join(".rust-cache-management-setup").join(format!(
            "{:x}.json",
            Sha256::digest(path.to_string_lossy().as_bytes())
        ))
    };
    let mut record: Value =
        serde_json::from_slice(&fs::read(record_path(&f.skill())).unwrap()).unwrap();
    record["target"] = serde_json::to_value(&native).unwrap();
    fs::write(record_path(&native), serde_json::to_vec(&record).unwrap()).unwrap();
    let before = fs::read(native.join("SKILL.md")).unwrap();
    let (code, json) = f.run(&["setup", "--agent", "gemini", "--yes", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert_eq!(fs::read(native.join("SKILL.md")).unwrap(), before);
    assert_eq!(fs::read(f.skill().join("SKILL.md")).unwrap(), before);
}

#[test]
fn verify_configuration_only_detection_is_not_compatibility_proof() {
    let f = Fixture::new();
    fs::create_dir(f.home.join(".codex")).unwrap();
    let (code, json) = f.run(&["detect", "--json"]);
    assert_eq!(code, 0);
    let codex = &json["agents"][0];
    assert_eq!(codex["compatibility"], "not_checked");
    assert_eq!(codex["version_status"], "not_checked");
    let (code, json) = f.run(&["verify", "--agent", "codex", "--json"]);
    assert_eq!(code, 3, "{json}");
    assert_eq!(json["status"], "partial");
    assert_eq!(json["agents"][0]["compatibility"], "unverified");
    #[cfg(unix)]
    assert_eq!(json["agents"][0]["version_status"], "not_available");
    #[cfg(not(unix))]
    assert_eq!(json["agents"][0]["catalog_status"], "unsupported_platform");
    assert_eq!(fs::read_dir(&f.home).unwrap().count(), 1);
    let empty = Fixture::new();
    let (code, json) = empty.run(&["verify", "--all-detected", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["status"], "complete");
    assert_ne!(json["status"], "verified");
}

#[cfg(unix)]
#[test]
fn explicit_verify_executes_only_catalog_protocol_and_default_commands_stay_passive() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let providers = f.root.join("providers");
    fs::create_dir(&providers).unwrap();
    let script = providers.join("codex");
    let calls = f.root.join("provider-calls");
    let runtime_home = f.root.join("runtime-home");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    let catalog = serde_json::json!({"id":2,"result":{"data":[{"skills":[{"name":"rust-cache-management","path":f.skill().join("SKILL.md"),"enabled":true}],"errors":[{"message":"unrelated malformed skill"}]}]}});
    let text = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {calls}\nif [ \"$1\" = --version ]; then printf 'codex-cli 9.8.7\\n'; exit 0; fi\nprintf '%s' \"$HOME\" > {home}\n[ -z \"${{OPENAI_API_KEY:-}}\" ] || exit 90\n[ -z \"${{ANTHROPIC_API_KEY:-}}\" ] || exit 91\nIFS= read -r request\nprintf '%s\\n' \"$request\" >> {calls}\nprintf '%s\\n' '{{\"id\":1,\"result\":{{}}}}'\nIFS= read -r request\nprintf '%s\\n' \"$request\" >> {calls}\nIFS= read -r request\nprintf '%s\\n' \"$request\" >> {calls}\nprintf '%s\\n' {catalog}\nwhile IFS= read -r request; do printf '%s\\n' \"$request\" >> {calls}; done\n",
        calls=quote(calls.to_str().unwrap()), home=quote(runtime_home.to_str().unwrap()), catalog=quote(&catalog.to_string())
    );
    fs::write(&script, text).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    for args in [
        vec!["detect", "--json"],
        vec!["setup", "--agent", "codex", "--dry-run", "--json"],
    ] {
        let output = f.cmd(&args).env("PATH", &providers).output().unwrap();
        let (code, json) = Fixture::output(output);
        assert_eq!(code, 0, "{json}");
        assert!(!calls.exists());
    }
    let (code, json) = f.run(&["setup", "--agent", "codex", "--yes", "--json"]);
    assert_eq!(code, 0, "{json}");
    assert!(!calls.exists());
    let before = fs::read(f.skill().join("SKILL.md")).unwrap();
    let output = f
        .cmd(&["verify", "--agent", "codex", "--json"])
        .env("PATH", &providers)
        .env("OPENAI_API_KEY", "must-not-leak")
        .env("ANTHROPIC_API_KEY", "must-not-leak")
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 0, "{json}");
    assert_eq!(json["status"], "verified");
    assert_eq!(json["agents"][0]["version"], "9.8.7");
    assert_eq!(json["agents"][0]["catalog_status"], "enabled");
    assert_eq!(
        json["agents"][0]["runtime_scope"],
        "isolated_native_catalog"
    );
    assert!(json["agents"][0]["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("unrelated")));
    let requests = fs::read_to_string(&calls).unwrap();
    assert!(requests.contains("skills/list"));
    assert!(!requests.contains("turn/start"));
    assert!(!requests.contains("thread/start"));
    let private = PathBuf::from(fs::read_to_string(&runtime_home).unwrap());
    assert_ne!(private, f.home);
    assert!(
        !private.exists(),
        "Private provider runtime was not removed"
    );
    assert_eq!(fs::read(f.skill().join("SKILL.md")).unwrap(), before);

    use std::os::unix::ffi::OsStringExt;
    let invalid_home = f
        .root
        .join(std::ffi::OsString::from_vec(b"invalid-home-\xff".to_vec()));
    fs::create_dir_all(invalid_home.join("skills")).unwrap();
    let output = f
        .cmd(&["verify", "--agent", "codex", "--json"])
        .env("PATH", &providers)
        .env("CODEX_HOME", &invalid_home)
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 3, "{json}");
    assert_eq!(json["agents"][0]["version"], "9.8.7");
    assert_eq!(
        json["agents"][0]["catalog_status"],
        "unsupported_configuration"
    );

    fs::write(
        &script,
        "#!/bin/sh\nif [ \"$1\" = --version ]; then printf 'codex-cli 9.8.7\\n'; fi\n",
    )
    .unwrap();
    let output = f
        .cmd(&["verify", "--agent", "codex", "--json"])
        .env("PATH", &providers)
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 3, "{json}");
    assert_eq!(json["agents"][0]["version_status"], "reported");
    assert_eq!(json["agents"][0]["compatibility"], "unverified");
    assert_eq!(json["agents"][0]["catalog_status"], "failed");
}

#[cfg(unix)]
#[test]
fn hermes_verification_snapshots_metadata_without_source_hub_writes() {
    use std::os::unix::fs::PermissionsExt;
    fn snapshot(root: &Path) -> Vec<(PathBuf, String)> {
        fn walk(root: &Path, folder: &Path, result: &mut Vec<(PathBuf, String)>) {
            for entry in fs::read_dir(folder).unwrap() {
                let path = entry.unwrap().path();
                let kind = fs::symlink_metadata(&path).unwrap();
                let value = if kind.is_dir() {
                    "directory".into()
                } else if kind.file_type().is_symlink() {
                    format!("link:{}", fs::read_link(&path).unwrap().display())
                } else {
                    format!("{:x}", Sha256::digest(fs::read(&path).unwrap()))
                };
                result.push((path.strip_prefix(root).unwrap().to_owned(), value));
                if kind.is_dir() {
                    walk(root, &path, result);
                }
            }
        }
        let mut result = Vec::new();
        walk(root, root, &mut result);
        result.sort();
        result
    }
    let f = Fixture::new();
    let source = f.home.join(".hermes/skills");
    let skill = source.join("group/rust-cache-management");
    f.manual(&skill);
    fs::write(source.join("user-notes"), "preserve supporting files").unwrap();
    let before = snapshot(&source);
    let providers = f.root.join("providers");
    fs::create_dir(&providers).unwrap();
    let script = providers.join("hermes");
    let private_root = f.root.join("private-hermes-root");
    let calls = f.root.join("hermes-calls");
    let quote = |s: &str| format!("'{}'", s.replace('\'', "'\\''"));
    fs::write(&script, format!(
        "#!/bin/sh\n[ \"$PYTHONDONTWRITEBYTECODE\" = 1 ] || exit 91\nprintf '%s\\n' \"$*\" >> {calls}\nif [ \"$1\" = --version ]; then printf 'Hermes Agent v0.21.3 (2026.9.14) upstream hash\\n'; exit 0; fi\n/bin/mkdir -p \"$HERMES_HOME/skills/.hub\"\nprintf 'native hub write' > \"$HERMES_HOME/skills/.hub/audit.log\"\nprintf '%s' \"$HERMES_HOME\" > {private}\n[ -f \"$HERMES_HOME/skills/group/rust-cache-management/SKILL.md\" ] || exit 92\n[ ! -L \"$HERMES_HOME/skills\" ] || exit 93\n[ ! -L \"$HERMES_HOME/skills/group/rust-cache-management/SKILL.md\" ] || exit 94\nprintf '│ rust-cache-management │ │ local │ local │ enabled │\\n'\n",
        calls=quote(calls.to_str().unwrap()), private=quote(private_root.to_str().unwrap())
    )).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
    let output = f
        .cmd(&["verify", "--agent", "hermes", "--json"])
        .env("PATH", &providers)
        .output()
        .unwrap();
    let (code, json) = Fixture::output(output);
    assert_eq!(code, 3, "{json}");
    assert_eq!(json["status"], "partial");
    let hermes = json["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "hermes")
        .unwrap();
    assert_eq!(hermes["version_status"], "reported");
    assert_eq!(hermes["version"], "0.21.3");
    assert_eq!(hermes["catalog_status"], "enabled");
    assert_eq!(hermes["compatibility"], "parser_verified");
    assert_eq!(hermes["runtime_scope"], "isolated_skill_snapshot");
    assert_eq!(snapshot(&source), before);
    assert!(!source.join(".hub").exists());
    let private = PathBuf::from(fs::read_to_string(private_root).unwrap());
    assert!(
        !private.exists(),
        "Private snapshot/hub runtime was not removed"
    );
    let calls = fs::read_to_string(calls).unwrap();
    assert!(calls.contains("--version"));
    assert!(calls.contains("skills list --source local --enabled-only"));
}
