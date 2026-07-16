#!/usr/bin/env bash
set -euo pipefail

app=${1:?usage: bundle-macos-libs.sh /path/Player.app /path/to/pinned-prefix}
prefix=${2:?usage: bundle-macos-libs.sh /path/Player.app /path/to/pinned-prefix}
binary="$app/Contents/MacOS/tonelag"
frameworks="$app/Contents/Frameworks"

[[ $(uname -s) == Darwin ]] || { echo "macOS is required" >&2; exit 1; }
[[ -x "$binary" ]] || { echo "application binary not found: $binary" >&2; exit 1; }
[[ -d "$prefix" ]] || { echo "native library prefix not found: $prefix" >&2; exit 1; }
mkdir -p "$frameworks"

declare -a pending=("$binary")
declare -a bundled=()

dependencies() {
  otool -L "$1" | tail -n +2 | awk '{ print $1 }'
}

for ((index = 0; index < ${#pending[@]}; index++)); do
  owner=${pending[$index]}
  while IFS= read -r dependency; do
    case "$dependency" in
      /System/*|/usr/lib/*|@*) continue ;;
      "$prefix"/*) ;;
      *)
        echo "refusing non-system dependency outside pinned prefix: $dependency" >&2
        exit 1
        ;;
    esac

    name=$(basename "$dependency")
    destination="$frameworks/$name"
    if [[ ! -f "$destination" ]]; then
      cp -L "$dependency" "$destination"
      chmod u+w "$destination"
      pending+=("$destination")
      bundled+=("$destination")
    fi
  done < <(dependencies "$owner")
done

((${#bundled[@]} > 0)) || { echo "no pinned dynamic libraries were discovered" >&2; exit 1; }

for library in "${bundled[@]}"; do
  install_name_tool -id "@rpath/$(basename "$library")" "$library"
done

for owner in "$binary" "${bundled[@]}"; do
  while IFS= read -r dependency; do
    name=$(basename "$dependency")
    if [[ -f "$frameworks/$name" && "$dependency" != "@rpath/$name" ]]; then
      install_name_tool -change "$dependency" "@rpath/$name" "$owner"
    fi
  done < <(dependencies "$owner")
done

if ! otool -l "$binary" | grep -q '@executable_path/../Frameworks'; then
  install_name_tool -add_rpath '@executable_path/../Frameworks' "$binary"
fi

codesign --remove-signature "$binary" 2>/dev/null || true
for library in "${bundled[@]}"; do
  codesign --remove-signature "$library" 2>/dev/null || true
done

echo "Bundled ${#bundled[@]} pinned dynamic libraries in $frameworks"
