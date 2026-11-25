"""Own prepared database fixtures until all test processes have exited."""

import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile


def stop_tree(child):
    # Cargo is only the immediate child. Its nextest/test descendants may still
    # own databases after Cargo itself has stopped.
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(child.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
        )
    else:
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    child.wait(timeout=30)


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: run.py <test command> [arguments...]")
    # The parent owns cleanup: a test process cannot delete a template that
    # later nextest processes still need, especially with Windows file locks.
    root = Path(tempfile.mkdtemp(prefix="memcastle-test-fixtures-")).resolve()
    root.joinpath(".owner").write_bytes(b"memcastle-test-fixtures-v1\n")
    env = dict(os.environ, MEMCASTLE_TEST_FIXTURE_ROOT=str(root))
    options = (
        {"creationflags": subprocess.CREATE_NEW_PROCESS_GROUP}
        if os.name == "nt" else {"start_new_session": True}
    )
    try:
        with subprocess.Popen(sys.argv[1:], env=env, **options) as child:
            try:
                status = child.wait()
            except BaseException:
                stop_tree(child)
                raise
    except BaseException:
        # Detached descendants may outlive cancellation. Retaining a disposable
        # root is preferable to deleting a database still held by one of them.
        print(f"Interrupted test invocation; fixtures retained at {root}", file=sys.stderr)
        raise
    if status < 0:
        stop_tree(child)
        print(f"Terminated test invocation; fixtures retained at {root}", file=sys.stderr)
        return 128 - status if os.name != "nt" else status
    shutil.rmtree(root)
    return status


if __name__ == "__main__":
    raise SystemExit(main())
