# Installation, updates, and recovery

Installers default to [4mGLn/rust-cache-management](https://github.com/4mGLn/rust-cache-management) and download its GitHub release assets. Override deliberately with `--repo owner/repository` or `-Repository owner/repository`; the selected repository is retained for updates. There is no telemetry, hidden service, or scheduled task. Installation never edits project instructions, Cargo settings, or caches.

## Pinned installation

After the `v0.2.0` release assets are published, download and review the tagged installer, then run it:

```sh
curl --fail --show-error --location --proto '=https' --proto-redir '=https' \
  https://raw.githubusercontent.com/4mGLn/rust-cache-management/v0.2.0/install.sh -o install.sh
sh install.sh --version v0.2.0
```

```powershell
Invoke-WebRequest 'https://raw.githubusercontent.com/4mGLn/rust-cache-management/v0.2.0/install.ps1' -OutFile install.ps1
& ./install.ps1 -Version v0.2.0
```

See the [repository](https://github.com/4mGLn/rust-cache-management) and [release list](https://github.com/4mGLn/rust-cache-management/releases) for publication status. These commands do not imply that the prepared release or native CI has already completed.

## Layout and commands

Unix installation defaults to `$HOME/.local/share/rust-cache-management`; use `--prefix /absolute/path` to override. Windows defaults to `$env:LOCALAPPDATA\rust-cache-management`; use `-Prefix` to override. The prefix is dedicated to this tool. Do not select a shared directory containing unrelated files.

Windows prefixes must be local drive-qualified paths such as `C:\Users\name\AppData\Local\rust-cache-management`; drive-relative and UNC prefixes are unsupported. Existing linked/reparse paths are rejected. Windows security policy may block unsigned PowerShell scripts or executables; installers do not alter execution policy or bypass OS controls.

The Windows `.cmd` launcher passes arguments through cmd.exe and PowerShell. For paths or arguments with shell metacharacters that require exact quoting, invoke the activated release's `bin/rust-cache-inspect.exe` directly. This also avoids the PowerShell launcher and its update behavior; normal OS executable policy still applies.

The prefix contains stable launchers under `bin/`, a manager script, versioned payloads under `releases/<version>-<target>/`, and activation/update state. Each payload contains the native diagnostic executable and the complete skill documents. Run `status` to identify the activated release and its `SKILL.md`. Retired versions are retained; disk use can grow as releases accumulate. No automatic pruning is authorized.

Unix:

```sh
PREFIX="$HOME/.local/share/rust-cache-management"
"$PREFIX/bin/rust-cache-inspect" --path /absolute/verified/target
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

The release executable itself performs diagnosis only. Invoke it directly from its activated release directory if you need an operation with no installer/update behavior.

## Automatic updates are opt-in

Automatic checks are disabled by default. Enable them explicitly through the manager or initial installer's `--auto-update` / `-AutoUpdate` option. The installed launcher checks for a release when invoked, at most once per day; nothing runs when the launcher is idle. Disable through `auto-off` / `AutoOff`.

An automatic check uses the stored repository as its trust source and may download and activate executable code. Use explicit updates if release review or reproducibility is required. Network latency can delay a launcher invocation. An update failure must be reported without changing the diagnostic executable's eventual exit status; direct invocation avoids all automatic checks.

Updates replace the activated **binary and skill payload**, not the stable bootstrap manager or launchers. Those keep their initially installed code and fixed payload schema. To upgrade the manager itself, review the newer installer and install it into a **new prefix**, then deliberately switch your launcher/agent paths. A future incompatible payload schema is rejected rather than silently rewriting the manager. Rollback switches payloads; it does not change the bootstrap manager. This also means manager security fixes require that explicit migration.

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
