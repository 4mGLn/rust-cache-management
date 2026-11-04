#!/bin/sh
# Versioned user-local install/update manager. No services or scheduled tasks.
set -eu
umask 077
fail() { printf '%s\n' "rust-cache-management: $*" >&2; exit 1; }
usage() {
  cat <<'EOF'
Usage: sh install.sh [--repo OWNER/REPO] [--prefix ABSOLUTE_PATH] [--version vN.N.N] [--auto-update]
Default repository: 4mGLn/rust-cache-management
Installed manager: sh PREFIX/manage.sh update|rollback|auto-on|auto-off|status
Automatic updates are OFF by default. Enabling checks on launcher invocation, at most daily.
No service, cron job, agent configuration, Cargo cache, or shell profile is changed.
EOF
}
valid_version() { printf '%s\n' "$1" | LC_ALL=C grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; }
valid_repo() { printf '%s\n' "$1" | LC_ALL=C grep -Eq '^[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*$'; }
plain_path() {
  case "$1" in /*) ;; *) fail 'installation prefix must be absolute';; esac
  case "$1" in *'
'*|*'/../'*|*'/..'|*'/./'*|*'/.'|/) fail 'unsafe installation prefix';; esac
  _walk=$1
  while :; do
    [ ! -L "$_walk" ] || fail "symlink refused: $_walk"
    [ "$_walk" != / ] || break
    _walk=$(dirname "$_walk")
  done
}
hash_file() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d ' ' -f 1
  else fail 'sha256sum or shasum is required'; fi
}
get() { curl --fail --silent --show-error --location --proto '=https' --proto-redir '=https' --retry 2 --connect-timeout 15 --max-time 180 --output "$2" "$1"; }
# This fixed payload list is also validated before any archive member is written.
file_list() {
  printf '%s\n' AGENTS-snippet.md CHANGELOG.md CONTRIBUTING.md LICENSE README.md SKILL.md THIRD-PARTY-NOTICES VERSION "bin/$binary" install.ps1 install.sh references/defaults.md references/distribution.md references/inspection.md references/policy.md | LC_ALL=C sort
}
read_pointer() {
  [ -f "$prefix/$1" ] && [ ! -L "$prefix/$1" ] || fail "missing or linked $1 pointer"
  _pointer=$(cat "$prefix/$1")
  case "$_pointer" in v*-"$target") ;; *) fail "invalid $1 pointer";; esac
  _v=${_pointer%-"$target"}; valid_version "$_v" || fail "invalid $1 version"
  printf '%s\n' "$_pointer"
}
verify_payload() {
  _dir=$1; _baseline=$2
  plain_path "$_dir"
  [ -d "$_dir" ] || fail 'release directory missing'
  [ -f "$_dir/MANIFEST.sha256" ] && [ ! -L "$_dir/MANIFEST.sha256" ] || fail 'manifest missing or linked'
  [ "$(hash_file "$_dir/MANIFEST.sha256")" = "$_baseline" ] || fail 'installed manifest was modified; update refused'
  file_list > "$stage/expected"
  sed -n 's/^[0-9a-f]\{64\}  //p' "$_dir/MANIFEST.sha256" | LC_ALL=C sort > "$stage/actual"
  cmp -s "$stage/expected" "$stage/actual" || fail 'invalid payload manifest'
  while IFS=' ' read -r _hash _file || [ -n "$_hash$_file" ]; do
    [ -f "$_dir/$_file" ] && [ ! -L "$_dir/$_file" ] || fail "missing or linked payload: $_file"
    plain_path "$_dir/$_file"
    [ "$(hash_file "$_dir/$_file")" = "$_hash" ] || fail "modified payload preserved; update refused: $_file"
  done < "$_dir/MANIFEST.sha256"
}
verify_release() {
  _release=$1
  [ -f "$prefix/records/$_release" ] && [ ! -L "$prefix/records/$_release" ] || fail 'release record missing or linked'
  verify_payload "$prefix/releases/$_release" "$(cat "$prefix/records/$_release")"
}
verify_manager() {
  for _name in manage.sh bin/rust-cache-inspect; do
    [ -f "$prefix/$_name" ] && [ ! -L "$prefix/$_name" ] || fail "missing or linked manager file: $_name"
    [ -f "$prefix/$_name.sha256" ] && [ ! -L "$prefix/$_name.sha256" ] || fail 'manager record missing or linked'
    [ "$(hash_file "$prefix/$_name")" = "$(cat "$prefix/$_name.sha256")" ] || fail "modified manager preserved; update refused: $_name"
  done
}
activate() {
  _new=$1
  if [ -f "$prefix/current" ]; then
    _old=$(read_pointer current)
    printf '%s\n' "$_old" > "$stage/previous"
    mv "$stage/previous" "$prefix/previous"
  fi
  printf '%s\n' "$_new" > "$stage/current"
  mv "$stage/current" "$prefix/current"
}
action=install; repo=4mGLn/rust-cache-management; version=; auto=false
prefix=${HOME:?HOME is required}/.local/share/rust-cache-management
case "${0##*/}" in manage.sh) prefix=$(CDPATH='' cd -- "$(dirname "$0")" && pwd -P); action=${1:-status}; [ "$#" -eq 0 ] || shift;; esac
while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo) [ "$#" -gt 1 ] || fail '--repo needs a value'; repo=$2; shift 2;;
    --prefix) [ "$#" -gt 1 ] || fail '--prefix needs a value'; prefix=${2%/}; shift 2;;
    --version) [ "$#" -gt 1 ] || fail '--version needs a value'; version=$2; shift 2;;
    --auto-update) auto=true; shift;;
    --help|-h) usage; exit 0;;
    *) fail "unknown argument: $1";;
  esac
