#!/bin/sh
# Offline fixture tests: mock curl maps only known HTTPS release requests to local bytes.
set -eu
root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd -P)
temp=$(mktemp -d)
temp=$(CDPATH='' cd -- "$temp" && pwd -P)
trap 'rm -rf "$temp"' EXIT HUP INT TERM
mkdir -p "$temp/mock" "$temp/assets/v0.2.0" "$temp/assets/v0.3.0"
cat > "$temp/helper" <<'EOF'
#!/bin/sh
[ "${1:-}" != --fixture-error ] || exit 7
if [ "${1:-}" = --fixture-args ]; then shift; printf '%s\n' "$@"; exit 0; fi
printf 'fixture diagnostic\n'
EOF
chmod +x "$temp/helper"
case "$(uname -s):$(uname -m)" in
 Linux:x86_64) target=x86_64-unknown-linux-gnu;; Linux:aarch64) target=aarch64-unknown-linux-gnu;; Darwin:arm64) target=aarch64-apple-darwin;; Darwin:x86_64) target=x86_64-apple-darwin;; *) exit 2;;
esac
for v in v0.2.0 v0.3.0; do
  sh "$root/scripts/package.sh" "$v" "$target" "$temp/helper" "$temp/assets/$v" >/dev/null
  cat "$temp/assets/$v/"*.sha256 > "$temp/assets/$v/SHA256SUMS"
done
cat > "$temp/mock/curl" <<'EOF'
#!/bin/sh
set -eu
out=; head=false; url=
while [ "$#" -gt 0 ]; do
 case "$1" in
  --output) out=$2; shift 2;; --head) head=true; shift;;
  --write-out|--proto|--proto-redir|--retry|--connect-timeout|--max-time) shift 2;;
  --fail|--silent|--show-error|--location) shift;; https://*) url=$1; shift;; *) exit 90;;
 esac
done
printf '%s\n' "$url" >> "$FIXTURE_LOG"
case "$url" in https://github.com/fixture/cache/releases/*|https://github.com/4mGLn/rust-cache-management/releases/*) ;; *) exit 91;; esac
if [ "$head" = true ]; then
 case "${FIXTURE_LATEST_MODE:-valid}" in
  http-error) exit 22;;
  invalid) printf 'https://github.com/fixture/cache/releases/tag/not-a-version';;
  ambiguous) printf 'https://example.invalid/fixture/cache/releases/tag/v0.2.0';;
  multiple) printf 'https://github.com/fixture/cache/releases/tag/v0.2.0\nhttps://github.com/fixture/cache/releases/tag/v0.2.0';;
  *) printf '%s/tag/%s' "${url%/latest}" "${FIXTURE_LATEST:-v0.2.0}";;
 esac
 exit 0
