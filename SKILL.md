---
name: rust-cache-management
description: Diagnose Rust build-cache growth and plan safe selective cleanup while preserving Cargo configuration and reusable artifacts. Use for Rust disk pressure, cache cleanup, or repeated-build storage concerns.
---

# Rust Build and Cache Management

Preserve correctness, valid artifacts, and stable configuration before optimizing disk usage. This skill grants no permission to delete files, install tools, or change settings.

## Ordinary development

- Respect existing Cargo settings and reuse build directories; do not force incremental compilation off or override debug settings by default.
- Prefer `cargo check` for quick feedback, but run project-required builds and tests before claiming completion.
- Never automatically perform a full clean, delete entire target/build directories or Cargo home, or remove unrelated caches. A verified non-deleting preview is allowed; scoped deletion still requires the applicable approval.

## Disk pressure or cleanup

Read [references/defaults.md](references/defaults.md) for the nine recommended defaults, evidence, and exceptions before choosing settings or cleanup methods. Read [references/policy.md](references/policy.md) for the full safety policy. Read [references/inspection.md](references/inspection.md) when determining paths, concurrency, or disk usage.

1. Identify the workspace, actual artifact paths, shared users, free space, and growth. Do not assume `./target` is the only cache. Optionally run the standalone Rust helper, `rust-cache-inspect --path /absolute/verified/target`, on explicitly verified paths; it neither resolves Cargo settings nor certifies cleanup safety. See [README.md](README.md) for installation and diagnostic limits. Do not enable updates or install anything without applicable authorization.
2. Separate read-only diagnosis from mutation. Produce a plan with exact paths, selection criteria, expected benefit, rebuild cost, and uncertainty.
3. Verify that the proposed method supports the installed Cargo version and layout. Preview candidates without deleting. Age alone is insufficient.
4. Coordinate all users of affected directories. A process snapshot is not a lock; if concurrent work cannot be excluded safely, stop cleanup.
5. Obtain required approval for the exact operation. Apply only the approved scope, recheck safety immediately beforehand, and stop if conditions change.
6. Report observed space reclaimed and validation actually performed. Never describe `cargo check` as successful linking or test execution.

If paths, ownership, permissions, tool behavior, or concurrency remain uncertain, leave artifacts untouched and report the blocker. Do not install cleanup tools automatically.

## Installing this skill for an agent

When the user explicitly asks to register this skill, read [references/distribution.md](references/distribution.md#agent-skill-setup) and use the packaged command's `agents detect` / `agents setup --dry-run` to preview selected agents, scope, paths and conflicts. Apply only the reviewed selection with explicit consent (`--yes` for noninteractive use). Preserve existing manual skills and configuration. Never grant project trust, edit provider settings, or install every detected agent by default. File registration is not proof of discovery or instruction adherence. This setup operation is separate from read-only cache diagnosis and grants no permission to delete cache artifacts.
