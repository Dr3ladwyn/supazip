#!/usr/bin/env python3
"""Sync-check for design/tokens.yaml and design/tokens.json.

Verifies:
  1. Top-level keys match exactly between YAML and JSON.
  2. For each nested dict, the set of subkeys matches exactly.
  3. All values under `themes.*.color.*` (and legacy `color.*`) match
     `^#?[0-9A-Fa-f]{6}$`, and YAML/JSON leaf values are identical.
  4. `meta.version` is identical in both files.
  5. No value is `None` (no missing values in YAML that should have been
     mirrored to JSON).
  6. Both files have `themes.dark` and `themes.light`, each with the
     required color groups (bg / fg / accent / semantic / border) and
     keys.

Stdlib only. Uses a hand-rolled YAML reader sufficient for the subset of
YAML used by tokens.yaml (nested mappings with `key: value` lines, scalar
values, inline comments). No external dependencies.

Exits 0 on success with `OK: tokens.yaml and tokens.json are in sync`.
Exits 1 on any failure with one `FAIL: <path>: <message>` line per issue.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
YAML_PATH = REPO_ROOT / "design" / "tokens.yaml"
JSON_PATH = REPO_ROOT / "design" / "tokens.json"

COLOR_HEX_RE = re.compile(r"^#?[0-9A-Fa-f]{6}$")

REQUIRED_THEMES = ("dark", "light")
REQUIRED_COLOR_GROUPS: dict[str, tuple[str, ...]] = {
    "bg": ("base", "surface", "sunken", "raised"),
    "fg": ("primary", "secondary", "muted", "inverse"),
    "accent": ("primary", "primary_hover", "pressed"),
    "semantic": ("success", "warning", "danger", "info"),
    "border": ("subtle", "strong", "focus"),
}


class YamlParseError(Exception):
    """Raised when the YAML reader encounters something it can't handle."""


def _strip_inline_comment(line: str) -> str:
    """Remove `# comment` from a line, leaving quoted strings intact.

    Only strips when `#` is preceded by whitespace, matching YAML's rule
    that `#` is a comment marker only when separated from the value.
    """
    in_single = False
    in_double = False
    for i, ch in enumerate(line):
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        elif ch == "#" and not in_single and not in_double:
            if i == 0 or line[i - 1].isspace():
                return line[:i].rstrip()
    return line


def _coerce_scalar(text: str) -> Any:
    """Best-effort scalar coercion for our hand-rolled YAML reader."""
    s = text.strip()
    if not s:
        return ""
    if len(s) >= 2 and (
        (s.startswith('"') and s.endswith('"'))
        or (s.startswith("'") and s.endswith("'"))
    ):
        return s[1:-1]
    if s.lower() in ("true", "yes"):
        return True
    if s.lower() in ("false", "no"):
        return False
    if s.lower() in ("null", "~"):
        return None
    try:
        return int(s)
    except ValueError:
        pass
    try:
        return float(s)
    except ValueError:
        pass
    return s


def _parse_yaml(text: str) -> Any:
    """Indent-based YAML subset parser.

    Handles nested mappings with `key: value` lines and consistent
    indentation. Returns a dict (the document root). Whitespace-only or
    comment-only lines are ignored.

    Raises YamlParseError on any structural issue.
    """
    root: dict[str, Any] = {}
    # Stack entries: (indent, container_dict)
    stack: list[tuple[int, dict[str, Any]]] = [(-1, root)]
    last_line_no = 0
    for raw in text.splitlines():
        last_line_no += 1
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        line = _strip_inline_comment(raw)
        if not line.strip():
            continue
        if ":" not in line:
            raise YamlParseError(
                f"line {last_line_no}: missing ':' separator: {raw!r}"
            )
        leading = len(line) - len(line.lstrip(" "))
        key, _, value = line.lstrip().partition(":")
        key = key.strip()
        value = value.strip()
        if not key:
            raise YamlParseError(
                f"line {last_line_no}: empty key in mapping: {raw!r}"
            )
        # Pop stack to current indent (strictly greater indent than current).
        while stack and stack[-1][0] >= leading:
            stack.pop()
        if not stack:
            stack = [(-1, root)]
        parent = stack[-1][1]
        if value == "":
            new_dict: dict[str, Any] = {}
            if key in parent and not isinstance(parent[key], dict):
                raise YamlParseError(
                    f"line {last_line_no}: duplicate or scalar key {key!r} "
                    f"cannot be redefined as a mapping"
                )
            parent[key] = new_dict
            stack.append((leading, new_dict))
        else:
            if key in parent:
                raise YamlParseError(
                    f"line {last_line_no}: duplicate key {key!r}"
                )
            parent[key] = _coerce_scalar(value)
    return root


