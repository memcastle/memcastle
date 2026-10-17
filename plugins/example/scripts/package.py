#!/usr/bin/env python3
"""Build this Core-owned fixture's source modules before Core packages them.

Run deliberately by a plugin author, never by GitTPL or by the MemCastle daemon.
Core owns manifest generation, archive validation and deterministic packaging.
"""

import os
from pathlib import Path
import shutil
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "plugin.toml"
def main() -> None:
    descriptor = tomllib.loads(MANIFEST.read_text())
    for module in descriptor.get("modules", []):
        name, kind = module["id"], module["type"]
        module_dir = ROOT / "modules" / name
        if kind == "source":
            if not (module_dir / "Cargo.lock").is_file():
                subprocess.run(["cargo", "generate-lockfile"], cwd=module_dir, check=True)
            cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).resolve()
            sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
            version = subprocess.check_output(["rustc", "--version", "--verbose"], text=True)
            commit = next(line.removeprefix("commit-hash: ") for line in version.splitlines() if line.startswith("commit-hash: "))
            build_env = os.environ.copy()
            # A local compiler wrapper and the runner's different Cargo home otherwise embed different paths in WASM.
            build_env["CARGO_BUILD_RUSTC_WRAPPER"] = ""
            build_env.pop("RUSTFLAGS", None)
            build_env["CARGO_ENCODED_RUSTFLAGS"] = "\x1f".join((
                f"--remap-path-prefix={cargo_home}=/cargo",
                f"--remap-path-prefix={sysroot / 'lib/rustlib/src/rust'}=/rustc/{commit}",
                f"--remap-path-prefix={module_dir}=/module",
            ))
            subprocess.run(["cargo", "build", "--locked", "--release", "--target", "wasm32-wasip2"], cwd=module_dir, env=build_env, check=True)
            built = module_dir / "target/wasm32-wasip2/release" / f"{name.replace('-', '_')}.wasm"
            if not built.is_file():
                raise ValueError(f"source {name!r} built no component at {built}")
            shutil.copyfile(built, safe_parent(module["entry"]))
            # The conformance runner uses each module's own `dist/source.wasm`; this is build output,
            # not another copy inside the plugin archive.
            test_component = module_dir / "dist/source.wasm"
            test_component.parent.mkdir(exist_ok=True)
            shutil.copyfile(built, test_component)
        elif kind == "integration":
            # Checked and collected by Core's packager, not by this build-only fixture script.
            continue
        else:
            raise ValueError(f"no packaging contract for module kind {kind!r}")
    subprocess.run(
        ["cargo", "run", "--quiet", "--manifest-path", str(ROOT.parents[1] / "Cargo.toml"),
         "--", "plugin", "package", str(MANIFEST)],
        check=True,
    )


def safe_parent(name: str) -> Path:
    """The sole file the build writes: source.wasm in the module's own directory."""
    parts = name.split("/")
    if len(parts) != 3 or parts[0] != "modules" or parts[1] in ("", ".", "..") or parts[2] != "source.wasm":
        raise ValueError(f"unsafe source component destination {name!r}")
    parent = ROOT
    for part in parts[:-1]:
        parent = parent / part
        if parent.is_symlink() or not parent.is_dir():
            raise ValueError(f"unsafe source component destination {name!r}")
    target = parent / "source.wasm"
    # `copyfile` follows a destination symlink, including one whose target is outside this plugin project.
    if target.is_symlink():
        raise ValueError(f"unsafe source component destination {name!r} is a symlink")
    return target


if __name__ == "__main__":
    main()
