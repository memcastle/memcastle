#!/usr/bin/env bash
set -euo pipefail

# The rules own supported source languages. Keep the rule fixtures in the same
# hook so CI rejects a rule edit that silently stops finding a violation.
ast-grep test --skip-snapshot-tests
ast-grep scan --report-style short
# The project fixtures also verify file globs, excluded assets, and Vue
# injection; do not recurse into these tests from their disposable copy.
if [[ ${ARCHITECTURE_FIXTURE_RUN:-} != 1 ]]; then
  bash scripts/test-architecture-guard.sh
fi

# ast-grep has no parser for every future integration asset (CSS, TOML, shell,
# etc.). Retain the old text guard *only* for those unparsed file types rather
# than silently dropping the original all-files scope. The AST rules handle
# comments in parsed code; this fallback keeps the old text semantics.
pattern='surrealdb|surrealkv|SurrealStore|(^|[^A-Za-z0-9_])(crate|memcastle)::(\{[^}]*)?(store|jobs)\b|/api/db\b'
for root in integrations web; do
  globs=(--glob '!*.md' --glob '!**/node_modules/**'
    --glob '!*.rs' --glob '!*.ts' --glob '!*.tsx' --glob '!*.js'
    --glob '!*.jsx' --glob '!*.vue' --glob '!*.html' --glob '!*.json')
  if [[ $root == web ]]; then
    globs+=(--glob '!**/dist/**' --glob '!bun.lock')
  fi
  if rg --hidden -n "${globs[@]}" "$pattern" "$root"; then
    printf 'Architecture violation in an unparsed %s file.\n' "$root" >&2
    exit 1
  else
    code=$?
    # No matches is success, but a bad regex or unreadable tree must fail CI.
    if [[ $code != 1 ]]; then exit "$code"; fi
  fi
done
