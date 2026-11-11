"""Validate the reviewed catalogue and render its documentation without network access."""

import argparse
import json
from pathlib import Path
import re
import sys
import textwrap


ROOT = Path(__file__).resolve().parents[2]
MODULE_TYPES = {"source", "integration", "embedding_provider"}
# Keep these rules aligned with domain/plugin_index.rs and its source-package helpers.
IDENTIFIER = re.compile(r"[a-z0-9](?:[a-z0-9-]{0,46}[a-z0-9])?")
REPOSITORY = re.compile(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+")
NUMBER = r"(?:0|[1-9][0-9]*)"
# Semver disallows leading zeroes in numeric prerelease identifiers, but allows them in alphanumeric ones.
PRERELEASE = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER = re.compile(
    rf"({NUMBER})\.({NUMBER})\.({NUMBER})"
    rf"(?:-{PRERELEASE}(?:\.{PRERELEASE})*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)


class CatalogueError(ValueError):
    """An invalid field with enough context for the catalogue editor to fix it."""


def require(condition, path, expected):
    if not condition:
        raise CatalogueError(f"{path}: expected {expected}; correct the catalogue and retry")


def object_fields(value, path, required, optional=()):
    require(isinstance(value, dict), path, "an object")
    missing = set(required) - value.keys()
    unknown = value.keys() - set(required) - set(optional)
    require(not missing, path, f"required fields {', '.join(sorted(missing))}")
    require(not unknown, path, f"no unknown fields ({', '.join(sorted(unknown))})")


def string(value, path):
    require(isinstance(value, str), path, "a string")


def identifier(value, path):
    require(
        isinstance(value, str) and IDENTIFIER.fullmatch(value) is not None,
        path,
        "an ID of 1–48 lowercase letters, digits or hyphens, without leading/trailing hyphens",
    )


def modules(value, path):
    require(isinstance(value, list), path, "an array of modules")
    seen = set()
    for index, module in enumerate(value):
        item = f"{path}[{index}]"
        object_fields(module, item, ("id", "type"))
        identifier(module["id"], f"{item}.id")
        require(
            isinstance(module["type"], str) and module["type"] in MODULE_TYPES,
            f"{item}.type",
            "source, integration or embedding_provider",
        )
        # Different module types may legitimately share an ID, as Pi's modules do.
        identity = (module["type"], module["id"])
        require(identity not in seen, item, "a unique (type, id) module identity")
        seen.add(identity)


def versions(value, path):
    require(isinstance(value, list), path, "an array of versions")
    seen = set()
    for index, version in enumerate(value):
        item = f"{path}[{index}]"
        object_fields(version, item, ("version", "url", "sha256"), ("signature", "modules", "yanked"))
        string(version["version"], f"{item}.version")
        match = SEMVER.fullmatch(version["version"])
        # Rust's semver parser uses u64 components; bound length before Python converts untrusted digits.
        require(
            match is not None and all(len(part) <= 20 and int(part) <= 2**64 - 1 for part in match.groups()),
            f"{item}.version",
            "a semantic version (for example, 1.2.3)",
        )
        require(version["version"] not in seen, f"{item}.version", "a unique version")
        seen.add(version["version"])
        string(version["url"], f"{item}.url")
        require(bool(version["url"]), f"{item}.url", "a nonempty archive URL")
        require(
            isinstance(version["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", version["sha256"]),
            f"{item}.sha256",
            "64 lowercase hexadecimal SHA-256 characters",
        )
        if version.get("signature") is not None:
            signature = version["signature"]
            object_fields(signature, f"{item}.signature", ("key", "value"))
            for field in ("key", "value"):
                string(signature[field], f"{item}.signature.{field}")
        if version.get("modules") is not None:
            modules(version["modules"], f"{item}.modules")
        if "yanked" in version:
            require(type(version["yanked"]) is bool, f"{item}.yanked", "a boolean")


def validate(catalogue):
    object_fields(catalogue, "$", ("format", "name", "plugins"))
    # Python considers True == 1, but the catalogue discriminator must be an integer.
    require(type(catalogue["format"]) is int and catalogue["format"] == 1, "$.format", "integer 1")
    string(catalogue["name"], "$.name")
    require(isinstance(catalogue["plugins"], list), "$.plugins", "an array of plugins")
    seen = set()
    for index, plugin in enumerate(catalogue["plugins"]):
        item = f"$.plugins[{index}]"
        object_fields(plugin, item, ("id", "description", "repository", "modules"), ("versions",))
        identifier(plugin["id"], f"{item}.id")
        require(plugin["id"] not in seen, f"{item}.id", "a unique plugin ID")
        seen.add(plugin["id"])
        string(plugin["description"], f"{item}.description")
        require(bool(plugin["description"].strip()), f"{item}.description", "a nonblank description")
        repository = plugin["repository"]
        require(
            isinstance(repository, str)
            and REPOSITORY.fullmatch(repository) is not None
            and all(part not in (".", "..") for part in repository.split("/")),
            f"{item}.repository",
            "a GitHub owner/repository using letters, digits, dots, underscores or hyphens",
        )
        modules(plugin["modules"], f"{item}.modules")
        if "versions" in plugin:
            versions(plugin["versions"], f"{item}.versions")
    return catalogue


def load(source):
    try:
        return validate(json.loads(source.read_text(encoding="utf-8")))
    except json.JSONDecodeError as error:
        raise CatalogueError(
            f"{source}:{error.lineno}:{error.colno}: invalid JSON: {error.msg}; fix the JSON and retry"
        ) from error
    except CatalogueError as error:
        raise CatalogueError(f"{source}: {error}") from error


def prose(value):
    # Treat catalogue summaries as text, not Markdown/HTML that could alter the page structure.
    return "".join(
        f"&#{ord(character)};" if character in "&<>\\`*_{}[]()#!|~+-." else character
        for character in " ".join(value.split())
    )


def render(catalogue):
    lines = [
        "<!-- Generated from docs/plugins.json; edit that file, not this build artifact. -->",
        "",
        "# Plugin catalogue",
        "",
        "Browse the plugins in the reviewed catalogue below.",
        "A catalogue entry is for discovery; it does not assert that a package is safe or has a published release.",
        "See [Plugin packages](plugins.md) for installation and the package contract,",
        "or read the [raw JSON catalogue](plugins.json).",
        "",
    ]
    if not catalogue["plugins"]:
        lines += ["No plugins are listed in this catalogue yet.", ""]
    for plugin in catalogue["plugins"]:
        repository = plugin["repository"]
        url = f"https://github.com/{repository}"
        lines += [
            f"## [{plugin['id']}]({url})",
            "",
            textwrap.fill(prose(plugin["description"]), width=120, break_long_words=False, break_on_hyphens=False),
            "",
            f"Repository: [{repository}]({url}).",
            "",
        ]
        if plugin["modules"]:
            lines += ["| Module ID | Type |", "| --- | --- |"]
            lines += [f"| `{module['id']}` | `{module['type']}` |" for module in plugin["modules"]]
        else:
            lines += ["No modules are listed for this plugin."]
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=ROOT / "docs/plugins.json")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/plugin-catalogue.md")
    args = parser.parse_args()
    try:
        # Invalid input must fail before touching the previous build's page.
        page = render(load(args.source))
        args.output.write_text(page, encoding="utf-8")
    except (CatalogueError, OSError, UnicodeError) as error:
        print(f"Plugin catalogue generation failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
