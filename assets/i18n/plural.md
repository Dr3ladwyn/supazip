# Plural rules — SupaZip i18n (0.2.0)

This document freezes the **on-disk format** of pluralised keys in
`assets/i18n/*.toml`. It is part of milestone 0.2.0 (WS-D). The
**runtime selection** of the right form per locale is intentionally
**not** implemented in 0.2.0 — that work is deferred to 0.5.0
together with the `icu_pluralrules` / `fluent` evaluation (see
`ROADMAP.md` → "Track D: i18n + a11y + docs" and
`memory-bank/decisionLog.md` entry 2026-06-06).

What 0.2.0 ships:

1. A flat TOML key whose value is a **sub-table** of plural forms
   instead of a single string.
2. The hand-rolled rules below for the three locales we currently
   support (`en`, `ru`, `de`).
3. A check in `design/scripts/check_i18n.py` that every plural
   sub-table has at least the `one` and `other` sub-keys, and that
   the `{}` placeholder is present in all four forms of a given key.

## File format

A pluralised key is a regular dotted key whose value is an inline
table / sub-table of the six CLDR plural categories. Only the
categories a given locale actually uses must be non-empty; missing
categories fall back to `other` at runtime.

```toml
# Generic shape — only illustrative; en/ru/de in this repo currently
# ship one plural key (`progress.entries`) and use the four forms
# shown below.
[progress.entries]
one   = "{} entry"
few   = "{} entries"   # ru only; other locales may leave this empty
many  = "{} entries"   # ru only
other = "{} entries"
```

- The leaf key (`progress.entries`) is what callers reference.
- The count placeholder is the bare `{}` token, not `{n}`. This
  keeps the format compatible with the existing `str::format!`
  pipeline once the runtime is wired up.
- `few` and `many` are kept as **empty strings** in `en` and `de`
  so the key set is uniform across locales; `check_i18n.py` does
  not require those forms to be present in two-form locales, only
  that `one` and `other` exist everywhere.

## Categories we use

| CLDR tag | Meaning                                             |
|----------|-----------------------------------------------------|
| `zero`   | n = 0 (Arabic-only; unused for en/ru/de)            |
| `one`    | n = 1                                               |
| `few`    | 2 ≤ n ≤ 4 (Russian)                                 |
| `many`   | 0 ≤ n ≤ 20 except n ∈ {1..4} (Russian)             |
| `other`  | every other n (English, German, fall-back)          |

## Per-locale rules

### English (`en`)

Two forms, matching CLDR English:

- `n == 1` → `one`
- everything else → `other`

`few` and `many` are **not used** and stay empty.

### Russian (`ru`)

Three forms, matching CLDR Russian:

- `n % 10 == 1 && n % 100 != 11` → `one` (1, 21, 31, …, 101, …)
- `n % 10 ∈ {2,3,4} && n % 100 ∉ {12,13,14}` → `few` (2-4, 22-24, …)
- everything else → `many` (0, 5-20, 25-30, …)

The `n % 100` carve-out is the standard CLDR exception for the
teens: 11, 12, 13, 14 always take `many`, never `one` / `few`.

### German (`de`)

Two forms, matching CLDR German:

- `n == 1` → `one`
- everything else → `other`

`few` and `many` are **not used** and stay empty.

## Hand-rolled runtime selection (deferred)

A 0.2.0 consumer that needs the plural form picks it with a
hand-rolled helper per locale:

```rust
// Pseudocode; do not implement in 0.2.0.
fn select_form(locale: &str, n: u64) -> &'static str {
    match locale {
        "en" | "de" => if n == 1 { "one" } else { "other" },
        "ru" => match n {
            1 => "one",
            n if n % 10 >= 2 && n % 10 <= 4
                && (n % 100 < 10 || n % 100 >= 20) => "few",
            _ => "many",
        },
        _ => "other",
    }
}
```

The actual `t_plural(key, locale, n, args) -> String` facade that
the GUI / CLI will call is scheduled for the 0.5.0 i18n milestone,
by which point we will pick `icu_pluralrules` or `fluent` and
remove this hand-rolled helper.

## Adding a new plural key

1. Add a sub-table to **every** locale file. Even locales that
   don't use `few` / `many` must list those keys (with an empty
   string value) so the key set stays flat and parseable.
2. Use `{}` for the count placeholder. Do not invent a second
   placeholder name — the runtime formatter will substitute the
   count at exactly one position.
3. Run `python design/scripts/check_i18n.py`. It fails if any
   plural sub-table is missing `one` or `other`, or if the `{}`
   placeholder is absent from any of the four forms.
