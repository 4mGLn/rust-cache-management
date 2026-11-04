mod agents;
use rust_cache_inspect::{report, DEFAULT_MAX_ENTRIES, MAX_ENTRIES};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

const HELP: &str = "rust-cache-inspect — read-only diagnostics for explicit cache directories\n\nUsage: rust-cache-inspect --path DIRECTORY [--path DIRECTORY ...] [--max-entries NUMBER]\n\n  --path DIRECTORY      Explicit, verified directory to inspect; repeat as needed\n  --max-entries NUMBER  Per-path entry budget, 1..10000000 (default 100000)\n  --help, -h            Show this help\n  --version, -V         Show version\n\nAgent integration: rust-cache-inspect agents detect --json\n  rust-cache-inspect agents setup --agent codex,gemini --scope user --dry-run --json\n  rust-cache-inspect agents verify --agent codex,claude --scope user --json\n  Verify explicitly runs provider version/catalog checks with isolated temporary runtime; detect/setup do not execute providers.\n  Add --yes to create skill copies and setup records; existing files are preserved. Project scope requires --project ABSOLUTE_DIRECTORY. agents --help lists setup exit codes.\n\nOutputs JSON. Never invokes Cargo or changes cache/configuration files.\nPaths containing links/reparse ancestors are rejected. No tilde/environment expansion.\nExit codes: 0 = complete within skip policy; 1 = errors or scan budget exhausted;\n2 = invalid arguments or output failure. Skips do not establish cleanup safety.\n";

fn argument_value(
    args: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<OsString, String> {
    args.next().ok_or_else(|| format!("{option} needs a value"))
}

fn run() -> Result<i32, String> {
    let values: Vec<OsString> = std::env::args_os().skip(1).collect();
    if values.first().is_some_and(|arg| arg == "agents") {
        return Ok(agents::run(values.into_iter().skip(1).collect()));
    }
    let mut args = values.into_iter();
    let mut paths = Vec::new();
    let mut limit = DEFAULT_MAX_ENTRIES;
    let mut seen_limit = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                print!("{HELP}");
                return Ok(0);
            }
            Some("--version" | "-V") => {
                println!("rust-cache-inspect {}", env!("CARGO_PKG_VERSION"));
                return Ok(0);
            }
            Some("--path") => {
                let value = argument_value(&mut args, "--path")?;
                if value.is_empty() {
                    return Err("--path must not be empty".into());
                }
                paths.push(PathBuf::from(value));
            }
            Some("--max-entries") => {
                if seen_limit {
                    return Err("--max-entries may only be supplied once".into());
                }
                seen_limit = true;
                let value = argument_value(&mut args, "--max-entries")?;
                limit = value
                    .to_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|n| (1..=MAX_ENTRIES).contains(n))
                    .ok_or_else(|| {
                        format!("--max-entries must be an integer between 1 and {MAX_ENTRIES}")
                    })?;
            }
            _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy())),
        }
    }
    if paths.is_empty() {
        return Err("At least one --path is required".into());
    }
    let report = report(&paths, limit);
    let exit = i32::from(report.paths.iter().any(|p| !p.complete_scan));
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &report)
        .map_err(|e| format!("Cannot write JSON: {e}"))?;
    writeln!(output).map_err(|e| format!("Cannot write JSON: {e}"))?;
    Ok(exit)
}

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("rust-cache-inspect: {error}\nUse --help for usage.");
            std::process::exit(2);
        }
    }
}
