#!/usr/bin/env bash
set -euo pipefail

directory=${1:-dist/release}
output="$directory/SHA256SUMS"
find "$directory" -maxdepth 1 -type f ! -name SHA256SUMS -print0 \
  | sort -z \
  | xargs -0 shasum -a 256 \
  | sed "s#  $directory/#  #" > "$output"
cat "$output"
