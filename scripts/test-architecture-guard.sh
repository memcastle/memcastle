#!/usr/bin/env bash
set -euo pipefail

# Native rule tests cannot test files/ignores globs or HTML's injected scripts.
# Use a throwaway checkout shape to exercise the actual hook without touching
# tracked sources (and without accidentally scanning the rule test snippets).
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/ast-grep" "$tmp/scripts" "$tmp/src/api" "$tmp/src/app" \
  "$tmp/src/server" "$tmp/src/domain" "$tmp/tests" "$tmp/web/src" \
  "$tmp/web/dist" "$tmp/web/node_modules" "$tmp/integrations/demo" \
  "$tmp/integrations/node_modules"
cp sgconfig.yml "$tmp/"
cp -R ast-grep/rules ast-grep/tests "$tmp/ast-grep/"
cp scripts/architecture-guard.sh "$tmp/scripts/"

check() {
  local result=$1 path=$2 source=$3
  printf '%s\n' "$source" > "$tmp/$path"
  if (cd "$tmp" && ARCHITECTURE_FIXTURE_RUN=1 bash scripts/architecture-guard.sh) > "$tmp/result" 2>&1; then
    if [[ $result != pass ]]; then
      printf 'Expected %s to fail the architecture hook\n' "$path" >&2
      return 1
    fi
  elif [[ $result != fail ]]; then
    printf 'Unexpected architecture failure for %s:\n' "$path" >&2
    cat "$tmp/result" >&2
    return 1
  fi
  rm "$tmp/$path"
}

check pass src/main.rs 'use memcastle::store::SurrealStore;'
check pass src/server/example.rs 'SurrealStore::connect(&backend);'
check pass src/domain/job.rs 'job.status = from;'
check pass web/src/clean.vue '<script setup lang="ts">// fetch("/api/db")</script>'
check pass web/dist/generated.js 'fetch("/api/db")'
check pass web/node_modules/vendor.js 'fetch("/api/db")'
check pass web/bun.lock 'surrealdb'
check pass integrations/README.md 'SurrealStore'
check pass integrations/node_modules/vendor.js 'fetch("/api/db")'
check fail src/api/violation.rs $'use crate::{\n  jobs::Scheduler,\n  app::AppServices,\n};'
check fail src/main.rs 'use memcastle::store::Other;'
check fail src/app/writer.rs 'SurrealStore::connect(&backend);'
check fail src/app/ddl.rs 'let sql = r#"DEFINE TABLE t SCHEMAFULL"#;'
check fail tests/violation.rs 'job.status = next;'
check fail integrations/demo/source.ts 'import { Surreal } from "surrealdb"'
check fail integrations/demo/source.tsx 'const View = () => <a href="/api/db">admin</a>'
check fail integrations/demo/source.js 'fetch("/api/db")'
check fail web/src/View.vue '<script setup lang="ts">import type { Db } from "surrealdb"</script>'
check fail web/src/View.vue '<script setup lang="ts">type Database = SurrealStore</script>'
check fail web/src/View.vue '<template><a href="/api/db">admin</a></template>'
check fail web/src/View.vue '<template><div>{{ fetch("/api/db") }}</div></template>'
check fail web/src/config.json '{"dependencies": {"surrealdb": "1.0"}}'
check fail web/src/View.tsx 'const View = () => <div>{SurrealStore.connect()}</div>'
check fail web/src/app.css 'a { background: url("/api/db") }'
check fail integrations/demo/config.toml 'url = "/api/db"'