fi
case "$url" in
 https://github.com/fixture/cache/*) relative=${url#https://github.com/fixture/cache/releases/download/};;
 *) relative=${url#https://github.com/4mGLn/rust-cache-management/releases/download/};;
esac
[ "$relative" != "$url" ] || exit 92
cp "$FIXTURE_ASSETS/$relative" "$out"
EOF
chmod +x "$temp/mock/curl"
export PATH="$temp/mock:$PATH" FIXTURE_ASSETS="$temp/assets" FIXTURE_LOG="$temp/network.log"
# Every ordinary fixture gets a separate command directory. The parent/user HOME is never changed.
install_fixture() {
 _fixture_prefix=${RUST_CACHE_INSTALL_DIR:-}
 _fixture_bin=
 _fixture_expect=
 for _fixture_arg in "$@"; do
  if [ "$_fixture_expect" = prefix ]; then _fixture_prefix=$_fixture_arg; _fixture_expect=; continue; fi
  if [ "$_fixture_expect" = bin ]; then _fixture_bin=$_fixture_arg; _fixture_expect=; continue; fi
  case "$_fixture_arg" in --prefix) _fixture_expect=prefix;; --bin-dir) _fixture_expect=bin;; esac
 done
 [ -n "$_fixture_prefix" ] || { printf 'Fixture needs explicit prefix\n' >&2; return 2; }
 [ -n "$_fixture_bin" ] || _fixture_bin=$_fixture_prefix-commands
 sh "$root/install.sh" "$@" --bin-dir "$_fixture_bin" 2>> "$temp/installer-messages.log"
}
install_fixture --prefix "$temp/default-repository" --version v0.2.0 >/dev/null
[ "$(cat "$temp/default-repository/repository")" = 4mGLn/rust-cache-management ]
# Common pipeline and default latest mode: no version argument and no actual user prefix.
export RUST_CACHE_INSTALL_DIR="$temp/latest-pipeline" RUST_CACHE_BIN_DIR="$temp/latest-pipeline-commands"
cat "$root/install.sh" | sh >/dev/null
[ "$(cat "$temp/latest-pipeline/current")" = "v0.2.0-$target" ]
[ "$(cat "$temp/latest-pipeline/repository")" = 4mGLn/rust-cache-management ]
# Explicit prefix overrides the process-only application setting.
install_fixture --prefix "$temp/explicit-prefix" >/dev/null
[ -f "$temp/explicit-prefix/current" ]
unset RUST_CACHE_INSTALL_DIR RUST_CACHE_BIN_DIR
for mode in http-error invalid ambiguous multiple; do
 if FIXTURE_LATEST_MODE="$mode" install_fixture --prefix "$temp/latest-$mode" 2>/dev/null; then exit 1; fi
 [ ! -e "$temp/latest-$mode/current" ]
 [ ! -e "$temp/latest-$mode" ]
