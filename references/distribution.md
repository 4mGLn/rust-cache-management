# Installation, updates, and recovery

Installers default to [4mGLn/rust-cache-management](https://github.com/4mGLn/rust-cache-management) and download its GitHub release assets. Override deliberately with `--repo owner/repository` or `-Repository owner/repository`; the selected repository is retained for updates. There is no telemetry, hidden service, or scheduled task. Installation never edits project instructions, Cargo settings, or caches.

## Latest installation

The ordinary commands select the latest published stable release automatically, without version arguments:

```sh
curl -fsSL https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.sh | sh
```

```powershell
irm https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.ps1 | iex
```

These commands execute remote code immediately and trust the repository, maintainers, release workflow and HTTPS transport. They do not independently authenticate a publisher. Reviewable alternatives download the same latest script first, let you inspect it, and then run without version arguments:

```sh
curl -fSL https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.sh -o install.sh
# Inspect install.sh, then:
sh install.sh
```

```powershell
Invoke-WebRequest 'https://github.com/4mGLn/rust-cache-management/releases/latest/download/install.ps1' -OutFile install.ps1
# Inspect install.ps1, then:
& ./install.ps1
```

A missing latest release, an HTTP lookup failure, an invalid version or an unexpected/ambiguous redirect stops activation. The final lookup redirect must identify the selected repository's HTTPS GitHub release tag. Advanced reproducibility can use an explicitly reviewed matching-version installer and `--version v0.2.2` / `-Version v0.2.2`; this is optional, not the default installation path.

See the [repository](https://github.com/4mGLn/rust-cache-management) and [release list](https://github.com/4mGLn/rust-cache-management/releases) for publication status. A configured post-publication check is not evidence until it completes.

## Layout and commands

Unix installation defaults to `$HOME/.local/share/rust-cache-management`; Windows defaults to `$env:LOCALAPPDATA\rust-cache-management`. The process-only application variable `RUST_CACHE_INSTALL_DIR` overrides this initial destination, so the one-line commands can install into an explicit private test or user directory. `--prefix /absolute/path` / `-Prefix` takes precedence over that variable. Managers always use their own existing prefix. No HOME/LOCALAPPDATA or persistent environment settings are changed. The prefix is dedicated to this tool. Do not select a shared directory containing unrelated files.

Unix exports regular executable wrappers to `$HOME/.local/bin`: `rust-cache-inspect` executes the stable prefix launcher with quoted arguments and the diagnostic exit status; `rust-cache-management` provides help and management commands. No arguments, `--help`, or `help` shows help without checking for updates. The prefix stays in `.local/share`; no symlinks or profile edits are required. If the chosen bin directory is absent from PATH, use the full paths printed by installation. PATH is never changed persistently.

Select a different command directory through `--bin-dir /absolute/path` / `-BinDir` or process-only `RUST_CACHE_BIN_DIR`; explicit parameters take precedence. Unix requires an external wrapper directory outside the managed prefix. Windows defaults to the internal `PREFIX/bin` and can export additional regular `.ps1`/`.cmd` shims to a separate selected directory. Windows PATH is not changed.

Re-running an installer validates an existing managed prefix, repository, bootstrap baseline and current payload before reusing or updating it. An intact 0.2.0/0.2.1 prefix can gain the new commands without rewriting its bootstrap or inspection launcher. Unknown/partial prefixes and local modifications are preserved and refused. Command collisions are checked before payload activation: unrelated files, symlinks/reparse paths, modified wrappers, and non-executable Unix wrappers are not replaced. Byte-identical owned wrappers are reused. Use a distinct prefix and bin directory for separate test installations.

Unix `--system` is an explicit opt-in requiring root permissions already available; it never invokes sudo. Existing system paths and ancestors must be root-owned and have no group/world write permission; managed files are checked before root updates. No ownership or existing permission settings are changed. Admin-writable macOS `/usr/local` layouts may be refused; select an administrator-controlled root-owned location deliberately rather than weakening the guard. Defaults become `/usr/local/share/rust-cache-management` and `/usr/local/bin`. Only newly created files/directories use the public read/execute modes needed by system users; no unrelated existing files or directories are chmodded. A hashed mode record keeps subsequent system updates and rollback pointers readable. Management requires root, while help/diagnosis can run as ordinary users. Existing user-mode prefixes cannot be converted in place.

Windows prefixes must be local drive-qualified paths such as `C:\Users\name\AppData\Local\rust-cache-management`; drive-relative and UNC prefixes are unsupported. Existing linked/reparse paths are rejected. Windows security policy may block unsigned PowerShell scripts or executables; installers do not alter execution policy or bypass OS controls.

The Windows `.cmd` launcher passes arguments through cmd.exe and PowerShell. For paths or arguments with shell metacharacters that require exact quoting, invoke the activated release's `bin/rust-cache-inspect.exe` directly. This also avoids the PowerShell launcher and its update behavior; normal OS executable policy still applies.

The prefix contains stable launchers under `bin/`, a manager script, versioned payloads under `releases/<version>-<target>/`, and activation/update state. Each payload contains the native diagnostic executable and the complete skill documents. Run `status` to identify the activated release and its `SKILL.md`. Retired versions are retained; disk use can grow as releases accumulate. No automatic pruning is authorized.

Unix:

```sh
PREFIX="$HOME/.local/share/rust-cache-management"
rust-cache-inspect --path /absolute/verified/target
rust-cache-management --help
rust-cache-management status
sh "$PREFIX/manage.sh" status
sh "$PREFIX/manage.sh" update
sh "$PREFIX/manage.sh" rollback
sh "$PREFIX/manage.sh" auto-on
sh "$PREFIX/manage.sh" auto-off
```

Windows PowerShell:

```powershell
$Prefix = Join-Path $env:LOCALAPPDATA 'rust-cache-management'
& "$Prefix/bin/rust-cache-inspect.cmd" --path 'C:\verified\target'
& "$Prefix/manage.ps1" -Action Status
& "$Prefix/manage.ps1" -Action Update
& "$Prefix/manage.ps1" -Action Rollback
& "$Prefix/manage.ps1" -Action AutoOn
& "$Prefix/manage.ps1" -Action AutoOff
```

The ordinary `--path` executable command performs read-only diagnosis. Invoke it directly from its activated release directory to avoid installer/update behavior. Its separate explicit `agents setup --yes` subcommand writes skill files as described below.

## Automatic updates are opt-in

Automatic checks are disabled by default. Enable them explicitly through the manager or initial installer's `--auto-update` / `-AutoUpdate` option. The installed launcher checks for a release when invoked, at most once per day; nothing runs when the launcher is idle. Disable through `auto-off` / `AutoOff`.

An automatic check uses the stored repository as its trust source and may download and activate executable code. Use explicit updates if release review or reproducibility is required. Network latency can delay a launcher invocation. An update failure must be reported without changing the diagnostic executable's eventual exit status; direct invocation avoids all automatic checks.

Updates replace the activated **binary and skill payload**, not the stable bootstrap manager or launchers. Those keep their initially installed code and fixed payload schema. Re-running a newer installer can validate/reuse that installation and add missing command wrappers, but it does not replace that bootstrap code. To upgrade the manager itself, review the newer installer and install it into a **new prefix**, then deliberately switch your launcher/agent paths. A future incompatible payload schema is rejected rather than silently rewriting the manager. Rollback switches payloads; it does not change the bootstrap manager. This also means manager security fixes require that explicit migration.

## Preserve local changes and recover

Updates download into staging, verify the selected asset and allowed payload files, and activate a separate versioned directory only after successful validation. Existing payloads remain available for rollback. The manager checks its installed baseline for local changes and rejects unsafe replacement rather than overwriting modified files. It does not merge user edits into a new release.

If an update reports local modifications, preserve those edits and review them. Keep customized skill copies outside the managed prefix and point agents at the chosen copy. Never delete a modified directory merely to satisfy the updater. Rollback selects a retained, validated version; it does not reverse unrelated changes to your agent configuration or project.

Interrupted downloads, checksum failures, unsupported targets, and unsafe archive paths must stop activation. Inspect any reported staging or lock residue before retrying. These checks do not defend against an administrator or process that can replace installation files concurrently. Use a private, trusted prefix. Symlink/reparse and identity checks have race limits; this is not a sandbox against hostile local filesystem mutation.

## Trust and integrity limits

HTTPS and SHA-256 checks detect corruption and mismatches against the downloaded checksum list. The executable, archive, manifest, checksum list, and installer come from the same GitHub repository/release. **A SHA-256 checksum from that same release is not an independent publisher signature and does not protect against a compromised repository, maintainer account, or release workflow.** A malicious publisher can replace both an asset and its checksum.

Signed source commits and release tags support source provenance when their signer is independently trusted. They do not automatically sign release binaries or prove that a downloaded executable was built from that source. These release assets are not independently code-signed or notarized. Do not claim otherwise. Review source, pin a version, and use separately trusted verification/build processes when your threat model requires stronger guarantees. No private signing key is requested or generated by the installer.

## Platform and verification scope

Release configuration builds Linux GNU x86_64/ARM64, macOS Intel/Apple Silicon, and Windows x86_64 using native CI runners. Linux GNU binaries depend on a compatible system C library; configured runner versions determine that baseline. Other architectures, older libc environments, OS security prompts, and enterprise policies may require a source build or separate packaging.

Local Linux execution tests, cross-target Rust checks, PowerShell syntax parsing, native Windows/macOS execution, CI execution, and published release downloads are distinct evidence. See the accompanying verification report for what was actually run. A configured workflow is not a published or tested release.

## Agent skill setup

The management wrapper forwards `agents ...` to the activated release binary directly, preserving its arguments and exit code and bypassing launcher auto-update checks. The diagnostic binary accepts the same subcommands. Old bootstrap managers remain unchanged; no additional payload file is required. Run the command from a packaged release so setup can find and validate its accompanying skill documents.

```sh
rust-cache-management agents detect --json
rust-cache-management agents setup --agent codex,gemini --scope user --dry-run --json
# After review and explicit consent:
rust-cache-management agents setup --agent codex,gemini --scope user --yes --json
rust-cache-management agents setup --agent claude --scope project --project /absolute/project --dry-run --json
```

`--agent` is an explicit comma-separated or repeated selection; `--all-detected` is a separate explicit opt-in to select detected tools. Setup does not register every detected tool by default. Choose `user` or `project`; project scope requires an explicit absolute project directory. No machine/system agent registration is supported. Preview is read-only, and noninteractive apply requires `--yes`. Missing consent returns `4`, argument errors `2`, conflicts/unsupported states `3`, I/O errors `1`, and successful detect/preview/apply `0`. The versioned JSON envelope has `schema_version: 1`, `command`, `scope`, `dry_run`, `status`, `agents`, `actions`, `errors`, and `warnings`. Overall status is `detected`, `planned`, `consent_required`, `complete`, `conflict`, `io_error`, `invalid`, or `help`; action status is `planned`, `created`, `managed_reuse`, `manual_present`, or `conflict`. `agents --help` and `agents setup --help` return exit `0` with `status: "help"` and a `usage` field; binary help needs neither HOME nor a verified release payload. Agent evidence includes detection, an unverified compatibility warning where version is unknown, and no secret environment values. Read the JSON result and exact paths rather than interpreting CLI/config-directory presence as current use or proven compatibility.

### Discovery roots and aliases

| Agent | User scope | Project scope | Compatibility limits |
| --- | --- | --- | --- |
| Codex | `~/.agents/skills` | `.agents/skills` | Discovery is tool/version dependent; repository discovery was separately observed with CLI 0.166.0. |
| Claude Code | `<CLAUDE_CONFIG_DIR>/skills`, default `~/.claude/skills` | `.claude/skills` | Native Claude root; `.agents` is not assumed to be a Claude source. |
| Gemini CLI | `<effective-home>/.agents/skills` | `.agents/skills` | Shared alias documented in released 0.28.0; catalog tests covered 0.66.0 only. `GEMINI_CLI_HOME` replaces the home root. |
| OpenCode | Shared `.agents/skills` when compatible | `.agents/skills` | Also discovers `.claude` and native roots; existing aliases/configuration can make setup uncertain. |
| Hermes Agent | Selected profile's `<HERMES_HOME>/skills`, default `~/.hermes/skills` | Unsupported by this adapter | Custom source/profile configuration can be unsupported; project trust is never granted. |

These entries describe the supported conservative adapter policy; they are not an all-versions guarantee. In particular, Gemini's `.gemini/skills` and `.agents/skills` must never both receive new copies for the same skill. An existing native/manual registration is preserved; adding a shared alias that creates a duplicate is blocked. OpenCode can see both shared and Claude roots, so sharing is resolved across selected agents before writing. Native flat/manual formats are considered conservatively for collisions; setup does not migrate formats or certify arbitrary custom sources. Existing known OpenCode JSON/JSONC configuration files or a nondefault XDG configuration root require manual review and stop OpenCode setup until effective custom sources can be resolved. A Codex-only request can deliberately layer user and project copies when Gemini usage is not detected; when Gemini is detected, that shared-path duplication is refused. This bounded detection is not proof that every possible alias/provider configuration has been found. Hermes custom source/profile configuration is unsupported when effective discovery cannot be resolved safely; setup does not rewrite YAML or grant trust to make a path discoverable.

Existing registrations are checked using both directory identity and declared skill metadata. A sole visible manual registration is reported as `manual_present` without certifying its version/content or adopting ownership. Matching managed copies are verified for reuse; modified managed files, malformed metadata, and overlapping/ambiguous registrations are preserved and refused. Setup stages its own files and stores ownership/content records and lock state under the selected process home’s `.rust-cache-management-setup` directory, including for project registration. This is this tool’s state, not provider configuration. Preview creates neither the skill copy nor this state. These records support repeatability. A managed copy can be recognized on repeat apply; user edits require review. Independently copied agent skills are not automatically kept in sync with release updates. Do not remove a manual registration merely to satisfy setup.

### Evidence and agent behavior

File installation, parser validation, catalog discovery, model selection and instruction adherence are separate results. Isolated Linux research observed Codex repository discovery (0.166.0), Gemini user catalog/duplicates (0.66.0), Claude parser validation (2.1.318) and Hermes catalog behavior (0.21.3). OpenCode 2.2.5 identity/help was observed, not catalog loading. Those provider tests do not establish Windows/macOS provider discovery or all versions; native release setup fixtures test this tool's file behavior on each runner. Project installation never changes Gemini or Hermes folder-trust decisions, plugin permissions, provider authorization or telemetry settings.

Official references: [Codex skills](https://developers.openai.com/codex/skills), [Claude skills](https://code.claude.com/docs/en/skills), [Gemini skill precedence](https://geminicli.com/docs/cli/skills/#precedence-and-aliases), [Gemini configuration](https://geminicli.com/docs/reference/configuration/), [OpenCode skills](https://opencode.ai/docs/skills/), [OpenCode V2 skills](https://opencode.ai/v2/docs/skills/), [Hermes skills](https://hermes-agent.nousresearch.com/docs/user-guide/features/skills), [Hermes profiles](https://hermes-agent.nousresearch.com/docs/user-guide/profiles).
