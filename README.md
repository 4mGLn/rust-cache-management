# rust-cache-management

Version **0.2.0**, release preparation **2026-11-04**.

[Repository](https://github.com/4mGLn/rust-cache-management) · [Releases](https://github.com/4mGLn/rust-cache-management/releases)

A portable agent skill for preserving useful Rust build artifacts, with a standalone Rust diagnostic executable. The executable inspects explicit directories and reports JSON. It does not clean caches, invoke Cargo, change Cargo settings, or require Python or a Rust installation on the machine running a release binary.

## Inspect a verified directory

```sh
rust-cache-inspect --path /absolute/verified/target --max-entries 100000
```

On Windows, use the executable or installed `rust-cache-inspect.cmd` launcher with the same arguments. Repeat `--path` for separately verified target/build directories. The entry budget applies per path and limits work; it is not a cleanup threshold. Do not sum overlapping paths.

The executable does not expand `~` or environment variables; the calling shell must supply the intended path. Relative paths use the current directory. Exit status is `0` for a completed scan within the documented skip policy, `1` for errors or an exhausted scan budget, and `2` for invalid arguments or output failure.

The helper reports bounded logical file size, filesystem space, errors, and skipped entries. Logical size is not allocated or reclaimable space. Symlinks and Windows reparse points are rejected at the selected root and its ancestors, and skipped within the tree. Other-device directories are skipped where the host exposes device identity. Hard links are deduplicated where file identity is available. Check the report's coverage and error fields before interpreting totals. A completed traversal can still contain intentionally skipped entries.

The helper does not resolve effective Cargo settings, shared users, active builds, quotas, or cleanup eligibility. Use stable, trusted directories: concurrent replacement and same-device bind mounts can invalidate scan assumptions. This is diagnosis, not a security boundary or a filesystem snapshot. Selecting the skill or installing the helper never grants permission to delete artifacts.

## Install a release

Release targets are Linux x86_64 and ARM64, macOS Intel and Apple Silicon, and Windows x86_64. Installers reject unsupported platforms. The pinned commands below require the `v0.2.0` release assets to be published; release preparation alone does not establish that downloads or native CI have succeeded.

The default repository is [4mGLn/rust-cache-management](https://github.com/4mGLn/rust-cache-management). Download and review the pinned installer, then run it:

```sh
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  https://raw.githubusercontent.com/4mGLn/rust-cache-management/v0.2.0/install.sh -o install.sh
sh install.sh --version v0.2.0
```

Windows PowerShell:

```powershell
Invoke-WebRequest 'https://raw.githubusercontent.com/4mGLn/rust-cache-management/v0.2.0/install.ps1' -OutFile install.ps1
& ./install.ps1 -Version v0.2.0
```

For an explicitly selected alternative repository, pass `--repo owner/repository` on Unix or `-Repository owner/repository` on Windows. The selected repository is recorded for subsequent updates. Downloading and running an installer trusts that repository and HTTPS transport; review the trust limits below.

Unix defaults to `$HOME/.local/share/rust-cache-management`; Windows defaults to `$env:LOCALAPPDATA\rust-cache-management`. Override with `--prefix` or `-Prefix`. No installer automatically edits PATH, agent instructions, or Cargo configuration. Use the launcher path printed by installation, or add that directory to your own PATH. Unix installation requires curl, tar, and a SHA-256 utility; Windows uses PowerShell and the system curl.exe and tar.exe. These are installation/update dependencies, not a Python runtime requirement.

See [references/distribution.md](references/distribution.md) for updates, rollback, and trust limits. Updates activate new binary/skill payloads; upgrading the stable installer/manager itself requires reviewing a newer installer and installing it into a new prefix.

## Reuse the skill across agents

Keep `SKILL.md`, `references/`, and `AGENTS-snippet.md` together. Run the installed manager's `status` command to find the activated release, and point your agent at its `SKILL.md`; alternatively copy the complete skill into that agent's documented skill location. Discovery and loading vary by tool. This package does not assume a universal agent directory.

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

한국어 요약: Python 없이 실행하는 Rust 진단 도구와 여러 에이전트가 읽을 수 있는 skill입니다. 설치와 업데이트는 캐시를 삭제하거나 Cargo 설정을 바꾸지 않습니다. 자동 업데이트는 기본적으로 꺼져 있으며, 켠 경우의 동작과 복구 방법은 배포 문서에 명시합니다. 기본 저장소는 `4mGLn/rust-cache-management`이며, 고정 설치 명령은 `v0.2.0` 공개 릴리스 게시 후 사용할 수 있습니다.
