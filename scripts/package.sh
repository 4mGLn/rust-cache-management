#!/bin/sh
# Build one fixed-layout release bundle from an already built native binary.
set -eu
umask 077
root=$(CDPATH='' cd -- "$(dirname "$0")/.." && pwd -P)
[ "$#" -eq 4 ] || { printf 'Usage: package.sh VERSION TARGET BINARY OUTPUT_DIRECTORY\n' >&2; exit 2; }
version=$1; target=$2; binary=$3; out=$4
printf '%s\n' "$version" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || exit 2
case "$target" in x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu|x86_64-apple-darwin|aarch64-apple-darwin) name=rust-cache-inspect;; x86_64-pc-windows-msvc) name=rust-cache-inspect.exe;; *) printf 'Unsupported target\n' >&2; exit 2;; esac
[ -f "$binary" ] && [ ! -L "$binary" ] || { printf 'Regular binary required\n' >&2; exit 2; }
mkdir -p "$out"
out=$(CDPATH='' cd -- "$out" && pwd -P)
stage=$(mktemp -d "$out/.package.XXXXXXXX")
trap 'rm -rf "$stage"' EXIT HUP INT TERM
mkdir -p "$stage/bin" "$stage/references"
cp "$binary" "$stage/bin/$name"
chmod 755 "$stage/bin/$name"
for file in AGENTS-snippet.md CHANGELOG.md CONTRIBUTING.md LICENSE README.md SKILL.md THIRD-PARTY-NOTICES install.sh install.ps1 references/defaults.md references/distribution.md references/inspection.md references/policy.md; do cp "$root/$file" "$stage/$file"; done
printf '%s\n' "$version" > "$stage/VERSION"
(cd "$stage"; find . -type f | sed 's|^./||' | LC_ALL=C sort) > "$out/.members-$$"
trap 'rm -rf "$stage"; rm -f "$out/.members-$$"' EXIT HUP INT TERM
while IFS= read -r file; do
  if command -v sha256sum >/dev/null 2>&1; then hash=$(sha256sum "$stage/$file" | cut -d ' ' -f 1); else hash=$(shasum -a 256 "$stage/$file" | cut -d ' ' -f 1); fi
  printf '%s  %s\n' "$hash" "$file" >> "$stage/MANIFEST.sha256"
done < "$out/.members-$$"
printf '%s\n' MANIFEST.sha256 >> "$out/.members-$$"
asset=rust-cache-management-$version-$target.tar.gz
[ ! -e "$out/$asset" ] && [ ! -L "$out/$asset" ] && [ ! -e "$out/$asset.sha256" ] && [ ! -L "$out/$asset.sha256" ] || { printf 'Refusing to overwrite existing bundle\n' >&2; exit 1; }
# -T contains fixed names only; no archive directories, symlinks, or absolute paths.
tar -czf "$out/$asset" -C "$stage" -T "$out/.members-$$"
if command -v sha256sum >/dev/null 2>&1; then hash=$(sha256sum "$out/$asset" | cut -d ' ' -f 1); else hash=$(shasum -a 256 "$out/$asset" | cut -d ' ' -f 1); fi
printf '%s  %s\n' "$hash" "$asset" > "$out/$asset.sha256"
printf '%s\n' "$out/$asset"