done
plain_path "$prefix"
case "$(uname -s):$(uname -m)" in
  Linux:x86_64) target=x86_64-unknown-linux-gnu;;
  Linux:aarch64|Linux:arm64) target=aarch64-unknown-linux-gnu;;
  Darwin:x86_64) target=x86_64-apple-darwin;;
  Darwin:arm64|Darwin:aarch64) target=aarch64-apple-darwin;;
  *) fail 'unsupported OS or architecture';;
esac
binary=rust-cache-inspect
case "$action" in install|update|rollback|auto-on|auto-off|status) ;; *) fail "unknown action: $action";; esac
if [ "$action" = install ]; then
  valid_repo "$repo" || fail 'invalid repository; expected OWNER/REPO'
  [ ! -e "$prefix" ] || fail 'prefix already exists; use its manager or choose a new prefix'
  mkdir -p "$prefix"
else
  [ -f "$prefix/repository" ] && [ ! -L "$prefix/repository" ] || fail 'repository record missing or linked'
  repo=$(cat "$prefix/repository"); valid_repo "$repo" || fail 'invalid repository record'
fi
for _path in releases records bin current previous auto-update last-check repository manage.sh manage.sh.sha256; do
  [ ! -L "$prefix/$_path" ] || fail "linked managed path refused: $_path"
done
[ -z "$version" ] || valid_version "$version" || fail 'version must be vN.N.N'
mkdir "$prefix/.lock" 2>/dev/null || fail 'another manager is running or a stale .lock needs inspection'
stage=$(mktemp -d "$prefix/.stage.XXXXXXXX")
trap 'rm -rf "$stage"; rmdir "$prefix/.lock" 2>/dev/null || true; if [ "$action" = install ] && [ ! -f "$prefix/manage.sh" ]; then rmdir "$prefix/bin" "$prefix/records" "$prefix/releases" "$prefix" 2>/dev/null || true; fi' EXIT HUP INT TERM
if [ "$action" != install ]; then verify_manager; current=$(read_pointer current); verify_release "$current"; fi
case "$action" in
  status) printf 'Repository: %s\nCurrent release: %s\nSkill: %s/releases/%s/SKILL.md\n' "$repo" "$current" "$prefix" "$current"; if [ -f "$prefix/auto-update" ]; then printf 'Auto update: enabled, invocation checks at most daily\n'; else printf 'Auto update: disabled\n'; fi; exit 0;;
  auto-on) printf 'enabled\n' > "$stage/auto-update"; mv "$stage/auto-update" "$prefix/auto-update"; printf 'Enabled: check on launcher invocation, at most daily; no service or scheduled task.\n'; exit 0;;
  auto-off) rm -f "$prefix/auto-update"; printf 'Automatic updates disabled.\n'; exit 0;;
  rollback) previous=$(read_pointer previous); verify_release "$previous"; activate "$previous"; printf 'Activated retained release %s\n' "$previous"; exit 0;;
