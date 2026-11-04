#!/bin/sh
# Live smoke in an ephemeral prefix. No version is supplied to the installer.
set -eu
mode=${1:-published}
expected=${2:-}
case "$mode" in source|published) ;; *) printf 'Usage: test-distribution-live.sh source|published [EXPECTED_RELEASE]\n' >&2; exit 2;; esac
root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd -P)
temp=$(mktemp -d)
temp=$(CDPATH='' cd -- "$temp" && pwd -P)
trap 'rm -rf "$temp"' EXIT HUP INT TERM
repo=4mGLn/rust-cache-management
latest=
attempt=0
while [ "$attempt" -lt 12 ]; do
  latest=$(curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 30 "https://api.github.com/repos/$repo/releases/latest" | node -e 'let s="";process.stdin.on("data",x=>s+=x);process.stdin.on("end",()=>process.stdout.write(JSON.parse(s).tag_name));')
  printf '%s\n' "$latest" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || exit 1
  [ -z "$expected" ] || [ "$latest" != "$expected" ] || break
  [ -n "$expected" ] || break
  attempt=$((attempt+1)); [ "$attempt" -ge 12 ] || sleep 5
done
[ -z "$expected" ] || [ "$latest" = "$expected" ] || { printf 'Latest API does not match expected release\n' >&2; exit 1; }
export RUST_CACHE_INSTALL_DIR="$temp/installed"
if [ "$mode" = published ]; then
  curl -fsSL "https://github.com/$repo/releases/latest/download/install.sh" | sh
else
  sh "$root/install.sh" --prefix "$RUST_CACHE_INSTALL_DIR"
fi
current=$(cat "$RUST_CACHE_INSTALL_DIR/current")
case "$current" in "$latest"-*) ;; *) printf 'Activated release differs from actual latest API\n' >&2; exit 1;; esac
[ ! -e "$RUST_CACHE_INSTALL_DIR/auto-update" ]
[ "$("$RUST_CACHE_INSTALL_DIR/bin/rust-cache-inspect" --version)" = "rust-cache-inspect ${latest#v}" ]
mkdir "$temp/cache"
printf hello > "$temp/cache/artifact"
"$RUST_CACHE_INSTALL_DIR/bin/rust-cache-inspect" --path "$temp/cache" > "$temp/diagnostic.json"
node -e 'const x=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).paths[0];if(x.logical_file_bytes!==5||x.errors.length||!x.complete_scan)process.exit(1);' "$temp/diagnostic.json"
[ "$(cat "$temp/cache/artifact")" = hello ]
printf 'Live latest installer passed: mode=%s latest=%s current=%s; optimized scan5bytes and fixture preservation verified.\n' "$mode" "$latest" "$current"
