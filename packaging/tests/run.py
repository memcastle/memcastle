"""Own prepared database fixtures until all test processes have exited."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: run.py <test command> [arguments...]")
    # The parent owns cleanup: a test process cannot delete a template that
    # later nextest processes still need, especially with Windows file locks.
    with tempfile.TemporaryDirectory(prefix="memcastle-test-fixtures-") as root:
        Path(root, ".owner").write_bytes(b"memcastle-test-fixtures-v1\n")
        env = dict(os.environ, MEMCASTLE_TEST_FIXTURE_ROOT=str(Path(root).resolve()))
        with subprocess.Popen(sys.argv[1:], env=env) as child:
            try:
                return child.wait()
            except BaseException:
                child.terminate()
                try:
                    child.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
                raise


if __name__ == "__main__":
    raise SystemExit(main())