esac
command -v curl >/dev/null 2>&1 || fail 'curl is required'
command -v tar >/dev/null 2>&1 || fail 'tar is required'
if [ -z "$version" ]; then
  latest=$(curl --fail --silent --show-error --head --location --proto '=https' --proto-redir '=https' --connect-timeout 15 --max-time 60 --output /dev/null --write-out '%{url_effective}' "https://github.com/$repo/releases/latest")
  version=${latest##*/}; valid_version "$version" || fail 'latest stable release could not be resolved'
fi
release=$version-$target
if [ "$action" = update ]; then
  oldversion=${current%-"$target"}
  [ "$version" != "$oldversion" ] || { printf 'Already at %s\n' "$version"; exit 0; }
  awk -v a="${version#v}" -v b="${oldversion#v}" 'BEGIN {split(a,x,"."); split(b,y,"."); for(i=1;i<=3;i++){if(x[i]+0>y[i]+0)exit 0;if(x[i]+0<y[i]+0)exit 1}exit 1}' || fail 'update must advance the version; use rollback for a retained release'
fi
if [ -e "$prefix/releases/$release" ] || [ -e "$prefix/records/$release" ]; then
  verify_release "$release"
  activate "$release"
  printf 'Activated verified retained release %s\n' "$release"
  exit 0
fi
[ ! -L "$prefix/records/$release" ] || fail 'linked release record refused'
[ ! -e "$prefix/releases/$release" ] || fail 'release directory already exists; retained versions are never overwritten'
asset=rust-cache-management-$version-$target.tar.gz
base=https://github.com/$repo/releases/download/$version
get "$base/SHA256SUMS" "$stage/SHA256SUMS"
get "$base/$asset" "$stage/bundle.tar.gz"
expected=$(awk -v asset="$asset" '$2==asset {print $1}' "$stage/SHA256SUMS")
printf '%s\n' "$expected" | LC_ALL=C grep -Eq '^[0-9a-f]{64}$' || fail 'missing or ambiguous release checksum'
[ "$(hash_file "$stage/bundle.tar.gz")" = "$expected" ] || fail 'download checksum mismatch'
file_list > "$stage/expected"
printf '%s\n' MANIFEST.sha256 >> "$stage/expected"
LC_ALL=C sort "$stage/expected" > "$stage/expected-sorted"
tar -tzf "$stage/bundle.tar.gz" | LC_ALL=C sort > "$stage/members"
cmp -s "$stage/expected-sorted" "$stage/members" || fail 'archive members differ from the fixed payload allowlist'
mkdir -p "$stage/payload/bin" "$stage/payload/references"
while IFS= read -r member; do tar -xOzf "$stage/bundle.tar.gz" "$member" > "$stage/payload/$member"; done < "$stage/expected-sorted"
printf '%s\n' "$version" > "$stage/version-expected"
cmp -s "$stage/version-expected" "$stage/payload/VERSION" || fail 'payload version mismatch'
mkdir -p "$prefix/releases" "$prefix/records" "$prefix/bin"
# Verify the staged payload before adding any retained release or switching pointers.
manifest_hash=$(hash_file "$stage/payload/MANIFEST.sha256")
verify_payload "$stage/payload" "$manifest_hash"
printf '%s\n' "$manifest_hash" > "$stage/record"
mv "$stage/record" "$prefix/records/$release"
mv "$stage/payload" "$prefix/releases/$release"
verify_release "$release"
chmod 755 "$prefix/releases/$release/bin/$binary"
if [ "$action" = install ]; then
  cp "$prefix/releases/$release/install.sh" "$prefix/manage.sh"
  cat > "$prefix/bin/rust-cache-inspect" <<'LAUNCHER'
#!/bin/sh
set -eu
root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd -P)
# Explicit opt-in only: no persistent background process or scheduled task.
if [ -f "$root/auto-update" ] && [ ! -L "$root/auto-update" ]; then
  now=$(date +%s); last=0
  if [ -f "$root/last-check" ] && [ ! -L "$root/last-check" ]; then last=$(cat "$root/last-check"); fi
  case "$last" in ''|0[0-9]*|*[!0-9]*|????????????*) last=0;; esac
  if [ "$last" -gt "$now" ] || [ "$((now-last))" -ge 86400 ]; then
    if [ ! -L "$root/last-check" ]; then
      if (umask 077; set -C; printf '%s\n' "$now" > "$root/.check-$$") && mv "$root/.check-$$" "$root/last-check"; then
        if ! sh "$root/manage.sh" update >&2; then printf 'Update failed; retained diagnostic release will run.\n' >&2; fi
      else printf 'Update check state unavailable; retained diagnostic release will run.\n' >&2; fi
    fi
  fi
fi
[ -f "$root/current" ] && [ ! -L "$root/current" ] || { printf 'Invalid current pointer\n' >&2; exit 1; }
release=$(cat "$root/current")
case "$release" in ''|*[!A-Za-z0-9._-]*|*..*) printf 'Invalid current pointer\n' >&2; exit 1;; esac
[ ! -L "$root/releases" ] && [ ! -L "$root/releases/$release" ] && [ ! -L "$root/releases/$release/bin" ] && [ ! -L "$root/releases/$release/bin/rust-cache-inspect" ] || exit 1
exec "$root/releases/$release/bin/rust-cache-inspect" "$@"
LAUNCHER
  chmod 755 "$prefix/bin/rust-cache-inspect"
  hash_file "$prefix/manage.sh" > "$prefix/manage.sh.sha256"
  hash_file "$prefix/bin/rust-cache-inspect" > "$prefix/bin/rust-cache-inspect.sha256"
  printf '%s\n' "$repo" > "$prefix/repository"
fi
activate "$release"
if [ "$auto" = true ]; then printf 'enabled\n' > "$prefix/auto-update"; fi
printf 'Installed %s\nLauncher: %s/bin/rust-cache-inspect\nSkill: %s/releases/%s/SKILL.md\n' "$release" "$prefix" "$prefix" "$release"