done
prefix=$temp/install
install_fixture --repo fixture/cache --prefix "$prefix" --version v0.2.0 >/dev/null
[ ! -e "$prefix/auto-update" ]
sh "$prefix/manage.sh" status >/dev/null
if "$prefix/bin/rust-cache-inspect" --fixture-error; then exit 1; else [ "$?" -eq 7 ]; fi
manager_before=$(shasum -a 256 "$prefix/manage.sh")
launcher_before=$(shasum -a 256 "$prefix/bin/rust-cache-inspect")
install_fixture --repo fixture/cache --prefix "$prefix" --version v0.2.0 >/dev/null
[ "$(shasum -a 256 "$prefix/manage.sh")" = "$manager_before" ]
[ "$(shasum -a 256 "$prefix/bin/rust-cache-inspect")" = "$launcher_before" ]
[ -f "$prefix-commands/rust-cache-inspect" ] && [ ! -L "$prefix-commands/rust-cache-inspect" ] && [ -x "$prefix-commands/rust-cache-inspect" ]
"$prefix-commands/rust-cache-management" --help >/dev/null
"$prefix-commands/rust-cache-management" >/dev/null
"$prefix-commands/rust-cache-management" status >/dev/null
if "$prefix-commands/rust-cache-inspect" --fixture-error; then exit 1; else [ "$?" -eq 7 ]; fi
if install_fixture --repo other/repository --prefix "$prefix" --version v0.2.0 2>/dev/null; then exit 1; fi
mkdir "$temp/unknown-prefix"; printf keep > "$temp/unknown-prefix/user-file"
if install_fixture --prefix "$temp/unknown-prefix" 2>/dev/null; then exit 1; fi
[ "$(cat "$temp/unknown-prefix/user-file")" = keep ]
# Unknown files and symlinks collide before an installation is created.
mkdir "$temp/collision-bin"; printf keep > "$temp/collision-bin/rust-cache-inspect"
if install_fixture --prefix "$temp/collision-prefix" --bin-dir "$temp/collision-bin" 2>/dev/null; then exit 1; fi
[ ! -e "$temp/collision-prefix" ] && [ "$(cat "$temp/collision-bin/rust-cache-inspect")" = keep ]
rm "$temp/collision-bin/rust-cache-inspect"; ln -s "$temp/helper" "$temp/collision-bin/rust-cache-inspect"
if install_fixture --prefix "$temp/collision-link-prefix" --bin-dir "$temp/collision-bin" 2>/dev/null; then exit 1; fi
[ -L "$temp/collision-bin/rust-cache-inspect" ] && [ ! -e "$temp/collision-link-prefix" ]
# Wrappers must quote an apostrophe-bearing prefix, preserve arguments, exit status and executable modes.
quoted_prefix="$temp/install space's"
quoted_bin="$temp/command space's"
install_fixture --prefix "$quoted_prefix" --bin-dir "$quoted_bin" --version v0.2.0 >/dev/null
"$quoted_bin/rust-cache-inspect" --fixture-args 'space arg' "quote'arg" '' > "$temp/actual-args"
printf '%s\n' 'space arg' "quote'arg" '' > "$temp/expected-args"
cmp -s "$temp/actual-args" "$temp/expected-args"
if "$quoted_bin/rust-cache-inspect" --fixture-error; then exit 1; else [ "$?" -eq 7 ]; fi
chmod 644 "$quoted_bin/rust-cache-inspect"
if install_fixture --prefix "$quoted_prefix" --bin-dir "$quoted_bin" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -x "$quoted_bin/rust-cache-inspect" ]
chmod 755 "$quoted_bin/rust-cache-inspect"; printf '# user edit\n' >> "$quoted_bin/rust-cache-inspect"
if install_fixture --prefix "$quoted_prefix" --bin-dir "$quoted_bin" --version v0.2.0 2>/dev/null; then exit 1; fi
tail -n 1 "$quoted_bin/rust-cache-inspect" | grep -q 'user edit'
grep -q 'not in PATH' "$temp/installer-messages.log"
# User explicitly authorized isolated child HOME for testing the true default ~/.local/bin layout.
probe_user_home_dir="$temp/probe-user-home"
mkdir "$probe_user_home_dir"
# Only child HOME is changed. Existing shell startup files and this process environment are preserved.
printf '# user profile keep\n' > "$probe_user_home_dir/.profile"
printf '# user bashrc keep\n' > "$probe_user_home_dir/.bashrc"
profile_before=$(shasum -a 256 "$probe_user_home_dir/.profile" "$probe_user_home_dir/.bashrc")
parent_home_before=${HOME:-}; parent_path_before=$PATH
env HOME="$probe_user_home_dir" RUST_CACHE_INSTALL_DIR= RUST_CACHE_BIN_DIR= PATH="$probe_user_home_dir/.local/bin:$PATH" sh "$root/install.sh" --repo fixture/cache >/dev/null
[ -f "$probe_user_home_dir/.local/bin/rust-cache-inspect" ] && [ ! -L "$probe_user_home_dir/.local/bin/rust-cache-inspect" ]
env HOME="$probe_user_home_dir" PATH="$probe_user_home_dir/.local/bin:$PATH" rust-cache-management --help >/dev/null
if env HOME="$probe_user_home_dir" PATH="$probe_user_home_dir/.local/bin:$PATH" rust-cache-inspect --fixture-error; then exit 1; else [ "$?" -eq 7 ]; fi
env HOME="$probe_user_home_dir" RUST_CACHE_INSTALL_DIR= RUST_CACHE_BIN_DIR= PATH="$probe_user_home_dir/.local/bin:$PATH" sh "$root/install.sh" --repo fixture/cache >/dev/null
[ "$(shasum -a 256 "$probe_user_home_dir/.profile" "$probe_user_home_dir/.bashrc")" = "$profile_before" ]
[ "${HOME:-}" = "$parent_home_before" ] && [ "$PATH" = "$parent_path_before" ]
env HOME="$probe_user_home_dir" RUST_CACHE_INSTALL_DIR= RUST_CACHE_BIN_DIR= PATH="$PATH" sh "$root/install.sh" --repo fixture/cache >/dev/null 2> "$temp/missing-path-warning"
grep -q 'Selected bin directory is not in PATH' "$temp/missing-path-warning"
grep -q 'No PATH or shell profile was changed' "$temp/missing-path-warning"
if install_fixture --prefix "$temp/internal-bin" --bin-dir "$temp/internal-bin/bin" 2>/dev/null; then exit 1; fi
[ ! -e "$temp/internal-bin" ]
if install_fixture --prefix "$temp/ambiguous//prefix" 2>/dev/null; then exit 1; fi
# Exercise system permission mode in owned temporary directories with a fixture-only id command.
# This is a mode/record test, not a privileged /usr/local installation.
cat > "$temp/mock/id" <<'ID'
#!/bin/sh
[ "$1" = -u ] || exit 92
printf '0\n'
ID
chmod +x "$temp/mock/id"
# Mock owner/ancestor metadata only; this does not establish real root security.
# Permission assertions below use actual stat mode values via the passthrough.
fixture_real_stat=$(command -v stat)
export FIXTURE_REAL_STAT="$fixture_real_stat"
cat > "$temp/mock/stat" <<'STAT'
#!/bin/sh
case "$2" in
 '%u %a'|'%u %Lp')
   case "${FIXTURE_SYSTEM_METADATA:-trusted}" in
    nonroot) printf '1000 755\n';;
    writable) printf '0 775\n';;
    *) printf '0 755\n';;
   esac;;
 *) exec "$FIXTURE_REAL_STAT" "$@";;
