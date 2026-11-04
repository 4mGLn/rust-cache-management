# Changelog

## 0.2.2 — 2026-11-04

- Preserve original fixture failures before bounded Windows cleanup retries, restore caller state even after cleanup errors, and report success only after cleanup completes.
- Use streaming .NET SHA-256 in the Windows manager and scope native PowerShell module lookup inside wrappers, preserving legacy bootstrap bytes and restoring caller module paths.
- Keep Windows archive/download native arguments relative to a Unicode-safe working directory; isolate POSIX fixture overrides and retain bounded failure diagnostics for shell portability verification.
- Add explicit agent discovery/setup with user/project preview, consent, structured results, conflict preservation and repeatable managed registration; keep ordinary disk diagnosis read-only. Add isolated native-CI fixtures for packaged setup, compatible shared registration, manual/duplicate handling and wrapper consent/auto-check bypass.
- Forward management `agents ...` directly to the activated binary without rewriting legacy bootstraps or changing the fixed payload schema; document agent/version/alias and project-trust limits.
- Make repeat installation safely validate/reuse or update an intact managed prefix, including legacy 0.2.0 prefixes, while preserving original bootstrap code and user modifications.
- Export regular executable Unix wrappers under `~/.local/bin` for primary `rust-cache-inspect` and secondary help/management command `rust-cache-management`; preserve argument quoting and exit status.
- Add explicit bin-directory options/application environment, collision preflight, missing-PATH guidance, and Windows command aliases without editing shell profiles or persistent PATH.
- Add explicit Unix system mode with root guard and validated persistent readable permissions; never invoke sudo or convert private installs in place.
- Extend isolated HOME, wrapper, repeated-install, baseline, permission-mode and native/live CI fixtures. Native Windows/macOS execution and publication must be reported from actual new run results.

## 0.2.1 — 2026-11-04

- Make the primary Unix and Windows one-line commands resolve the latest stable release without version arguments; retain optional explicit version selection.
- Add process-only `RUST_CACHE_INSTALL_DIR` installation destinations, with explicit prefix options taking precedence and installed managers retaining their own root.
- Reject invalid or ambiguous latest-release redirects and add HTTP/latest-resolution failure fixtures.
- Exercise actual no-version Windows installation from the new source in CI and published latest Unix pipeline/Windows IEX installation after release, with temporary prefixes and read-only diagnostics.

Native CI and post-publication checks for 0.2.1 must be reported from their actual run results.

## 0.2.0 — 2026-11-04

- Replace the optional Python helper with a standalone Rust diagnostic executable for explicitly supplied cache paths.
- Set the confirmed default repository to `4mGLn/rust-cache-management`, retaining explicit installer overrides.
- Add native release packaging for Linux, macOS, and Windows, plus Unix and PowerShell installers.
- Add staged versioned installation, modification checks, explicit updates, opt-in on-invocation update checks, and rollback while retaining previous versions.
- Document same-source checksum trust limits and require signed Git commits and signed annotated release tags.
- Preserve the existing cache-management safety policy and on-demand references.
- License project code, installers, and skill documentation under MIT, copyright 2026 4mGLn; add author/repository metadata and consolidated third-party dependency, Unicode, and Rust 1.97.1 runtime notices for binary distribution.
- Make the non-UTF-8 path fixture filesystem-aware: retain successful inspection coverage where supported, and verify partial/error plus lossy-path diagnostics when macOS rejects the filename with EILSEQ.
- Execute each native release's optimized binary through the installed launcher and a read-only fixture before publishing its asset.

## Initial package — Library version 1

- Initial portable skill package, supplied as Library file version 1, with Markdown guidance and an optional Python diagnostic helper.
