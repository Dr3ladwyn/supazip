#!/usr/bin/env python3
r"""Sync-check for assets/i18n/en.toml and assets/i18n/ru.toml.

Verifies:
  1. The set of leaf keys (dotted, including `[section] [subsection.key]`
     flattening) is identical in both files.
  2. For each key, the set of `{placeholder}` tokens in the value string
     is identical in both files (regex `` \{[a-z_][a-z_0-9]*\} ``).
  3. Both files have at least one value.

Stdlib only. Uses `tomllib` (3.11+); on older Pythons, falls back to a
hand-rolled reader for the subset of TOML we need (string values,
`sections`, dotted keys within a section).

Exits 0 on success with `OK: en.toml and ru.toml are in sync (N keys)`.
Exits 1 on failure with per-key diagnostics.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
EN_PATH = REPO_ROOT / "assets" / "i18n" / "en.toml"
RU_PATH = REPO_ROOT / "assets" / "i18n" / "ru.toml"

PLACEHOLDER_RE = re.compile(r"\{[a-z_][a-z_0-9]*\}")


class TomlParseError(Exception):
    """Raised when the hand-rolled TOML reader encounters something it
    can't handle."""


def _load_toml(path: Path) -> dict[str, Any]:
    """Load a flat TOML mapping. Uses stdlib `tomllib` on 3.11+; falls
    back to a hand-rolled parser on older versions.

    The hand-rolled parser handles the subset of TOML used by en.toml and
    ru.toml: `[table]` headers, `key = "string"` (or unquoted string)
    values, blank lines, and `# comments`. It does not handle arrays,
    inline tables, multi-line strings, dates, or numbers.
    """
    try:
        import tomllib  # type: ignore[import-not-found]
    except ImportError:
        tomllib = None

    if tomllib is not None:
        with path.open("rb") as f:
            return tomllib.load(f)

    return _parse_toml_manual(path.read_text(encoding="utf-8"))


