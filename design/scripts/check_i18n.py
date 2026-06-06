#!/usr/bin/env python3
r"""Sync-check for assets/i18n/*.toml (en, ru, de, ...).

Verifies, for every pair of locale files in ``assets/i18n/``:

  1. The set of top-level (non-plural) leaf keys is identical. Plural
     keys (sub-tables with sub-keys like ``one`` / ``other``) are
     tracked separately so a locale that does not use ``few`` does
     not fail the parity check.
  2. For each non-plural key, the set of ``{placeholder}`` tokens in
     the value string is identical across locales.
  3. Every plural key has at least the ``one`` and ``other`` sub-keys
     in every locale, and every non-empty plural form contains the
     ``{}`` count placeholder.
  4. Each file has at least one leaf value.

Stdlib only. Uses ``tomllib`` (Python 3.11+); on older Pythons, falls
back to a hand-rolled reader for the subset of TOML we need (string
values, ``[sections]``, dotted keys, and sub-tables of string values).

Exits 0 on success with ``OK: <N> locales in sync (<K> keys)``.
Exits 1 on failure with per-key diagnostics.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

REPO_ROOT = Path(__file__).resolve().parents[2]
I18N_DIR = REPO_ROOT / "assets" / "i18n"

NAMED_PLACEHOLDER_RE = re.compile(r"\{[a-z_][a-z_0-9]*\}")
BARE_PLACEHOLDER_RE = re.compile(r"\{\}")
PLURAL_CATEGORIES = ("zero", "one", "two", "few", "many", "other")
PLURAL_REQUIRED = ("one", "other")


class TomlParseError(Exception):
    """Raised when the hand-rolled TOML reader encounters something it
    can't handle."""


def _load_toml(path: Path) -> dict[str, Any]:
    """Load a flat TOML mapping. Uses stdlib ``tomllib`` on 3.11+;
    falls back to a hand-rolled parser on older versions."""
    try:
        import tomllib  # type: ignore[import-not-found]
    except ImportError:
        tomllib = None

    if tomllib is not None:
        with path.open("rb") as f:
            return tomllib.load(f)

    return _parse_toml_manual(path.read_text(encoding="utf-8"))


def _strip_inline_comment(line: str) -> str:
    """Remove ``# comment`` from a TOML line, respecting quoted strings."""
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
    s = raw.strip()
    if not s:
        return ""
    if len(s) >= 2 and s[0] == s[-1] and s[0] in ('"', "'"):
        return s[1:-1]
    return s


def _parse_toml_manual(text: str) -> dict[str, Any]:
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
                        "is not a table"
                    )
                current = nxt
            continue
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
                        "is not a table"
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


def _is_plural_table(value: Any) -> bool:
    if not isinstance(value, dict) or not value:
        return False
    return all(
        isinstance(k, str) and k in PLURAL_CATEGORIES and isinstance(v, str)
        for k, v in value.items()
    )


def _split_scalars_and_plurals(
    data: dict[str, Any], prefix: str = ""
) -> tuple[dict[str, str], dict[str, dict[str, str]]]:
    scalars: dict[str, str] = {}
    plurals: dict[str, dict[str, str]] = {}

    def visit(node: Any, path: str) -> None:
        if isinstance(node, dict):
            if _is_plural_table(node):
                if path in plurals:
                    raise TomlParseError(
                        f"duplicate plural key {path!r}"
                    )
                plurals[path] = {k: v for k, v in node.items()}
                return
            for k, v in node.items():
                child = f"{path}.{k}" if path else str(k)
                visit(v, child)
            return
        scalars[path] = node if isinstance(node, str) else str(node)

    visit(data, prefix)
    return scalars, plurals


def _named_placeholders(value: str) -> frozenset[str]:
    return frozenset(NAMED_PLACEHOLDER_RE.findall(value))


def _has_bare_placeholder(value: str) -> bool:
    return bool(BARE_PLACEHOLDER_RE.search(value))


