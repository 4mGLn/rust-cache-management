## Rust Build Cache

Preserve Cargo settings and reusable artifacts. Never automatically perform a full clean or delete entire target/build directories, Cargo home, or unrelated caches. For Rust disk pressure or cleanup, use the `rust-cache-management` skill; if discovery is unavailable, read `docs/agent-skills/rust-cache-management/SKILL.md` at its actual installed path. If unavailable, diagnose only and do not clean. Run project-required builds and tests before completion.

Skill registration is a separate requested mutation: preview explicit agents and user/project scope with `rust-cache-management agents setup --agent codex --scope user --dry-run`; apply only after review and explicit consent. Do not edit agent configuration, grant project trust, or overwrite manual skills merely to enable discovery.