def _strip_inline_comment(line: str) -> str:
    """Remove `# comment` from a TOML line, respecting quoted strings.

    TOML's rule: `#` starts a comment only when preceded by whitespace or
    at the start of a line.
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


def _parse_string_value(raw: str) -> str:
    """Coerce a TOML scalar RHS into a Python string.

    Supports basic "double", 'single', and bare unquoted strings. Returns
    the bare string with surrounding whitespace stripped.
    """
    s = raw.strip()
    if not s:
        return ""
    if len(s) >= 2 and s[0] == s[-1] and s[0] in ('"', "'"):
        return s[1:-1]
    return s


def _parse_toml_manual(text: str) -> dict[str, Any]:
    """Minimal TOML parser for our i18n files.

    Returns a nested dict. A `[section]` header opens (or resets) a
    subtable; a `[a.b]` header opens nested subtables. Keys with a dot
    inside a section also create nested subtables, matching TOML's
    dotted-key semantics.
    """
    root: dict[str, Any] = {}
    current: dict[str, Any] = root
    last_line_no = 0
    for raw in text.splitlines():
        last_line_no += 1
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        line = _strip_inline_comment(raw)
        if not line.strip():
            continue
        if line.lstrip().startswith("["):
            # Table header: must be a single [section] on this line.
            if not (line.startswith("[") and line.endswith("]")):
                raise TomlParseError(
                    f"line {last_line_no}: invalid table header: {raw!r}"
                )
            header = line[1:-1].strip()
            if not header:
                raise TomlParseError(
                    f"line {last_line_no}: empty table header"
                )
            current = root
            for part in header.split("."):
                part = part.strip()
                if not part:
                    raise TomlParseError(
                        f"line {last_line_no}: empty segment in header"
                    )
                nxt = current.get(part)
                if nxt is None:
                    nxt = {}
                    current[part] = nxt
                elif not isinstance(nxt, dict):
                    raise TomlParseError(
                        f"line {last_line_no}: table segment {part!r} "
                        f"is not a table"
                    )
                current = nxt
            continue
        # key = value
        if "=" not in line:
            raise TomlParseError(
                f"line {last_line_no}: missing '=' in key/value: {raw!r}"
            )
        key, _, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if not key:
            raise TomlParseError(
                f"line {last_line_no}: empty key: {raw!r}"
            )
        # Allow dotted keys to create nested tables.
        target = current
        if "." in key:
            parts = key.split(".")
            for part in parts[:-1]:
                part = part.strip()
                if not part:
                    raise TomlParseError(
                        f"line {last_line_no}: empty dotted segment: {raw!r}"
                    )
                nxt = target.get(part)
                if nxt is None:
                    nxt = {}
                    target[part] = nxt
                elif not isinstance(nxt, dict):
                    raise TomlParseError(
                        f"line {last_line_no}: dotted segment {part!r} "
                        f"is not a table"
                    )
                target = nxt
            leaf = parts[-1].strip()
        else:
            leaf = key
        if leaf in target:
            raise TomlParseError(
                f"line {last_line_no}: duplicate key {leaf!r}"
            )
        target[leaf] = _parse_string_value(value)
    return root


def _flatten(data: Any, prefix: str = "") -> dict[str, str]:
    """Flatten nested dicts into dotted keys → string values.

    Non-string leaf values are coerced via `str()`; i18n files only
    contain strings, but this keeps the function robust.
    """
    out: dict[str, str] = {}
    if isinstance(data, dict):
        for k, v in data.items():
            child = f"{prefix}.{k}" if prefix else str(k)
            if isinstance(v, dict):
                out.update(_flatten(v, child))
            else:
                out[child] = v if isinstance(v, str) else str(v)
    return out


def _placeholders(value: str) -> frozenset[str]:
    return frozenset(PLACEHOLDER_RE.findall(value))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="check_i18n",
        description=(
            "Verify that assets/i18n/en.toml and assets/i18n/ru.toml are "
            "in sync (keys + placeholder tokens)."
        ),
    )
    args = parser.parse_args(argv)
    del args  # noqa: F841

    if not EN_PATH.exists():
        print(f"FAIL: {EN_PATH}: file not found")
        return 1
    if not RU_PATH.exists():
        print(f"FAIL: {RU_PATH}: file not found")
        return 1

    try:
        en_raw = _load_toml(EN_PATH)
    except (TomlParseError, OSError) as exc:
        print(f"FAIL: {EN_PATH}: TOML parse error: {exc}")
        return 1

    try:
        ru_raw = _load_toml(RU_PATH)
    except (TomlParseError, OSError) as exc:
        print(f"FAIL: {RU_PATH}: TOML parse error: {exc}")
        return 1

    en = _flatten(en_raw)
    ru = _flatten(ru_raw)

    if not en:
        print(f"FAIL: {EN_PATH}: no leaf keys found (file appears empty)")
        return 1
    if not ru:
        print(f"FAIL: {RU_PATH}: no leaf keys found (file appears empty)")
        return 1

    failures: list[str] = []
    en_keys = set(en.keys())
    ru_keys = set(ru.keys())

    for key in sorted(en_keys - ru_keys):
        failures.append(f"FAIL: key '{key}' missing in {RU_PATH.name}")
    for key in sorted(ru_keys - en_keys):
        failures.append(f"FAIL: key '{key}' missing in {EN_PATH.name}")

    shared = en_keys & ru_keys
    for key in sorted(shared):
        en_ph = _placeholders(en[key])
        ru_ph = _placeholders(ru[key])
        if en_ph != ru_ph:
            failures.append(
                f"FAIL: key '{key}' placeholders differ: "
                f"en={sorted(en_ph)} ru={sorted(ru_ph)}"
            )

    if failures:
        for line in failures:
            print(line)
        return 1

    print(f"OK: en.toml and ru.toml are in sync ({len(shared)} keys)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