def _load_yaml(path: Path) -> Any:
    return _parse_yaml(path.read_text(encoding="utf-8"))


def _check_node(
    path: str,
    y_val: Any,
    j_val: Any,
    failures: list[str],
) -> None:
    if isinstance(y_val, dict) and isinstance(j_val, dict):
        y_keys = set(y_val.keys())
        j_keys = set(j_val.keys())
        if y_keys != j_keys:
            only_y = y_keys - j_keys
            only_j = j_keys - y_keys
            if only_y:
                failures.append(
                    f"FAIL: {path}: keys only in YAML: {sorted(only_y)}"
                )
            if only_j:
                failures.append(
                    f"FAIL: {path}: keys only in JSON: {sorted(only_j)}"
                )
        for k in sorted(y_keys & j_keys):
            _check_node(f"{path}.{k}", y_val[k], j_val[k], failures)
        return

    if y_val is None or j_val is None:
        if y_val is None:
            failures.append(f"FAIL: {path}: YAML value is null/None")
        if j_val is None:
            failures.append(f"FAIL: {path}: JSON value is null/None")
        return

    if type(y_val) is not type(j_val):
        # Allow int/float equivalence.
        if not (
            isinstance(y_val, (int, float)) and isinstance(j_val, (int, float))
        ):
            failures.append(
                f"FAIL: {path}: type mismatch "
                f"(YAML={type(y_val).__name__}, JSON={type(j_val).__name__})"
            )
            return

    if _is_color_path(path):
        if isinstance(y_val, str) and not COLOR_HEX_RE.match(y_val):
            failures.append(
                f"FAIL: {path}: YAML color '{y_val}' does not match "
                f"^#?[0-9A-Fa-f]{{6}}$"
            )
        if isinstance(j_val, str) and not COLOR_HEX_RE.match(j_val):
            failures.append(
                f"FAIL: {path}: JSON color '{j_val}' does not match "
                f"^#?[0-9A-Fa-f]{{6}}$"
            )

    if y_val != j_val:
        failures.append(
            f"FAIL: {path}: value mismatch (YAML={y_val!r}, JSON={j_val!r})"
        )


def _check_structure(
    yaml: Any,
    json_data: Any,
    failures: list[str],
) -> None:
    if not isinstance(yaml, dict) or not isinstance(json_data, dict):
        failures.append("FAIL: root: both files must deserialize to objects")
        return

    yaml_keys = set(yaml.keys())
    json_keys = set(json_data.keys())
    if yaml_keys != json_keys:
        only_yaml = yaml_keys - json_keys
        only_json = json_keys - yaml_keys
        if only_yaml:
            failures.append(
                f"FAIL: root: keys only in YAML: {sorted(only_yaml)}"
            )
        if only_json:
            failures.append(
                f"FAIL: root: keys only in JSON: {sorted(only_json)}"
            )

    for key in sorted(yaml_keys & json_keys):
        _check_node(key, yaml[key], json_data[key], failures)


def _is_color_path(path: str) -> bool:
    """True for paths under a theme color tree (or legacy top-level color).

    Matches `themes.<name>.color` and `themes.<name>.color.*`, plus
    leftover `color` / `color.*` so a reverted v1 file still gets hex
    validation.
    """
    parts = path.split(".")
    if parts[0] == "color":
        return True
    return len(parts) >= 3 and parts[0] == "themes" and parts[2] == "color"


