# Rust Build and Cache Management Policy

This reference retains the supplied policy with researched clarifications. Read [defaults.md](defaults.md) for the nine recommended defaults, documented Cargo behavior, and explicit exceptions. Sources were checked on 2026-11-04; recheck version-sensitive behavior when used.

- Preserve existing Cargo settings and reuse build artifacts. Avoid unnecessary configuration changes and builds.
- Use `cargo check` for quick feedback; run project-required builds and tests before completion.
- Do not automatically perform a full clean or delete entire target/build directories, `~/.cargo`, or unrelated caches. Verified non-deleting previews are allowed; scoped deletion requires applicable approval.
- When disk space is low, identify actual cache paths and shared users. Coordinate active builds, preview selective cleanup, and obtain required approval. If uncertain, do not delete.
- Keep incremental compilation and debug settings aligned with the project; change them only when justified.

## Agent registration is separate

Explicit agent setup may install this skill into a reviewed user/project scope after consent. It must preserve existing skills and configuration and must not grant project trust or authorize cache deletion. Detection/preview and ordinary disk diagnosis remain read-only. See [distribution.md](distribution.md#agent-skill-setup) when setup is requested; do not load provider configuration or all references for unrelated cache diagnosis.

## Objective

Preserve build correctness and reuse valid artifacts while controlling disk usage and avoiding unnecessary recompilation.

Priority order:
1. Correctness and project-required validation.
2. Reuse of valid artifacts and consistent configuration.
3. Fast, efficient development.
4. Safe, selective cleanup when needed.

## Build and Validation

- Respect the project's existing toolchain, Cargo configuration, profiles, features, target triples, and CI requirements.
- Do not modify project configuration or persistent environment variables solely to apply this policy without authorization.
- Use `cargo check` for a fast intermediate check when appropriate. It does not replace builds or tests.
- Before reporting completion, run the builds and tests required by the change and project acceptance criteria. Run release builds when the project or task requires them.
- Avoid changing build settings repeatedly, because configuration changes may trigger recompilation or generate additional artifacts.
- Keep incremental compilation enabled when it benefits repeated local development. Consider disabling it for disk-constrained or one-off builds after weighing the recompilation cost.
- Use reduced debug information only when appropriate, consistently with project settings and debugging needs. Do not override a project's required settings.
- Reuse the existing build directories. Do not create a new target directory for each task without a specific reason.

## Cache Preservation

- Never automatically perform a full clean, delete an entire target or build directory, or remove `~/.cargo`. Only explicit informed authorization and verified scope can permit a full-clean exception; see [defaults.md](defaults.md).
- Preserve active artifacts, recently used dependencies, registry and Git caches, and files needed for debugging or tests.
- Do not delete a cache merely because it is large. Inspect available disk space, quotas, growth, and current workflow needs.
- Do not delete source files, user data, or unrelated projects' artifacts.

## Disk Monitoring

- During build-intensive work, inspect disk usage when relevant.
- Identify the effective Cargo workspace, target directory, and, where separately configured, build directory. Check applicable environment variables and project or user Cargo configuration.
- `cargo metadata --format-version 1 --no-deps` can help identify `target_directory`; it does not by itself prove the location of every intermediate artifact.
- Account for shared directories and concurrent work by other projects, processes, or agents.
- Use commands appropriate to the operating system; Bash and GNU `du` examples are not universally portable.
- Absolute size thresholds are advisory, not automatic deletion triggers. Consider remaining disk space and growth rate as well.

## Safe Cleanup

When cleanup is justified:
1. Confirm the exact paths, ownership, disk usage, and whether the directories are shared.
2. Coordinate with all work using those directories. Do not clean during active builds or tests; if safe coordination cannot be established, do not clean.
3. Identify specific candidates and preview the operation when supported.
4. Obtain any approval required by the user's instructions and environment before deletion.
5. Prefer the smallest safe, reversible cleanup scope. Preserve useful and recently used artifacts.
6. If the scope, tool compatibility, or safety is uncertain, report the findings and request guidance instead of deleting.

- Age alone does not prove an artifact is unused. Treat any retention period as a guideline, not a safety guarantee.
- Do not install cleanup tools without permission. Use a tool only after checking its maintenance status, compatibility, and exact behavior.
- `cargo-sweep` is currently marked unmaintained by its maintainers. Do not assume it is available, safe for every Cargo version, or able to clean all cache types.
- Never initiate agent-driven purging of registry sources, downloaded dependencies, or unrelated shared caches. This does not disable Cargo's own configured global-cache garbage collection; inspect that behavior and request authorization before changing it.

## Reporting

Report the checks and tests actually run, any failures or skipped validation, and any cleanup performed. Do not claim a successful build or test based only on `cargo check`.

**Core principle:** Preserve useful artifacts, keep configuration stable, validate correctly, and clean only when justified and safe.

## Sources for version-sensitive details

- Cargo profiles: https://doc.rust-lang.org/cargo/reference/profiles.html
- Cargo build cache: https://doc.rust-lang.org/cargo/reference/build-cache.html
- Cargo check: https://doc.rust-lang.org/cargo/commands/cargo-check.html
- cargo-sweep: https://github.com/holmgr/cargo-sweep
