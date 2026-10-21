#!/usr/bin/env bash
# Lay out Core's shared skills and any Core-shipped integrations as a local package tree.
#
# Usage: packaging/integrations/build.sh [out-dir]
#
# Always: prepares any declarative integrations still shipped by Core; provider-owned agents now arrive as plugins.
#
# With <out-dir>: also writes the tree an OS package installs under `share/memcastle/` -
#   <out-dir>/integrations/<id>/{memcastle-integration.toml,dist/,skills/ (only if the integration has its own)}
#   <out-dir>/skills/<name>/...
# and checks that nothing which must not ship came along. One script for 📦 Publish Release and for
# `mise run integrations:package`, so what a release bundles is what a developer can build and test.
#
# Core still packages the shared skills that plugins may reference; no agent-specific assets are bundled.
set -euo pipefail

if [ "$#" -gt 1 ]; then
  echo "usage: $0 [out-dir]" >&2
  exit 2
fi
out="${1:-}"

# Which integrations ship is a packaging decision and does not constrain the runtime (docs/adr/034): any directory
# under `integrations/` with a `memcastle-integration.toml` and a `dist/` could be added here.
BUNDLED=()

cd "$(dirname "$0")/../.."
if [ -z "${out}" ]; then
  exit 0
fi

mkdir -p "${out}"
out="$(realpath "${out}")"
# A stale tree from an earlier run would ship files that no longer exist in the sources.
rm -rf "${out}/integrations" "${out}/skills"
mkdir -p "${out}/integrations" "${out}/skills"

for id in "${BUNDLED[@]}"; do
  mkdir -p "${out}/integrations/${id}"
  cp "integrations/${id}/memcastle-integration.toml" "${out}/integrations/${id}/"
  cp -r "integrations/${id}/dist" "${out}/integrations/${id}/dist"
  # The skills only this integration exposes (`[[skills]]` with `local = true`) travel inside it; the shared ones below
  # are referenced by name from the manifest and are not duplicated here.
  if [ -d "integrations/${id}/skills" ]; then
    cp -r "integrations/${id}/skills" "${out}/integrations/${id}/skills"
  fi
done

# Only the skills themselves: `skills/README.md` is a working document of the repository.
for skill in skills/*/; do
  cp -r "${skill%/}" "${out}/skills/"
done

# What must never ship: the sources the bundle replaced, and their dependencies.
if find "${out}" \( -name node_modules -o -name '*.ts' -o -name bun.lock \) -print -quit | grep -q .; then
  echo "error: ${out} holds sources or dependencies that must not ship:" >&2
  find "${out}" \( -name node_modules -o -name '*.ts' -o -name bun.lock \) >&2
  exit 1
fi
