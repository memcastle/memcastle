#!/usr/bin/env bash
# Build the .deb and .rpm for every Linux binary staged in a directory.
#
# Usage: packaging/nfpm/build.sh <version> <dist-dir>
#
# Reads  <dist-dir>/memcastle_<version>_linux-{amd64,arm64}
# Reads  $SKILLS_DIR and $WEB_DIST_DIR (default: skills/ and web/dist/)
# Runs   the amd64 binary once, to print its shell completion scripts
# Writes <dist-dir>/memcastle_<version>_linux-{amd64,arm64}.{deb,rpm}
#
# One script for 📦 Publish Release and for CI's packaging check, so the check
# exercises exactly the code the release runs. The first release failed here
# (a `${BINARY}` that nfpm did not expand) because nothing ran this before a tag.
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <version> <dist-dir>" >&2
  exit 2
fi
version="$1"
dist="$2"

# Deterministic package timestamps; callers may set it, the commit time is the default.
SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"
export SOURCE_DATE_EPOCH

# Completion scripts come from running the binary, so they always match its
# commands and flags. They are plain text and identical on every architecture,
# so the amd64 binary (the only one a CI runner can execute) serves both
# packages. The staged file lost its execute bit in the artifact download.
completions="$(mktemp -d)"
trap 'rm -rf "${completions}"' EXIT
amd64="${dist}/memcastle_${version}_linux-amd64"
if [ ! -f "${amd64}" ]; then
  echo "error: ${amd64} is missing; stage the binaries before packaging" >&2
  exit 1
fi
chmod +x "${amd64}"
# Resolve the path so the shell never mistakes the executable for a PATH lookup.
runner="$(realpath "${amd64}")"
for shell in bash zsh fish; do
  "${runner}" completions "${shell}" > "${completions}/memcastle.${shell}"
  # A binary that prints nothing would ship a package whose completion file is
  # empty, and nothing else would notice.
  if [ ! -s "${completions}/memcastle.${shell}" ]; then
    echo "error: \`memcastle completions ${shell}\` printed nothing" >&2
    exit 1
  fi
done
export COMPLETIONS_DIR="${completions}"

# Package the actual inputs; an empty fallback tree would silently produce an incomplete release.
export SKILLS_DIR="${SKILLS_DIR:-skills}"
export WEB_DIST_DIR="${WEB_DIST_DIR:-web/dist}"
# Recovery may package an older binary that still requires the former nested layout.
export WEB_INSTALL_DIR="${WEB_INSTALL_DIR:-web}"
if [ ! -f "${WEB_DIST_DIR}/index.html" ]; then
  echo "error: ${WEB_DIST_DIR}/index.html is missing; run mise run web:build before packaging" >&2
  exit 1
fi

for asset in linux-amd64 linux-arm64; do
  binary="${dist}/memcastle_${version}_${asset}"
  # A missing binary would otherwise surface as nfpm's glob error, which names
  # the unexpanded template and not the file that is actually absent.
  if [ ! -f "${binary}" ]; then
    echo "error: ${binary} is missing; stage the binaries before packaging" >&2
    exit 1
  fi
  export VERSION="${version}"
  export BINARY="${binary}"
  # nfpm accepts Go architecture names and translates them for each package format.
  export NFPM_ARCH="${asset#linux-}"
  for format in deb rpm; do
    # Recovery runs repaired tooling beside tagged sources; its config must come from the same revision as the script.
    nfpm package --config "$(dirname "$0")/nfpm.yaml" --packager "${format}" \
      --target "${dist}/memcastle_${version}_${asset}.${format}"
  done
done