def _check_required_keys(
    node: Any,
    path: str,
    required: tuple[str, ...] | set[str],
    label: str,
    failures: list[str],
) -> dict[str, Any] | None:
    """Require `node` to be a mapping whose keys equal `required`."""
    if not isinstance(node, dict):
        failures.append(f"FAIL: {path}: missing or not a mapping in {label}")
        return None
    have = set(node.keys())
    need = set(required)
    missing = need - have
    extra = have - need
    if missing:
        failures.append(
            f"FAIL: {path}: missing keys in {label}: {sorted(missing)}"
        )
    if extra:
        failures.append(
            f"FAIL: {path}: unexpected keys in {label}: {sorted(extra)}"
        )
    return node


def _check_theme_schema(data: Any, label: str, failures: list[str]) -> None:
    """Require themes.dark / themes.light with the v2 color contract."""
    if not isinstance(data, dict):
        failures.append(f"FAIL: root: not a mapping in {label}")
        return
    themes = data.get("themes")
    if not isinstance(themes, dict):
        failures.append(f"FAIL: themes: missing or not a mapping in {label}")
        return
    for name in REQUIRED_THEMES:
        if name not in themes:
            failures.append(f"FAIL: themes.{name}: missing in {label}")
            continue
        theme = themes[name]
        if not isinstance(theme, dict):
            failures.append(
                f"FAIL: themes.{name}: must be a mapping in {label}"
            )
            continue
        color = theme.get("color")
        color_node = _check_required_keys(
            color,
            f"themes.{name}.color",
            tuple(REQUIRED_COLOR_GROUPS),
            label,
            failures,
        )
        if color_node is None:
            continue
        for group, keys in REQUIRED_COLOR_GROUPS.items():
            _check_required_keys(
                color_node.get(group),
                f"themes.{name}.color.{group}",
                keys,
                label,
                failures,
            )


def _check_meta_version(
    yaml: Any,
    json_data: Any,
    failures: list[str],
) -> None:
    y_ver = yaml.get("meta", {}).get("version") if isinstance(yaml, dict) else None
    j_ver = (
        json_data.get("meta", {}).get("version")
        if isinstance(json_data, dict)
        else None
    )
    if y_ver is None or j_ver is None:
        missing = "YAML" if y_ver is None else "JSON"
        failures.append(f"FAIL: meta.version: missing in {missing}")
        return
    if y_ver != j_ver:
        failures.append(
            f"FAIL: meta.version: mismatch (YAML={y_ver!r}, JSON={j_ver!r})"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="check_tokens",
        description=(
            "Verify that design/tokens.yaml and design/tokens.json are in "
            "sync (keys, types, theme colors, meta.version)."
        ),
    )
    args = parser.parse_args(argv)
    del args  # noqa: F841 — placeholder for future flags

    if not YAML_PATH.exists():
        print(f"FAIL: {YAML_PATH}: file not found")
        return 1
    if not JSON_PATH.exists():
        print(f"FAIL: {JSON_PATH}: file not found")
        return 1

    try:
        yaml_data = _load_yaml(YAML_PATH)
    except YamlParseError as exc:
        print(f"FAIL: {YAML_PATH}: YAML parse error: {exc}")
        return 1
    except OSError as exc:
        print(f"FAIL: {YAML_PATH}: I/O error: {exc}")
        return 1

    try:
        with JSON_PATH.open("rb") as f:
            json_data = json.load(f)
    except json.JSONDecodeError as exc:
        print(f"FAIL: {JSON_PATH}: JSON parse error: {exc}")
        return 1

    failures: list[str] = []
    _check_structure(yaml_data, json_data, failures)
    _check_meta_version(yaml_data, json_data, failures)
    _check_theme_schema(yaml_data, "YAML", failures)
    _check_theme_schema(json_data, "JSON", failures)

    if failures:
        for line in failures:
            print(line)
        return 1

    print("OK: tokens.yaml and tokens.json are in sync")
    return 0


if __name__ == "__main__":
    sys.exit(main())
