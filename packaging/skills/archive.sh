#!/usr/bin/env bash
# Archive shared skills directly from the checkout, without an intermediate asset tree.
# Usage: packaging/skills/archive.sh <assets-root> <release-assets-dir> <tag>
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <assets-root> <release-assets-dir> <tag>" >&2
  exit 2
fi
assets="$1"
dist="$2"
tag="$3"
if [[ ! "$tag" =~ ^[[:alnum:]._-]+$ ]] || [ ! -d "$assets/skills" ] || [ ! -d "$dist" ]; then
  echo "error: expected a release tag, assets root with skills and release assets directory" >&2
  exit 1
fi

# Select directories rather than shipping the repository's skills/README.md.
shopt -s nullglob
skills=("$assets"/skills/*/)
if [ "${#skills[@]}" -eq 0 ]; then
  echo "error: no shared skills to package under $assets/skills" >&2
  exit 1
fi
members=()
for skill in "${skills[@]}"; do
  if [ ! -f "${skill}SKILL.md" ]; then
    echo "error: ${skill}SKILL.md is missing; every packaged skill needs its instructions" >&2
    exit 1
  fi
  members+=("skills/$(basename "$skill")")
done

export LC_ALL=C
epoch="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
name="memcastle-skills-$tag"
# Fixed metadata and a path transform preserve the release layout without copying the inputs.
tar -C "$assets" --sort=name --format=gnu --mtime="@$epoch" \
  --owner=0 --group=0 --numeric-owner --mode='u=rwX,go=rX' \
  --transform="s|^|$name/|" -cf - "${members[@]}" \
  | gzip -n -9 > "$dist/memcastle_${tag}_skills.tar.gz"