esac
STAT
chmod +x "$temp/mock/stat"
for metadata_case in nonroot writable; do
 if FIXTURE_SYSTEM_METADATA="$metadata_case" install_fixture --system --prefix "$temp/system-refused-$metadata_case" --bin-dir "$temp/system-refused-bin-$metadata_case" --repo fixture/cache --version v0.2.0 2>/dev/null; then exit 1; fi
 [ ! -e "$temp/system-refused-$metadata_case" ]
done
install_fixture --system --prefix "$temp/system-mode" --bin-dir "$temp/system-commands" --repo fixture/cache --version v0.2.0 >/dev/null
[ "$(cat "$temp/system-mode/installation-mode")" = system ]
assert_mode() {
 expected_mode=$1; shift
 for mode_path in "$@"; do
  case "$(uname -s)" in Darwin) actual_mode=$(stat -f %Lp "$mode_path");; *) actual_mode=$(stat -c %a "$mode_path");; esac
  [ "$actual_mode" = "$expected_mode" ]
 done
}
assert_mode 755 "$temp/system-mode" "$temp/system-mode/releases/v0.2.0-$target"
assert_mode 644 "$temp/system-mode/current"
sh "$temp/system-mode/manage.sh" update --version v0.3.0 >/dev/null
assert_mode 644 "$temp/system-mode/current" "$temp/system-mode/previous"
sh "$temp/system-mode/manage.sh" rollback >/dev/null
assert_mode 644 "$temp/system-mode/current"
"$temp/system-commands/rust-cache-inspect" >/dev/null
rm "$temp/mock/id" "$temp/mock/stat"
unset FIXTURE_REAL_STAT
if [ "$(id -u)" -ne 0 ]; then
 if install_fixture --system --prefix "$temp/not-system" --bin-dir "$temp/not-system-bin" 2>/dev/null; then exit 1; fi
 [ ! -e "$temp/not-system" ]
