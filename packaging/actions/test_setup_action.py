"""Exercise the public setup action itself, with offline GitHub responses."""

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
PAYLOAD = b"verified Core executable\n"
SHA = "a" * 40

# Stub only the network boundary: the action's real jq and checksum commands still run.
GH = r'''
import hashlib, json, os, pathlib, sys
root = pathlib.Path(os.environ["FIXTURE"])
case = json.loads((root / "case.json").read_text())
args = sys.argv[1:]
with (root / "calls").open("a") as output:
    output.write(json.dumps(args) + "\n")
if args[0] == "api":
    endpoint = next(arg for arg in args if arg.startswith("repos/"))
    if "?per_page=" in endpoint:
        print(json.dumps(case.get("pages", [])))
    else:
        counter = root / "reads"
        reads = int(counter.read_text()) if counter.exists() else 0
        counter.write_text(str(reads + 1))
        sha = "b" * 40 if reads and case.get("changed") else "a" * 40
        metadata = {"draft": False, "prerelease": True,
            "body": "Core main SHA: " + sha,
            "assets": [{"name": "memcastle_linux-amd64", "label": sha}]}
        metadata.update(case.get("metadata", {}))
        if reads and case.get("changed_body"): metadata["body"] = "Core main SHA: " + "b" * 40
        if reads and case.get("changed_label"): metadata["assets"][0]["label"] = "b" * 40
        print(json.dumps(metadata))
else:
    assert args[:2] == ["release", "download"], args
    dest = pathlib.Path(args[args.index("--dir") + 1])
    names = [args[index + 1] for index, arg in enumerate(args) if arg == "--pattern"]
    name = next(name for name in names if name != "SHA256SUMS")
    payload = b"verified Core executable\n"
    if case.get("missing") != "binary":
        (dest / name).write_bytes(payload)
    if case.get("missing") != "manifest":
        digest = "0" * 64 if case.get("tampered") else hashlib.sha256(payload).hexdigest()
        member = "another-asset" if case.get("missing") == "entry" else name
        (dest / "SHA256SUMS").write_text(digest + "  " + member + "\n")
'''


def release(tag, **flags):
    return {"tag_name": tag, "draft": False, "prerelease": False, **flags}


class SetupActionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.action = json.loads(subprocess.check_output(["yq", "-o", "json", ".", str(ROOT / "action.yml")]))
        cls.script = cls.action["runs"]["steps"][0]["run"]

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "bin").mkdir()
        gh = self.root / "bin/gh"
        gh.write_text(f"#!{sys.executable}\n" + GH)
        gh.chmod(0o755)

    def run_action(self, version, **case):
        (self.root / "case.json").write_text(json.dumps(case))
        env = dict(os.environ, VERSION=version, FIXTURE=str(self.root), RUNNER_TEMP=str(self.root),
                   PATH=str(self.root / "bin") + os.pathsep + os.environ["PATH"])
        for name in ("OUTPUT", "ENV", "PATH"):
            env["GITHUB_" + name] = str(self.root / name)
        return subprocess.run(["bash", "-euo", "pipefail", "-c", self.script], env=env,
                              capture_output=True, text=True)

    def outputs(self):
        return dict(line.split("=", 1) for line in (self.root / "OUTPUT").read_text().splitlines())

    def test_an_exact_release_is_verified_and_exposed_directly_from_its_unique_directory(self):
        result = self.run_action("0.5.2")
        self.assertEqual(result.returncode, 0, result.stderr)
        binary = Path(self.outputs()["binary"])
        self.assertEqual(binary.name, "memcastle")
        self.assertFalse(binary.is_symlink())
        self.assertEqual(binary.read_bytes(), PAYLOAD)
        self.assertTrue(os.access(binary, os.X_OK))
        self.assertEqual((self.root / "ENV").read_text(), f"MEMCASTLE_BIN={binary}\n")
        self.assertEqual((self.root / "PATH").read_text(), f"{binary.parent}\n")
        calls = [json.loads(line) for line in (self.root / "calls").read_text().splitlines()]
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].count("--pattern"), 2)

    def test_release_trees_compare_numbers_across_pages_and_ignore_ineligible_releases(self):
        pages = [[release("1.2.9"), release("1.20.0"), release("10.2.0")],
                 [release("1.2.10"), release("1.2.99", draft=True),
                  release("1.2.98", prerelease=True), release("provider-1.2.100")]]
        result = self.run_action("1.2", pages=pages)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.outputs()["version"], "1.2.10")

    def test_a_major_tree_selects_the_highest_minor(self):
        result = self.run_action("1", pages=[[release("1.9.0"), release("1.10.0"), release("10.0.0")]])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.outputs()["version"], "1.10.0")

    def test_no_matching_stable_release_fails_before_any_download(self):
        self.assertNotEqual(self.run_action("2", pages=[[release("1.2.3")]]).returncode, 0)
        self.assertFalse((self.root / "OUTPUT").exists())

    def test_missing_assets_or_selected_checksum_entries_never_expose_an_executable(self):
        for missing in ("binary", "manifest", "entry"):
            with self.subTest(missing=missing):
                result = self.run_action("0.5.2", missing=missing)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / "OUTPUT").exists())

    def test_a_checksum_mismatch_is_refused(self):
        self.assertNotEqual(self.run_action("0.5.2", tampered=True).returncode, 0)
        self.assertFalse((self.root / "OUTPUT").exists())

    def test_development_metadata_is_checked_again_after_download(self):
        for field in ("changed_body", "changed_label"):
            with self.subTest(field=field):
                (self.root / "reads").unlink(missing_ok=True)
                result = self.run_action("development", **{field: True})
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("changed during download", result.stdout + result.stderr)

    def test_invalid_development_metadata_never_exposes_an_executable(self):
        for metadata in ({"draft": True}, {"prerelease": False}, {"body": "no source SHA"},
                         {"body": "Core main SHA: invalid"}, {"assets": []},
                         {"assets": [{"name": "memcastle_linux-amd64", "label": "wrong"}]}):
            with self.subTest(metadata=metadata):
                result = self.run_action("development", metadata=metadata)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((self.root / "OUTPUT").exists())

    def test_development_reports_its_source_sha(self):
        result = self.run_action("development")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.outputs()["core_sha"], SHA)

    def test_repeated_setup_does_not_retarget_a_previously_returned_executable(self):
        self.assertEqual(self.run_action("0.5.2").returncode, 0)
        first = self.outputs()["binary"]
        self.assertEqual(self.run_action("0.5.2").returncode, 0)
        self.assertNotEqual(first, self.outputs()["binary"])
        self.assertTrue(Path(first).exists())


if __name__ == "__main__":
    unittest.main()
