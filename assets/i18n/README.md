# `assets/i18n/` — SupaZip localisation files

This directory holds the user-facing string tables for every locale
SupaZip ships. Every file is a flat TOML document; the on-disk format
is enforced by [`design/scripts/check_i18n.py`](../../design/scripts/check_i18n.py),
which runs in CI and from the local `scripts/ci.sh` entry point.

## Layout

| File           | Locale  | Notes                                                                                       |
|----------------|---------|---------------------------------------------------------------------------------------------|
| `en.toml`      | English | Source-of-truth copy. New keys are added here first.                                         |
| `ru.toml`      | Russian | Brand voice: short imperative verbs; `error:` / `warning:` prefixes stay in English.        |
| `de.toml`      | German  | Third locale, introduced in 0.2.0 (WS-D). Plural rules are the same as English (two forms).|
| `plural.md`    | —       | Reference for the on-disk plural format and the per-locale rules.                           |
| `README.md`    | —       | This file.                                                                                  |

A new locale is added by dropping a `<locale>.toml` file in this
directory; the check script picks it up automatically on the next
run.

## Key format

Keys are dotted, lowercase, and live under a logical section. The
section name is the first segment of the key; the rest of the path
identifies the leaf string.

```toml
[toolbar]
open    = "Open…"
extract = "Extract"

[grid.size]
bytes   = "{n} B"
```

Flattens to the dotted keys `toolbar.open`, `toolbar.extract` and
`grid.size.bytes`. The script enforces that every locale file has
the **same set** of dotted keys.

## Placeholders

Two placeholder styles are in use:

1. **Named** — `{name}`, `{path}`, `{n}`. The runtime substitutes
   the matching positional or keyword argument. The check script
   compares the **set** of named placeholders between locales for
   the same key; an English key with `{path}` must have the same
   `{path}` placeholder (or a translated one — the script does not
   translate, only checks presence) in every other locale.
2. **Bare** — `{}`. Used **only** in plural forms, where the
   runtime substitutes the count. See `plural.md`.

The two styles never mix in the same value: a plural form uses
`{}`, a regular string uses `{name}`.

## Plural sections

A pluralised key is a regular dotted key whose value is a
**sub-table** of CLDR plural categories. See [`plural.md`](plural.md)
for the full specification and per-locale rules. The summary:

```toml
[progress.entries]
one   = "{} entry"
few   = "{} entries"   # Russian uses this
many  = "{} entries"   # Russian uses this
other = "{} entries"
```

Rules enforced by `check_i18n.py`:

* Every plural key has at least `one` and `other` in every locale.
* Every non-empty plural form contains the bare `{}` placeholder.
* A category used by *any* locale (e.g. `few` in Russian) is
  declared in *every* locale, even if the value is an empty string
  (the runtime falls back to `other`).

## Adding a new key

1. Pick a section that fits the key's role
   (`app.*` / `toolbar.*` / `dialog.*` / `grid.*` /
   `status.*` / `cli.*` / `format.*` / `progress.*` / `error.*`).
2. Add the entry to `en.toml` first.
3. Mirror the entry into every other locale. Use the same
   placeholder set as English.
4. Run `python design/scripts/check_i18n.py`. It must exit 0.

If the key is pluralised, follow the rules in `plural.md` and
add a sub-table to **every** locale (locales that do not use
`few` / `many` keep them as empty strings).

## Adding a new locale

1. Copy `en.toml` to `<locale>.toml`.
2. Translate every value. Keep the section names, key names, and
   placeholder tokens identical.
3. For plural rules, see `plural.md` for the per-locale table.
4. Update this `README.md` to list the new file in the layout
   table above.
5. Run `python design/scripts/check_i18n.py`. It must exit 0.

## Verifying

```sh
python design/scripts/check_i18n.py    # exits 0 / 1
python design/scripts/check_tokens.py  # unrelated; the design-token
                                        # sync check
```

The CI workflow at `.github/workflows/ci.yml` runs both scripts on
every push and fails the build on a non-zero exit.
