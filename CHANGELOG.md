# Changelog

## 0.2.0 — 2026-11-04 (release preparation)

- Replace the optional Python helper with a standalone Rust diagnostic executable for explicitly supplied cache paths.
- Set the confirmed default repository to `4mGLn/rust-cache-management`, retaining explicit installer overrides.
- Add native release packaging for Linux, macOS, and Windows, plus Unix and PowerShell installers.
- Add staged versioned installation, modification checks, explicit updates, opt-in on-invocation update checks, and rollback while retaining previous versions.
- Document same-source checksum trust limits and require signed Git commits and signed annotated release tags.
- Preserve the existing cache-management safety policy and on-demand references.
- License project code, installers, and skill documentation under MIT, copyright 2026 4mGLn; add author/repository metadata and consolidated third-party dependency, Unicode, and Rust 1.97.1 runtime notices for binary distribution.
- Make the non-UTF-8 path fixture filesystem-aware: retain successful inspection coverage where supported, and verify partial/error plus lossy-path diagnostics when macOS rejects the filename with EILSEQ.

Release preparation does not claim successful native CI or publication; report those results separately when they run.

## Initial package — Library version 1

- Initial portable skill package, supplied as Library file version 1, with Markdown guidance and an optional Python diagnostic helper.