def _check_plural(
    key: str,
    per_locale: dict[str, dict[str, str]],
    failures: list[str],
) -> None:
    used: set[str] = set()
    for forms in per_locale.values():
        used.update(forms.keys())
    for locale, forms in per_locale.items():
        for required in PLURAL_REQUIRED:
            if required not in forms:
                failures.append(
                    f"FAIL: plural key '{key}' in {locale}.toml "
                    f"missing required form '{required}'"
                )
        for cat, value in forms.items():
            if value and not _has_bare_placeholder(value):
                failures.append(
                    f"FAIL: plural key '{key}' form '{cat}' in "
                    f"{locale}.toml is non-empty but missing the "
                    f"'{{}}' count placeholder"
                )
        for cat in used:
            if cat not in forms:
                failures.append(
                    f"FAIL: plural key '{key}' in {locale}.toml "
                    f"missing form '{cat}' (used by another locale)"
                )


def _check_locale_pair(
    a_name: str,
    a_scalars: dict[str, str],
    a_plurals: dict[str, dict[str, str]],
    b_name: str,
    b_scalars: dict[str, str],
    b_plurals: dict[str, dict[str, str]],
    failures: list[str],
) -> None:
    for key in sorted(a_scalars.keys() - b_scalars.keys()):
        failures.append(
            f"FAIL: key '{key}' missing in {b_name}.toml"
        )
    for key in sorted(b_scalars.keys() - a_scalars.keys()):
        failures.append(
            f"FAIL: key '{key}' missing in {a_name}.toml"
        )
    for key in sorted(a_scalars.keys() & b_scalars.keys()):
        if _named_placeholders(a_scalars[key]) != _named_placeholders(
            b_scalars[key]
        ):
            failures.append(
                f"FAIL: key '{key}' placeholders differ: "
                f"{a_name}={sorted(_named_placeholders(a_scalars[key]))} "
                f"{b_name}={sorted(_named_placeholders(b_scalars[key]))}"
            )

    for key in sorted(a_plurals.keys() - b_plurals.keys()):
        failures.append(
            f"FAIL: plural key '{key}' missing in {b_name}.toml"
        )
    for key in sorted(b_plurals.keys() - a_plurals.keys()):
        failures.append(
            f"FAIL: plural key '{key}' missing in {a_name}.toml"
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="check_i18n",
        description=(
            "Verify that assets/i18n/*.toml are in sync (keys, "
            "placeholder tokens, plural sections)."
        ),
    )
    args = parser.parse_args(argv)
    del args  # noqa: F841

    if not I18N_DIR.exists():
        print(f"FAIL: {I18N_DIR}: directory not found")
        return 1

    locale_files = sorted(
        p for p in I18N_DIR.glob("*.toml") if p.is_file()
    )
    if not locale_files:
        print(f"FAIL: {I18N_DIR}: no .toml locale files found")
        return 1

    locales: dict[str, tuple[dict[str, str], dict[str, dict[str, str]]]] = {}
    failures: list[str] = []

    for path in locale_files:
        locale = path.stem
        try:
            raw = _load_toml(path)
        except (TomlParseError, OSError) as exc:
            print(f"FAIL: {path}: TOML parse error: {exc}")
            return 1
        try:
            scalars, plurals = _split_scalars_and_plurals(raw)
        except TomlParseError as exc:
            print(f"FAIL: {path}: {exc}")
            return 1
        if not scalars and not plurals:
            print(f"FAIL: {path}: no leaf keys found (file appears empty)")
            return 1
        locales[locale] = (scalars, plurals)

    locale_names = sorted(locales.keys())
    for i, a in enumerate(locale_names):
        for b in locale_names[i + 1 :]:
            _check_locale_pair(
                a, *locales[a], b, *locales[b], failures,
            )

    all_plural_keys: set[str] = set()
    for _, plurals in locales.values():
        all_plural_keys.update(plurals.keys())
    for key in sorted(all_plural_keys):
        per_locale = {
            locale: plurals[key]
            for locale, (_, plurals) in locales.items()
            if key in plurals
        }
        _check_plural(key, per_locale, failures)

    if failures:
        for line in failures:
            print(line)
        return 1

    sample = next(iter(locales.values()))
    n_keys = len(sample[0]) + len(sample[1])
    total = sum(
        1 for _, (s, p) in locales.items() for _ in (*s, *p)
    )
    print(
        f"OK: {len(locales)} locales in sync "
        f"({n_keys} keys, {total} total leaves across locales)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