fi
# A source installer rerun can advance latest without replacing old bootstrap code.
install_fixture --repo fixture/cache --prefix "$temp/rerun-update" --version v0.2.0 >/dev/null
rerun_manager=$(shasum -a 256 "$temp/rerun-update/manage.sh")
FIXTURE_LATEST=v0.3.0 install_fixture --repo fixture/cache --prefix "$temp/rerun-update" >/dev/null
[ "$(cat "$temp/rerun-update/current")" = "v0.3.0-$target" ]
[ "$(cat "$temp/rerun-update/previous")" = "v0.2.0-$target" ]
[ "$(shasum -a 256 "$temp/rerun-update/manage.sh")" = "$rerun_manager" ]
printf '# preserve modified primary\n' >> "$temp/rerun-update/bin/rust-cache-management"
if FIXTURE_LATEST=v0.3.0 install_fixture --repo fixture/cache --prefix "$temp/rerun-update" 2>/dev/null; then exit 1; fi
grep -q 'preserve modified primary' "$temp/rerun-update/bin/rust-cache-management"
sh "$prefix/manage.sh" update --version v0.3.0 >/dev/null
[ "$(cat "$prefix/current")" = "v0.3.0-$target" ]
[ "$(cat "$prefix/previous")" = "v0.2.0-$target" ]
sh "$prefix/manage.sh" rollback >/dev/null
[ "$(cat "$prefix/current")" = "v0.2.0-$target" ]
skill=$prefix/releases/v0.2.0-$target/SKILL.md
cp "$skill" "$temp/skill-original"
printf '\nuser edit\n' >> "$skill"
if sh "$prefix/manage.sh" update --version v0.3.0 2>/dev/null; then exit 1; fi
tail -n 1 "$skill" | grep -q 'user edit'
cp "$temp/skill-original" "$skill"
ln -s "$temp" "$temp/linked"
if install_fixture --repo fixture/cache --prefix "$temp/linked/rejected" --version v0.2.0 2>/dev/null; then exit 1; fi
# A checksum failure never activates and an empty failed-install prefix is retryable.
cp "$temp/assets/v0.2.0/SHA256SUMS" "$temp/sums-original"
printf '%064d  rust-cache-management-v0.2.0-%s.tar.gz\n' 0 "$target" > "$temp/assets/v0.2.0/SHA256SUMS"
if install_fixture --repo fixture/cache --prefix "$temp/corrupt" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -e "$temp/corrupt/current" ]
[ ! -e "$temp/corrupt" ]
cp "$temp/sums-original" "$temp/assets/v0.2.0/SHA256SUMS"
install_fixture --repo fixture/cache --prefix "$temp/corrupt" --version v0.2.0 >/dev/null
# Unexpected archive members rejected even if the outer checksum matches.
mkdir "$temp/unsafe-payload"
printf 'do not extract\n' > "$temp/unsafe-payload/unexpected"
asset=rust-cache-management-v0.2.0-$target.tar.gz
cp "$temp/assets/v0.2.0/$asset" "$temp/archive-original"
tar -czf "$temp/assets/v0.2.0/$asset" -C "$temp/unsafe-payload" unexpected
if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); else h=$(shasum -a 256 "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); fi
printf '%s  %s\n' "$h" "$asset" > "$temp/assets/v0.2.0/SHA256SUMS"
if install_fixture --repo fixture/cache --prefix "$temp/unsafe" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -e "$temp/unsafe" ]
cp "$temp/archive-original" "$temp/assets/v0.2.0/$asset"
cp "$temp/sums-original" "$temp/assets/v0.2.0/SHA256SUMS"
# A correct outer checksum cannot excuse a mismatched inner manifest.
mkdir "$temp/inner-payload"
tar -xzf "$temp/archive-original" -C "$temp/inner-payload"
printf '\nchanged after manifest\n' >> "$temp/inner-payload/SKILL.md"
tar -tzf "$temp/archive-original" > "$temp/member-list"
tar -czf "$temp/assets/v0.2.0/$asset" -C "$temp/inner-payload" -T "$temp/member-list"
if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); else h=$(shasum -a 256 "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); fi
printf '%s  %s\n' "$h" "$asset" > "$temp/assets/v0.2.0/SHA256SUMS"
if install_fixture --repo fixture/cache --prefix "$temp/inner-bad" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -e "$temp/inner-bad" ]
cp "$temp/archive-original" "$temp/assets/v0.2.0/$asset"
cp "$temp/sums-original" "$temp/assets/v0.2.0/SHA256SUMS"
# A failed automatic update leaves the old pointer and still returns the diagnostic's exit7.
install_fixture --repo fixture/cache --prefix "$temp/fail-update" --version v0.2.0 --auto-update >/dev/null
cp "$temp/assets/v0.3.0/SHA256SUMS" "$temp/sums3-original"
printf '%064d  rust-cache-management-v0.3.0-%s.tar.gz\n' 0 "$target" > "$temp/assets/v0.3.0/SHA256SUMS"
export FIXTURE_LATEST=v0.3.0
if "$temp/fail-update/bin/rust-cache-inspect" --fixture-error 2>/dev/null; then exit 1; else [ "$?" -eq 7 ]; fi
[ "$(cat "$temp/fail-update/current")" = "v0.2.0-$target" ]
[ ! -e "$temp/fail-update/releases/v0.3.0-$target" ]
cp "$temp/sums3-original" "$temp/assets/v0.3.0/SHA256SUMS"
# Opt-in auto-update warnings cannot replace diagnostic exit status; check only once daily.
sh "$prefix/manage.sh" auto-on >/dev/null
export FIXTURE_LATEST=v0.3.0
if "$prefix/bin/rust-cache-inspect" --fixture-error 2>/dev/null; then exit 1; else [ "$?" -eq 7 ]; fi
[ "$(cat "$prefix/current")" = "v0.3.0-$target" ]
[ "$(cat "$prefix/previous")" = "v0.2.0-$target" ]
lines=$(wc -l < "$FIXTURE_LOG")
if "$prefix/bin/rust-cache-inspect" --fixture-error 2>/dev/null; then exit 1; else [ "$?" -eq 7 ]; fi
[ "$(wc -l < "$FIXTURE_LOG")" -eq "$lines" ]
printf '08\n' > "$prefix/last-check"
if "$prefix/bin/rust-cache-inspect" --fixture-error 2>/dev/null; then exit 1; else [ "$?" -eq 7 ]; fi
sh "$prefix/manage.sh" auto-off >/dev/null
[ ! -e "$prefix/auto-update" ]
# Linked payload and lock refuse update, without touching the current pointer.
skill=$prefix/releases/$(cat "$prefix/current")/SKILL.md
mv "$skill" "$temp/skill"
ln -s "$temp/skill" "$skill"
if sh "$prefix/manage.sh" update --version v0.3.0 2>/dev/null; then exit 1; fi
rm "$skill"; mv "$temp/skill" "$skill"
mkdir "$prefix/.lock"
if sh "$prefix/manage.sh" update --version v0.3.0 2>/dev/null; then exit 1; fi
rmdir "$prefix/.lock"
if [ -n "${RCM_TEST_BINARY:-}" ]; then
  actual_version=v$(node -e 'const s=require("fs").readFileSync(process.argv[1],"utf8");process.stdout.write(s.match(/^version = "([^"]+)"/m)[1])' "$root/Cargo.toml")
  mkdir -p "$temp/assets/$actual_version"
  sh "$root/scripts/package.sh" "$actual_version" "$target" "$RCM_TEST_BINARY" "$temp/assets/$actual_version" >/dev/null
  cat "$temp/assets/$actual_version/"*.sha256 > "$temp/assets/$actual_version/SHA256SUMS"
  install_fixture --repo fixture/cache --prefix "$temp/real-binary" --version "$actual_version" >/dev/null
  "$temp/real-binary/bin/rust-cache-inspect" --version
  mkdir "$temp/cache"
  printf 'hello' > "$temp/cache/artifact"
  "$temp/real-binary/bin/rust-cache-inspect" --path "$temp/cache" > "$temp/diagnostic.json"
  tr -d '[:space:]' < "$temp/diagnostic.json" | grep -q '\"logical_file_bytes\":5'
  [ "$(cat "$temp/cache/artifact")" = hello ]
  node "$root/scripts/test-agent-setup.mjs" "$temp/real-binary/releases/$actual_version-$target/bin/rust-cache-inspect" "$temp/real-binary-commands/rust-cache-management"
  "$temp/real-binary-commands/rust-cache-management" agents detect --json > "$temp/agents-detect.json"
  node -e 'JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"))' "$temp/agents-detect.json"
  printf 'Real Rust binary launcher and packaged setup smoke passed; selected host asset is a local packaging fixture.\n'
fi
printf 'Distribution fixture tests passed (install, update, rollback, preservation, hashes, archive allowlist, links, lock, opt-in cadence, exit codes).\n'
