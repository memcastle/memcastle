#!/usr/bin/env python3
"""Select changed provider projects, not unrelated source packages or Core jobs."""

import json
import os
from pathlib import Path
import subprocess


def changes_contract(path: str) -> bool:
    """A module or installer contract change needs the generated archive checked against current Core."""
    return path.startswith((
        "wit/", "src/plugin/", "src/distribution/plugin.rs", "src/domain/plugin",
        "src/app/plugins.rs", "src/api/plugins.rs", "src/store/plugins.rs", "database/schema/plugin",
        "src/config/mod.rs", "src/integration/", "src/mining/", "src/source/",
        "src/domain/source_package.rs", "src/app/source_packages.rs",
        "database/schema/source_package.surql", ".github/workflows/plugins.yaml",
    ))


def select(paths: list[str], providers: list[str]) -> list[str]:
    """Run all providers for a changed contract, otherwise only touched provider roots."""
    shared = any(changes_contract(path) or path.startswith(("packaging/plugins/", "tests/wasm_plugin.rs")) for path in paths)
    return providers if shared else sorted({
        path.split("/")[1] for path in paths
        if path.startswith("plugins/") and path.split("/")[1] in providers
    })


def main() -> None:
    base = os.environ.get("BASE", "")
    head = os.environ.get("GITHUB_SHA", "HEAD")
    providers = sorted(path.name for path in Path("plugins").iterdir() if (path / "plugin.toml").is_file())
    if not base or set(base) == {"0"}:
        selected = providers
        contract = True
    else:
        paths = subprocess.run(
            ["git", "diff", "--name-only", base, head, "--"],
            check=True, capture_output=True, text=True,
        ).stdout.splitlines()
        selected = select(paths, providers)
        contract = any(map(changes_contract, paths))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            stream.write(f"providers={json.dumps(selected, separators=(',', ':'))}\n")
            stream.write(f"contract={'true' if contract else 'false'}\n")
    else:
        print(json.dumps(selected))


if __name__ == "__main__":
    main()
