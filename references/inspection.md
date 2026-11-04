# Read-only inspection and cleanup planning

Use only the sections needed. This guide contains no deletion commands. The optional standalone Rust helper, `rust-cache-inspect --path /absolute/verified/target`, accepts explicit verified paths only and outputs read-only JSON diagnostics; see [../README.md](../README.md) for usage and limitations. It requires no Python runtime. Updating the installed tool is a separate operation and is not part of the diagnostic executable.

## Resolve paths before measuring

- Inspect the workspace root, applicable `.cargo/config.toml` or legacy config, relevant user Cargo config, command-line overrides, and `CARGO_TARGET_DIR` / `CARGO_BUILD_BUILD_DIR` when supported by the installed Cargo version. Do not dump the entire environment or credential files.
- Cargo configuration discovery depends on the invocation directory. Record that directory and resolve relative settings according to the installed version's documentation.
- `cargo metadata --format-version 1 --no-deps` can report `workspace_root` and `target_directory`. Use offline/locked options where appropriate; metadata can fail or affect dependency resolution/lockfiles, so it is not a guarantee of a filesystem-only probe. Prefer existing configuration inspection first when no writes are permitted.
- Check separately configured intermediate build directories, symlinks/junctions, external mounts, and shared ownership. Inspect, do not traverse unrelated caches.

## Measure using the host OS

Replace placeholders with verified paths; quote paths. Avoid recursive scans of an entire home directory.

- Linux: `df -h "/verified/path"` for filesystem capacity; `du -sh "/verified/path"` for directory usage. GNU `du --max-depth` is optional and not portable.
- macOS/BSD: use supported `df` and `du` flags; do not assume GNU options exist.
- Windows: use the host's storage tools or PowerShell for the verified volume/path. Recursive file-size totals measure logical bytes and may not equal allocated disk space, particularly with sparse files, compression, hard links, or inaccessible entries. Skip reparse points and report skipped entries.

Large scans can be expensive. Start with the relevant artifact directories and expand only if needed.

## Plan rather than guess

For each candidate, record:
- Canonical path and project/shared ownership.
- Evidence that it is not needed for active work; age is only supporting evidence.
- Exact tool/version, selection scope, preview result, and expected rebuild cost.
- Method of coordinating concurrent users and the required approval.

Do not call a cleanup command with a supposed preview flag until its installed help or authoritative documentation confirms that flag. Do not substitute arbitrary recursive deletion when a tool is unavailable.

A move to quarantine on the same filesystem generally does not reclaim space. Do not call it successful disk cleanup. Recovery from deleting build artifacts usually means rebuilding, which can require time, toolchains, network access, and dependency availability.

## Minimal result

Report: paths examined, disk observations, findings, proposed/approved actions, blockers, and build/test results actually obtained. If no cleanup is necessary, stop without deleting anything.
