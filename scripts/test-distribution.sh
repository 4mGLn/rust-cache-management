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
sh "$root/install.sh" --prefix "$temp/default-repository" --version v0.2.0 >/dev/null
[ "$(cat "$temp/default-repository/repository")" = 4mGLn/rust-cache-management ]
# Common pipeline and default latest mode: no version argument and no actual user prefix.
export RUST_CACHE_INSTALL_DIR="$temp/latest-pipeline"
cat "$root/install.sh" | sh >/dev/null
[ "$(cat "$temp/latest-pipeline/current")" = "v0.2.0-$target" ]
[ "$(cat "$temp/latest-pipeline/repository")" = 4mGLn/rust-cache-management ]
# Explicit prefix overrides the process-only application setting.
sh "$root/install.sh" --prefix "$temp/explicit-prefix" >/dev/null
[ -f "$temp/explicit-prefix/current" ]
unset RUST_CACHE_INSTALL_DIR
for mode in http-error invalid ambiguous multiple; do
 if FIXTURE_LATEST_MODE="$mode" sh "$root/install.sh" --prefix "$temp/latest-$mode" 2>/dev/null; then exit 1; fi
 [ ! -e "$temp/latest-$mode/current" ]
 [ ! -e "$temp/latest-$mode" ]
done
prefix=$temp/install
sh "$root/install.sh" --repo fixture/cache --prefix "$prefix" --version v0.2.0 >/dev/null
[ ! -e "$prefix/auto-update" ]
sh "$prefix/manage.sh" status >/dev/null
if "$prefix/bin/rust-cache-inspect" --fixture-error; then exit 1; else [ "$?" -eq 7 ]; fi
if sh "$root/install.sh" --repo fixture/cache --prefix "$prefix" --version v0.2.0 2>/dev/null; then exit 1; fi
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
if sh "$root/install.sh" --repo fixture/cache --prefix "$temp/linked/rejected" --version v0.2.0 2>/dev/null; then exit 1; fi
# A checksum failure never activates and an empty failed-install prefix is retryable.
cp "$temp/assets/v0.2.0/SHA256SUMS" "$temp/sums-original"
printf '%064d  rust-cache-management-v0.2.0-%s.tar.gz\n' 0 "$target" > "$temp/assets/v0.2.0/SHA256SUMS"
if sh "$root/install.sh" --repo fixture/cache --prefix "$temp/corrupt" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -e "$temp/corrupt/current" ]
[ ! -e "$temp/corrupt" ]
cp "$temp/sums-original" "$temp/assets/v0.2.0/SHA256SUMS"
sh "$root/install.sh" --repo fixture/cache --prefix "$temp/corrupt" --version v0.2.0 >/dev/null
# Unexpected archive members rejected even if the outer checksum matches.
mkdir "$temp/unsafe-payload"
printf 'do not extract\n' > "$temp/unsafe-payload/unexpected"
asset=rust-cache-management-v0.2.0-$target.tar.gz
cp "$temp/assets/v0.2.0/$asset" "$temp/archive-original"
tar -czf "$temp/assets/v0.2.0/$asset" -C "$temp/unsafe-payload" unexpected
if command -v sha256sum >/dev/null 2>&1; then h=$(sha256sum "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); else h=$(shasum -a 256 "$temp/assets/v0.2.0/$asset" | cut -d ' ' -f 1); fi
printf '%s  %s\n' "$h" "$asset" > "$temp/assets/v0.2.0/SHA256SUMS"
if sh "$root/install.sh" --repo fixture/cache --prefix "$temp/unsafe" --version v0.2.0 2>/dev/null; then exit 1; fi
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
if sh "$root/install.sh" --repo fixture/cache --prefix "$temp/inner-bad" --version v0.2.0 2>/dev/null; then exit 1; fi
[ ! -e "$temp/inner-bad" ]
cp "$temp/archive-original" "$temp/assets/v0.2.0/$asset"
cp "$temp/sums-original" "$temp/assets/v0.2.0/SHA256SUMS"
# A failed automatic update leaves the old pointer and still returns the diagnostic's exit7.
sh "$root/install.sh" --repo fixture/cache --prefix "$temp/fail-update" --version v0.2.0 --auto-update >/dev/null
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
  rm "$temp/assets/v0.2.0/$asset" "$temp/assets/v0.2.0/$asset.sha256"
  sh "$root/scripts/package.sh" v0.2.0 "$target" "$RCM_TEST_BINARY" "$temp/assets/v0.2.0" >/dev/null
  cat "$temp/assets/v0.2.0/"*.sha256 > "$temp/assets/v0.2.0/SHA256SUMS"
  sh "$root/install.sh" --repo fixture/cache --prefix "$temp/real-binary" --version v0.2.0 >/dev/null
  "$temp/real-binary/bin/rust-cache-inspect" --version
  mkdir "$temp/cache"
  printf 'hello' > "$temp/cache/artifact"
  "$temp/real-binary/bin/rust-cache-inspect" --path "$temp/cache" > "$temp/diagnostic.json"
  tr -d '[:space:]' < "$temp/diagnostic.json" | grep -q '\"logical_file_bytes\":5'
  [ "$(cat "$temp/cache/artifact")" = hello ]
  printf 'Real Rust binary launcher smoke passed; selected host asset is a local packaging fixture.\n'
fi
printf 'Distribution fixture tests passed (install, update, rollback, preservation, hashes, archive allowlist, links, lock, opt-in cadence, exit codes).\n'
