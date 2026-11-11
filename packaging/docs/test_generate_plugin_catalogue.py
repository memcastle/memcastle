"""Behavioural checks for catalogue builds, independent of the current inventory."""

import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from generate_plugin_catalogue import CatalogueError, ROOT, load, prose, render, validate


def catalogue():
    return {
        "format": 1,
        "name": "Example catalogue",
        "plugins": [{
            "id": "example",
            "description": "Example history and integration.",
            "repository": "acme/example",
            "modules": [
                {"id": "example", "type": kind}
                for kind in ("source", "integration", "embedding_provider")
            ],
        }],
    }


def version():
    return {
        "version": "1.2.3-rc.1+build.2",
        "url": "example.tar.gz",
        "sha256": "a" * 64,
        "signature": {"key": "signer", "value": "signature"},
        "modules": [{"id": "older", "type": "source"}],
        "yanked": False,
    }


class CatalogueTests(unittest.TestCase):
    def test_plugin_names_and_repositories_are_linked_with_each_module_type_visible(self):
        data = validate(catalogue())
        page = render(data)
        self.assertIn("## [example](https://github.com/acme/example)", page)
        self.assertIn("[acme/example](https://github.com/acme/example)", page)
        self.assertIn(prose(data["plugins"][0]["description"]), page)
        for module in data["plugins"][0]["modules"]:
            self.assertIn(f"| `{module['id']}` | `{module['type']}` |", page)
        self.assertIn("(plugins.json)", page)
        self.assertIn("(plugins.md)", page)
        self.assertEqual(page, render(data))
        self.assertNotIn("1.2.3", page)

    def test_empty_catalogues_and_module_inventories_explain_their_empty_state(self):
        data = catalogue()
        data["plugins"][0]["modules"] = []
        self.assertIn("No modules are listed", render(validate(data)))
        data["plugins"] = []
        self.assertIn("No plugins are listed", render(validate(data)))

    def test_descriptions_cannot_inject_markdown_html_or_new_sections(self):
        data = catalogue()
        data["plugins"][0]["description"] = "<script>alert(1)</script>\n\n# [link](https://evil.test) *bold* | cell"
        page = render(validate(data))
        self.assertNotIn("<script>", page)
        self.assertNotIn("[link]", page)
        self.assertNotIn("*bold*", page)
        self.assertNotIn("\n# [link]", page)
        self.assertIn("&#60;script&#62;", page)
        self.assertIn("&#124; cell", page)

    def test_every_current_catalogue_entry_is_rendered_without_an_inventory_copy(self):
        data = load(ROOT / "docs/plugins.json")
        page = render(data)
        for plugin in data["plugins"]:
            self.assertIn(f"[{plugin['id']}](https://github.com/{plugin['repository']})", page)
            for module in plugin["modules"]:
                self.assertIn(f"| `{module['id']}` | `{module['type']}` |", page)

    def test_malformed_catalogue_fields_report_the_offending_path(self):
        cases = [
            ((), [], "$"),
            (("format",), True, "$.format"),
            (("format",), 2, "$.format"),
            (("name",), None, "$.name"),
            (("plugins",), {}, "$.plugins"),
            (("plugins", 0), None, "$.plugins[0]"),
            (("plugins", 0, "id"), "../bad", "$.plugins[0].id"),
            (("plugins", 0, "id"), "x" * 49, "$.plugins[0].id"),
            (("plugins", 0, "description"), "  ", "$.plugins[0].description"),
            (("plugins", 0, "description"), 3, "$.plugins[0].description"),
            (("plugins", 0, "repository"), "https://github.com/acme/example", "$.plugins[0].repository"),
            (("plugins", 0, "repository"), "acme/..", "$.plugins[0].repository"),
            (("plugins", 0, "modules"), None, "$.plugins[0].modules"),
            (("plugins", 0, "modules", 0, "type"), "unknown", "$.plugins[0].modules[0].type"),
            (("plugins", 0, "modules", 0, "type"), [], "$.plugins[0].modules[0].type"),
            (("plugins", 0, "modules", 0, "id"), "-bad", "$.plugins[0].modules[0].id"),
            (("plugins", 0, "versions"), None, "$.plugins[0].versions"),
        ]
        for path, replacement, diagnostic in cases:
            with self.subTest(path=path, replacement=replacement):
                data = catalogue()
                if not path:
                    data = replacement
                else:
                    target = data
                    for key in path[:-1]:
                        target = target[key]
                    target[path[-1]] = replacement
                with self.assertRaises(CatalogueError) as error:
                    validate(data)
                self.assertIn(diagnostic, str(error.exception))

    def test_missing_and_unknown_fields_are_rejected(self):
        for path in ((), ("plugins", 0), ("plugins", 0, "modules", 0)):
            for change in ("missing", "unknown"):
                with self.subTest(path=path, change=change):
                    data = catalogue()
                    target = data
                    for key in path:
                        target = target[key]
                    if change == "missing":
                        del target[next(iter(target))]
                    else:
                        target["typo"] = "oops"
                    with self.assertRaisesRegex(CatalogueError, "required fields|unknown fields"):
                        validate(data)

    def test_duplicate_plugin_and_same_type_module_identities_are_rejected(self):
        data = catalogue()
        validate(data)  # Shared IDs across different types are valid.
        data["plugins"].append(copy.deepcopy(data["plugins"][0]))
        with self.assertRaisesRegex(CatalogueError, "unique plugin ID"):
            validate(data)
        data = catalogue()
        data["plugins"][0]["modules"].append(copy.deepcopy(data["plugins"][0]["modules"][0]))
        with self.assertRaisesRegex(CatalogueError, "unique .*module identity"):
            validate(data)

    def test_static_versions_are_validated_without_becoming_inferred_page_metadata(self):
        data = catalogue()
        data["plugins"][0]["versions"] = [version()]
        self.assertNotIn("1.2.3", render(validate(data)))
        for field, invalid in (
            ("version", "01.2.3"), ("version", "1.2"), ("version", "1.2.3-01"),
            ("version", "18446744073709551616.0.0"), ("url", ""), ("sha256", "F" * 64),
            ("signature", {"key": 3, "value": "text"}), ("signature", {"unknown": "text"}),
            ("modules", [{"id": "-invalid", "type": "source"}]), ("yanked", 1),
        ):
            with self.subTest(field=field, invalid=invalid):
                entry = version()
                entry[field] = invalid
                data["plugins"][0]["versions"] = [entry]
                with self.assertRaises(CatalogueError) as error:
                    validate(data)
                self.assertIn("$.plugins[0].versions[0]", str(error.exception))
        data["plugins"][0]["versions"] = [version(), version()]
        with self.assertRaisesRegex(CatalogueError, "unique version"):
            validate(data)
        data["plugins"][0]["versions"] = [{**version(), "modules": None, "signature": None}]
        validate(data)

    def test_cli_failures_explain_the_input_and_preserve_the_previous_page(self):
        script = Path(__file__).with_name("generate_plugin_catalogue.py")
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "plugins.json"
            output = Path(directory) / "page.md"
            output.write_text("previous page", encoding="utf-8")
            for content, diagnostic in (("{", "invalid JSON"), ('{"format": 2}', "required fields")):
                with self.subTest(content=content):
                    source.write_text(content, encoding="utf-8")
                    result = subprocess.run(
                        [sys.executable, "-B", str(script), "--source", str(source), "--output", str(output)],
                        capture_output=True, text=True, check=False,
                    )
                    self.assertEqual(result.returncode, 1)
                    self.assertIn(str(source), result.stderr)
                    self.assertIn(diagnostic, result.stderr)
                    self.assertNotIn("Traceback", result.stderr)
                    self.assertEqual(output.read_text(encoding="utf-8"), "previous page")
            source.write_text(json.dumps(catalogue()), encoding="utf-8")
            result = subprocess.run(
                [sys.executable, "-B", str(script), "--source", str(source), "--output", str(output)],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("# Plugin catalogue", output.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
