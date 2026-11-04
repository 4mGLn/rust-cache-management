# rust-cache-management

Version **0.2.2**, updated **2026-11-04**.

[Repository](https://github.com/4mGLn/rust-cache-management) · [Releases](https://github.com/4mGLn/rust-cache-management/releases)

## Install for your agent

Repository: **https://github.com/4mGLn/rust-cache-management**. The URL identifies source; it is not permission to execute scripts or change settings. Give your agent that URL and request: “Install this skill for my selected agent and scope. Show the plan first; preserve existing skills and configuration.” Install the release command [below](#install-a-release), then preview an explicit user or project registration:

```sh
rust-cache-management agents detect --json
rust-cache-management agents setup --all-detected --scope user --dry-run --json
rust-cache-management agents setup --agent codex,gemini --scope user --dry-run --json
rust-cache-management agents setup --agent codex --scope project --project /absolute/project --dry-run --json
```

Follow the user’s chosen user/project scope and inspect the preview. After reviewing the exact destinations and conflicts, apply the same request with `--yes` instead of `--dry-run` only when the user has authorized those writes. A URL alone does not authorize `--yes` or remote installer execution. Noninteractive apply requires that explicit consent. On Windows use the printed full `rust-cache-management.cmd` path when it is absent from PATH. Supported agent identifiers are `codex`, `claude`, `gemini`, `opencode`, and `hermes`; compatibility depends on version and configuration. Detection never launches provider CLIs, logs in, grants project trust, or edits agent configuration.

Setup preserves manual installations and refuses ambiguous paths/configuration or conflicting registrations. A conflict in any selected agent stops all selected installations; `--yes` does not override conflicts. Check the JSON `status`, `actions`, and `errors` before expecting a skill to appear. Unrelated dangling discovery links are preserved and reported as warnings; reserved Rust skill paths and unresolved discovery roots still block setup. Skill discovery reads bounded frontmatter rather than imposing that limit on entire skill bodies. Passive discovery leaves versions unchecked until an explicit verification request. JSON reports distinguish the selected paths and results; exit codes are `0` success/preview, `2` invalid arguments, `3` conflict/unsupported state, `4` missing consent, and `1` I/O failure. See [agent setup details](references/distribution.md#agent-skill-setup) for discovery roots, duplicate handling and limitations. Installing files does not guarantee that an agent discovers, selects or follows the skill.

To explicitly check installed versions and native catalog discovery on Unix, run:

```sh
rust-cache-management agents verify --all-detected --scope user --json
# Or verify only the agents you use:
rust-cache-management agents verify --agent codex,claude --scope user --json
```

`detect` and `setup` remain passive: `compatibility` is `not_checked` or `not_detected`, with advice to use `verify`. Verification executes the selected installed commands in private temporary runtime directories without sending model prompts. JSON reports `version`, `version_status`, `catalog_status`, `runtime_scope`, `source_paths` (source discovery roots), and a specific `reason`. A reported version alone never establishes compatibility. `verified` means the native catalog lists this skill through isolated mappings; ordinary session settings, authentication, trust and instruction execution remain unchecked. Hermes checks a bounded private snapshot of skill metadata files because its catalog command writes hub state; `parser_verified` with `isolated_skill_snapshot` proves only snapshot parsing and keeps the overall result `partial`. A missing skill is `not_registered`, rather than a claim that the agent version is incompatible. Unsupported adapters/configuration remain explicitly unverified. Exit `0` with overall `verified` means all selected catalog checks passed; `partial`/exit `3` means at least one remains unverified. Windows verification returns `unsupported_platform` without executing providers; Windows detection and setup retain their existing behavior. See the [verification details](references/distribution.md#explicit-native-verification) for adapters and limits.

A portable agent skill for preserving useful Rust build artifacts, with a standalone Rust diagnostic executable. The default diagnostic command inspects explicit directories and reports read-only JSON. The explicit `agents setup --yes` subcommand installs skill files into the selected agent scope; it is a separate mutation. Neither command cleans caches, invokes Cargo, changes Cargo settings, or requires Python or a Rust installation on the machine running a release binary.

## Inspect a verified directory

```sh
rust-cache-inspect --path /absolute/verified/target --max-entries 100000
```

On Windows, use the executable or installed `rust-cache-inspect.cmd` launcher with the same arguments. Repeat `--path` for separately verified target/build directories. The entry budget applies per path and limits work; it is not a cleanup threshold. Do not sum overlapping paths.

Scan path arguments do not expand `~` or environment variables; the calling shell must supply the intended path. Relative paths use the current directory. Exit status is `0` for a completed scan within the documented skip policy, `1` for errors or an exhausted scan budget, and `2` for invalid arguments or output failure.

The helper reports bounded logical file size, filesystem space, errors, and skipped entries. Logical size is not allocated or reclaimable space. Symlinks and Windows reparse points are rejected at the selected root and its ancestors, and skipped within the tree. Other-device directories are skipped where the host exposes device identity. Hard links are deduplicated where file identity is available. Check the report's coverage and error fields before interpreting totals. A completed traversal can still contain intentionally skipped entries.

The helper does not resolve effective Cargo settings, shared users, active builds, quotas, or cleanup eligibility. Use stable, trusted directories: concurrent replacement and same-device bind mounts can invalidate scan assumptions. This is diagnosis, not a security boundary or a filesystem snapshot. Selecting the skill or installing the helper never grants permission to delete artifacts.

## Install a release

Release targets are Linux x86_64 and ARM64, macOS Intel and Apple Silicon, and Windows x86_64. Installers reject unsupported platforms. Ordinary installation resolves the latest published stable release automatically; no version argument is needed.

Unix (Linux/macOS):

```sh
curl -fsSL https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.ps1 | iex
```

These commands immediately execute remote code. They trust the repository, its maintainers and release workflow, and HTTPS transport. A compromised installer can act before its own checksum checks run. For review before execution, download the latest installer and inspect it, then run it without a version argument:

```sh
curl -fSL https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.sh -o install.sh
# Read install.sh before the next command.
sh install.sh
```

```powershell
Invoke-WebRequest 'https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.ps1' -OutFile install.ps1
# Read install.ps1 before the next command.
& ./install.ps1
```

If GitHub cannot resolve latest, returns an invalid version or an unexpected redirect, installation stops without activating a payload. For advanced reproducibility, `--version v0.2.2` / `-Version v0.2.2` selects a published version explicitly; use its matching reviewed installer too. An alternate repository can be selected with `--repo owner/repository` / `-Repository owner/repository` and is recorded for later updates.

Unix keeps its payload at `$HOME/.local/share/rust-cache-management` and creates executable **regular shell wrappers**, not symlinks, at `$HOME/.local/bin/rust-cache-inspect` and `$HOME/.local/bin/rust-cache-management`. The primary `rust-cache-inspect` wrapper executes the existing prefix launcher with the same arguments and exit status. The secondary command shows help with no arguments or `--help`, and supports `status`, `update`, `rollback`, `auto-on`, `auto-off`, and `inspect`. Help performs no update check.

Re-running the latest installation command validates and reuses an intact managed prefix or advances its payload to latest. This also adds the wrappers to an intact 0.2.0/0.2.1 installation while preserving its original bootstrap manager and diagnostic launcher. Unknown or partial prefixes, repository mismatches, modified managed files, and unrelated/modified command files or symlinks are refused without overwriting them. Existing byte-identical executable wrappers remain unchanged.

Windows defaults to `$env:LOCALAPPDATA\rust-cache-management`, with `rust-cache-inspect.cmd` and `rust-cache-management.cmd` under its `bin` directory. Use the full commands printed by installation unless that directory is already in PATH. There is no persistent PATH or registry edit.

The process-only `RUST_CACHE_INSTALL_DIR` variable selects a different prefix; `--prefix` / `-Prefix` takes precedence. `RUST_CACHE_BIN_DIR` or `--bin-dir` / `-BinDir` selects a separate wrapper directory. On Unix the external bin directory must be outside the managed prefix. For isolated tests, choose a dedicated prefix **and** bin directory. Installers never edit shell profiles, PATH settings, agent instructions, or Cargo configuration; a missing PATH entry produces a warning and full command paths. Release installation alone performs no agent skill registration; use a separately reviewed `agents setup` request.

Unix system-wide installation requires the explicit `--system` option and existing root permissions; it defaults to `/usr/local/share/rust-cache-management` and `/usr/local/bin`. It never invokes sudo. Existing system paths and ancestors must be root-owned and have no group/world write permission; managed files are checked before root updates. No ownership or existing permission settings are changed. Admin-writable macOS `/usr/local` layouts may be refused; select an administrator-controlled root-owned location deliberately rather than weakening the guard. New system files are readable by other users and executable where needed; the validated mode record preserves that behavior on updates. System mode requires a 0.2.2-or-newer payload. System management requires root; diagnosis and help remain readable. An existing private user installation cannot be converted by changing its permissions. Ordinary user installs remain private.

Unix installation requires curl, tar, and a SHA-256 utility; Windows uses PowerShell and system curl.exe and tar.exe. These are installation/update dependencies, not a Python runtime requirement.

See [references/distribution.md](references/distribution.md) for updates, rollback, and trust limits. Updates activate new binary/skill payloads; upgrading the stable installer/manager itself requires reviewing a newer installer and installing it into a new prefix.

## Reuse the skill across agents

For an offline/manual fallback when the release command is unavailable, obtain and review a complete source/release copy and place its skill documents together in the selected agent’s documented skill directory after authorization. Preserve existing files and check for duplicate registrations first; this manual copy is not claimed as updater-managed. Keep `SKILL.md`, `references/`, and `AGENTS-snippet.md` together. Run the installed manager's `status` command to find the activated release, and point your agent at its `SKILL.md`; alternatively copy the complete skill into that agent's documented skill location. Discovery and loading vary by tool. This package does not assume a universal agent directory.

For tools without skill discovery, explicitly ask the agent to read the actual installed `SKILL.md`. Add the short rule from `AGENTS-snippet.md` to the instruction file that tool reads, using the real skill path. Preserve existing instructions. Updates do not merge changes into agent-specific copies; update such copies deliberately after review.

Only discovery metadata needs to be exposed initially in compatible tools. Load `SKILL.md` when relevant and read references on demand. Do not paste every reference into every project's instructions.

Example request:

> Use rust-cache-management to inspect this workspace's cache growth. Start with diagnosis and a cleanup proposal only. Do not delete artifacts, install tools, or change settings.

The policy remains conservative: preserve effective incremental/debug settings, distinguish Cargo's registry cache from workspace artifacts, avoid universal age/size deletion rules, and require verified scope and applicable approval for cleanup. See [references/defaults.md](references/defaults.md) for the nine defaults and their sources. Installation performs no cache cleanup.

## Build and validate from source

```sh
cargo test --locked
cargo build --release --locked
cargo run --release --locked -- --help
```

Source compilation requires Rust and Cargo; running downloaded native binaries does not. Native tests validate the host on which they run. `cargo check --target ...` validates target-specific Rust code without linking or executing it and must not be reported as a working native release binary. CI configuration alone is not evidence that remote jobs or release publication succeeded.

See [CONTRIBUTING.md](CONTRIBUTING.md) for signed commit requirements and [CHANGELOG.md](CHANGELOG.md) for changes. No live cache cleanup is part of the test suite.

## License

Copyright (c) 2026 4mGLn. Project code, installers, and skill documentation are available under the [MIT License](LICENSE). Third-party components retain their own licenses and copyright notices; [THIRD-PARTY-NOTICES](THIRD-PARTY-NOTICES) includes the locked dependency texts, the additional Unicode data notice, and the Rust 1.97.1 standard-library runtime notices. Preserve both files when distributing release binaries. Review the runtime notice snapshot when changing the compiler or release targets.

한국어 요약: Python 없이 실행하는 Rust 진단 도구와 여러 에이전트가 읽을 수 있는 skill입니다. 설치와 업데이트는 캐시를 삭제하거나 Cargo 설정을 바꾸지 않습니다. 자동 업데이트는 기본적으로 꺼져 있으며, 켠 경우의 동작과 복구 방법은 배포 문서에 명시합니다. 기본 저장소는 `4mGLn/rust-cache-management`이며 일반 설치는 버전 인자 없이 최신 공개 릴리스를 선택합니다. 한 줄 설치는 원격 코드를 즉시 실행하므로, 필요하면 먼저 내려받아 검토한 후 실행하세요.
