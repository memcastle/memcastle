#!/usr/bin/env bash
# Add separately published source archives to a release, when Core bundles any.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <bundled-sources-dir> <release-assets-dir>" >&2
  exit 2
fi

archives="$1/archives"
dist="$2"
if [ ! -d "$archives" ] || [ ! -d "$dist" ]; then
  echo "error: expected the source archives and release assets directories to exist" >&2
  exit 1
fi

# An empty Core bundle has no registry assets; an unmatched glob passed to cp would abort the release.
shopt -s nullglob
files=("$archives"/*)
if [ "${#files[@]}" -gt 0 ]; then
  cp -- "${files[@]}" "$dist/"
fi
