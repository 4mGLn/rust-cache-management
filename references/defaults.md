# Evidence-backed defaults and exceptions

Reviewed against official primary sources on **2026-11-04**. Recheck support with the installed Cargo version. These are decision defaults, not permission to modify configuration. Explicit project settings and required validation take precedence. Distinguish **Cargo defaults**, **upstream tuning advice**, and **this skill's conservative recommendations**.

## 1. Incremental compilation

**Default:** preserve effective settings. For an otherwise unconfigured local development workspace, retain Cargo's `dev` incremental setting (`true`); `test` inherits `dev`. Release defaults to `false` and must not be changed merely for cache management.

**Exception:** consider `false` for a disposable one-off CI environment with no meaningful reuse, or demonstrated disk pressure. This is a workflow-dependent recommendation, not a universal CI rule. Compare representative rebuild time and storage before adopting it. Do not alternate environment overrides on each command. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#incremental)

## 2. Debug information

**Default:** preserve the project's value. Cargo `dev` defaults to full debug information; `test` inherits it. There is no universally correct smaller value. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#debug)

**Opt-in disk/speed preset:** when approved and full debugger support is unnecessary, prefer the official performance guide's `debug = "line-tables-only"` for development and `debug = false` for non-workspace dependencies. Keep a full-debug option for diagnosis. This retains useful file/line backtraces for workspace code while reducing debug data. Changing settings can require rebuilding; measure the actual workflow. [Cargo performance guide](https://doc.rust-lang.org/cargo/guide/build-performance.html#reduce-amount-of-generated-debug-information)

**Exceptions:** use full debug information for variables/types, dependency debugging, or tooling that requires it; check package-specific and inherited overrides too. On an older toolchain, named debug values require Rust 1.71 or later. Numeric `1` is a possible approved fallback, not an exact equivalent to `line-tables-only`. Do not silently upgrade the compiler. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html#debug)

## 3. Dependency caches

**Default:** no agent-driven purging of registry/Git sources or downloads. Preserve existing Cargo cache configuration. On supporting Cargo versions, built-in global-cache garbage collection may still occur during normal Cargo commands: its default check frequency is one day, not a one-day retention period. The documented unused-age thresholds are three months for network downloads and one month for regenerable files. This does **not** clean workspace build artifacts. [Cargo global cache](https://doc.rust-lang.org/cargo/reference/config.html#global-caches)

**Exceptions:** for deliberate long-term offline preservation or mixed old/new Cargo usage, inspect the installed version's tracking limitations and consider a separately authorized configuration change. Cargo disables its automatic deletion in offline/frozen operation; those flags can also affect resolution or fail when dependencies are missing. Do not add them indiscriminately. [Cargo cache configuration](https://doc.rust-lang.org/cargo/reference/config.html#cache)

## 4. Target and intermediate paths

**Default:** reuse the effective paths. With no overrides, both target and intermediate build directories default to `target` at the workspace root. A configured build directory can differ from the target directory. [Cargo build cache](https://doc.rust-lang.org/cargo/reference/build-cache.html)

**Exceptions:** isolation, incompatible workflows, shared-cache contention, or storage placement can justify an authorized alternative. Do not centralize all projects or create per-task directories automatically. Resolve actual settings, symlinks, mounts, and shared users before inspection or deletion; relative config paths must be interpreted correctly. [Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html#configuration-relative-paths)

## 5. Cleanup tooling

**Default:** diagnosis and proposal only. For an approved package-specific cleanup, prefer a supported built-in Cargo scope over arbitrary filesystem deletion. Verify installed help, paths, and concurrency first; preview with package selection, `--dry-run`, and `--verbose` where supported. This is an exception to the blanket command ban only for a verified non-deleting preview or explicitly approved scoped deletion. A package selector is not an age-based retention filter. [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)

**Exceptions:** a third-party tool requires approval to install and evidence of compatibility and scope. As of the review date, cargo-sweep's upstream repository warns it is unmaintained; do not make it the default. Its examples do not establish a safe retention policy. If no safe method is available, report instead of deleting. [cargo-sweep upstream](https://github.com/holmgr/cargo-sweep)

## 6. Retention

**Default:** no automatic age-based expiry for workspace build artifacts. Neither 14 nor 30 days is adopted as a universal safe value. Protect active dependencies and less frequently used configurations.

**Exception:** the project may approve a candidate-review window based on branch/release cadence, cost of rebuilding, and evidence of use. Label it a **local heuristic**, not Cargo guidance or proof that a file is unused. Cargo's global-cache thresholds in item 3 apply to a different cache category. [Cargo global cache](https://doc.rust-lang.org/cargo/reference/config.html#global-caches)

## 7. Monitoring and cleanup triggers

**Default (skill heuristic):** inspect at the start of a known build-heavy task when useful, on disk/quota warnings, or when observed growth threatens the next operation. Do not clean after every build. No fixed GB or free-space percentage is an unconditional trigger.

**Exceptions:** use project-supplied quotas or measured peak build needs as local thresholds. If the next required build cannot fit, pause and propose the smallest safe remedy. Cache size alone does not authorize deletion. Measure effects in the actual workflow, consistent with the upstream advice to benchmark tuning. [Cargo performance guide](https://doc.rust-lang.org/cargo/guide/build-performance.html)

## 8. Full clean

**Default:** prohibit automatic full clean, even for a failed build or a large directory. Unscoped `cargo clean` removes all workspace packages and dependencies in its selected artifact scope. [Cargo clean](https://doc.rust-lang.org/cargo/commands/cargo-clean.html)

**Exception (skill safety policy):** only an explicit, informed request for a clean-room rebuild or justified recovery after narrower remedies, with exact directory scope, shared users, rebuild cost, and required approval established. Never extend this to Cargo home, source files, or unrelated caches. Prefer an isolated test of the suspected issue when authorized rather than erasing useful artifacts first.

## 9. Validation

**Default:** use targeted `cargo check` for rapid feedback, then the builds/tests required by the project's acceptance criteria and actual changes. Check omits final code generation and can miss errors caught later. [Cargo check](https://doc.rust-lang.org/cargo/commands/cargo-check.html)

**Exceptions:** release-only, target-specific, feature-specific, native-linking, or packaging changes need the matching validation. Do not use `--all-features` or a broad test matrix blindly when combinations are incompatible. Do not repeatedly rebuild unchanged code or rebuild everything solely because stale artifacts were removed; report relevant checks performed and any remaining validation limits.
