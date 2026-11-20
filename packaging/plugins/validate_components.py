"""Compile and validate Core's component fixtures against the WASI 0.3 baseline."""

import pathlib
import re
import subprocess
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parents[2]
PROJECTS = (
    "sources/directory",
    "tests/fixtures/mock-source",
    "tests/fixtures/mock-embedding",
    "plugins/example/modules/files-one",
    "plugins/example/modules/files-two",
)


def validate(component: pathlib.Path) -> None:
    subprocess.run(
        ["wasm-tools", "validate", "--features", "cm-async,cm-async-stackful,cm-more-async-builtins", str(component)],
        check=True,
    )
    wit = subprocess.check_output(["wasm-tools", "component", "wit", str(component)], text=True)
    # Only the root world's imports describe the artifact; later packages describe interface definitions.
    world = wit.split("world root {", 1)[1].split("\n}", 1)[0]
    imports = re.findall(r"^\s*import (wasi:[^;]+);", world, re.MULTILINE)
    if not imports or any(not name.endswith("@0.3.0") for name in imports):
        raise ValueError(f"{component}: expected WASI 0.3.0 imports, got {imports}")
    print(f"Validated {component}: {len(imports)} WASI 0.3.0 imports", flush=True)


def main() -> None:
    components = [pathlib.Path(argument) for argument in sys.argv[1:]]
    if not components:
        # Match tests/common/wasm.rs so the execution tests reuse these compiled guest dependencies.
        target = ROOT / "target/tmp/source-projects"
        for project in PROJECTS:
            manifest = ROOT / project / "Cargo.toml"
            package = tomllib.loads(manifest.read_text())["package"]["name"]
            subprocess.run(
                ["cargo", "build", "--locked", "--target", "wasm32-wasip3", "--target-dir", str(target),
                 "--manifest-path", str(manifest)],
                check=True,
            )
            components.append(target / "wasm32-wasip3/debug" / f"{package.replace('-', '_')}.wasm")
    for component in components:
        validate(component)


if __name__ == "__main__":
    main()
