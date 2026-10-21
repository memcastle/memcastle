#!/usr/bin/env bash
# Archive Core's shared skills for package managers that install the raw release binary.
# Usage: packaging/skills/archive.sh <packaged-assets-dir> <release-assets-dir> <tag>
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <packaged-assets-dir> <release-assets-dir> <tag>" >&2
  exit 2
fi
assets="$1"
dist="$2"
tag="$3"
if [[ ! "$tag" =~ ^[[:alnum:]._-]+$ ]] || [ ! -d "$assets/skills" ] || [ ! -d "$dist" ]; then
  echo "error: expected a release tag, packaged skills directory and release assets directory" >&2
  exit 1
fi

# An empty archive would pass tar but leave AUR and Homebrew with no shared skills.
shopt -s nullglob
skills=("$assets"/skills/*/SKILL.md)
if [ "${#skills[@]}" -eq 0 ]; then
  echo "error: no shared skills to package under $assets/skills" >&2
  exit 1
fi

export LC_ALL=C
epoch="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
name="memcastle-skills-$tag"
install -d -m 755 "$stage/$name/skills"
cp -r "$assets/skills/." "$stage/$name/skills/"
find "$stage/$name" -type d -exec chmod 755 {} +
find "$stage/$name" -type f -exec chmod 644 {} +

# Fixed order, timestamp, ownership and gzip header make rebuilding this asset deterministic.
tar -C "$stage" --sort=name --format=gnu --mtime="@$epoch" \
  --owner=0 --group=0 --numeric-owner -cf - "$name" \
  | gzip -n -9 > "$dist/memcastle_${tag}_skills.tar.gz"
