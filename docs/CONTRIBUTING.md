# Contributing to SupaZip

Welcome. SupaZip is a small, focused project: a Rust cross-platform archive
manager for 7z and ZIP, with a shared engine, a CLI, and an eframe/egui
desktop GUI. The team is small and friendly, the bar for review is reasonable,
and contributions of all kinds are welcome — Rust code, tests, documentation,
design tokens, i18n strings, brand assets, security review, and bug reports.

The rest of this document explains how the repo is laid out, what the
pre-PR checklist looks like, and the specific workflow for changing anything
in the design system (tokens, UI strings, logo). If something here is wrong or
out of date, please open a PR — the doc lives at `docs/CONTRIBUTING.md` and
evolves with the project.

## Project layout

```
SupaZip/
├── DESIGN.md                # the design system doc
├── design/
│   ├── tokens.yaml          # single source of truth
│   ├── tokens.json          # JSON mirror
│   ├── cli-table.tera       # CLI table layout contract
│   └── scripts/             # check_tokens.py, check_i18n.py
├── assets/
│   ├── logo.svg             # canonical wordmark + mark
│   ├── logo-mark.svg        # icon-only mark
│   ├── i18n/                # en.toml, ru.toml
│   └── fonts/README.md      # JetBrains Mono guidance
├── scripts/ci.ps1           # local CI entry point
├── docs/CONTRIBUTING.md     # this file
└── supazip/                 # Cargo workspace
```

The full repository layout (including `.cursor/`, `memory-bank/`, and the
CI workflow) is in the project's [README](../README.md#repository-layout).

## Before opening a PR

Run the following checks locally. They are the same checks the local
`scripts/ci.ps1` entry point runs; CI will run them again on Windows.

- `cd supazip && cargo fmt --all`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `python design/scripts/check_tokens.py` (also run by `scripts/ci.ps1`)
- `python design/scripts/check_i18n.py`

For the longer setup (toolchain, GUI build prerequisites, `JetBrains Mono`
install), see the project's [Quickstart](../README.md#quickstart).

## Changing the design

The design system is the contract between every visual surface in SupaZip.
Anything that touches it must keep the files in lockstep. The subsections
below are the workflow for each kind of change; the full rationale lives in
[DESIGN.md](../DESIGN.md).

### Changing a token

1. Edit `design/tokens.yaml`. YAML is the human-written source of truth.
2. Mirror the change into `design/tokens.json` (same key path, same value).
3. Bump `meta.version` in both files (patch for additive changes, minor
   for breaking visual changes — see [DESIGN.md §10](../DESIGN.md)).
4. Run `python design/scripts/check_tokens.py` and confirm it exits with
   `OK: tokens.yaml and tokens.json are in sync`. The script checks the key
   tree, the `^#?[0-9A-Fa-f]{6}$` shape of every value under `color.*`, and
   that `meta.version` matches in both files.

Do not edit `tokens.json` by hand in isolation — it is a mirror, not a
source. The CI step will fail if the two diverge.

### Changing a UI string

1. Edit BOTH `assets/i18n/en.toml` AND `assets/i18n/ru.toml`. English is the
   source language; Russian ships in parallel.
2. Use the same dot-notation key in both files (e.g. `status.opening`).
3. The set of `{placeholder}` names must match between the two files. For
   example, if `en.toml` has `"extracting {archive} to {out}…"` then
   `ru.toml` must also reference exactly `{archive}` and `{out}` (in
   whatever order reads naturally in Russian). The set, not the order, is
   checked.
4. Run `python design/scripts/check_i18n.py` and confirm it exits with
   `OK: en.toml and ru.toml are in sync (N keys)`.
5. Do NOT change the word "SupaZip" (the literal product name) without a
   separate review that touches `DESIGN.md` §2 (brand voice) and the empty
   state in `assets/i18n/{en,ru}.toml` together.

### Changing the logo

1. Only touch `assets/logo.svg` and `assets/logo-mark.svg`. These are the
   only two brand assets shipped in v1.
2. Keep `currentColor` for stroke and fill so the dark palette is the only
   palette that needs shipping. The viewer (a theme, a CSS class, an egui
   `Color32`) decides the colour.
3. Do not introduce gradients, filters, or shadows. The brand is monoline
   and theme-agnostic.
4. Keep each file ≤ 2 KB. A 24×24 mark and a 192×48 lockup with `currentColor`
   strokes and no effects fit comfortably in that budget.
5. If the wordmark text "SupaZip" must change, see the "do NOT change the
   word 'SupaZip'" note in the previous subsection — it requires a
   `DESIGN.md` review.

## Adding a new language

The current `check_i18n.py` only knows about the `en`+`ru` pair: it reads
`assets/i18n/en.toml` and `assets/i18n/ru.toml` by default and does not
accept command-line arguments. To add a third language:

1. Copy `en.toml` to `assets/i18n/<lang>.toml` (where `<lang>` is a
   standard BCP-47 tag, e.g. `de`, `fr`, `zh-Hans`).
2. Translate each value. Keep the keys and `{placeholder}` names identical
   to `en.toml`.
3. The default `python design/scripts/check_i18n.py` invocation will not
   pick up the new file. Either extend `check_i18n.py` to accept
   positional `en.toml <lang>.toml` arguments (so contributors can run
   `python design/scripts/check_i18n.py en.toml de.toml` locally), or
   add a per-language pair script. The script's `main(argv)` is already
   wired through `argparse`, so adding optional positional paths is a
   small follow-up patch.
4. Update `scripts/ci-design.ps1` to check every pair, not just `en`+`ru`.
5. Add a line to the `assets/i18n/` section of this document and to
   `DESIGN.md` §8 listing the new locale.

Until both the script and the CI step understand the new pair, the new
file will not be enforced — so do step 4 in the same PR as step 1.

## Security audits

CI runs **cargo-audit** and **cargo-deny** on every push/PR to `master`
(see `.github/workflows/audit.yml`). To run them locally:

```bash
cargo install cargo-audit && cd supazip && cargo audit
cargo install cargo-deny  && cd supazip && cargo deny check
```

For details on suppressing advisories, handling license failures, and
managing ban hits, see [docs/security.md](security.md).

## Testing

- **Engine + CLI:** `cd supazip && cargo test -p supazip-core -p supazip-cli`
  runs the unit tests in `supazip-core` and the `cli_round_trip` integration
  tests in `supazip-cli` (list, extract, create, test, encryption, error
  variants, `Limits`, cancellation, path-traversal refusal).
- **GUI:** `cargo test -p supazip-gui` runs the `AppController` unit tests
  (state transitions, error mapping, status bar string selection). Visual
  snapshots are deferred to v1.1.
- **Design system:** `python design/scripts/check_tokens.py` and
  `python design/scripts/check_i18n.py` are the equivalent for the design
  layer; both are wired into `scripts/ci.ps1` and GitHub Actions.
- **Unified entry point:** `scripts/ci.ps1` (Windows) or `scripts/ci.sh`
  (POSIX) runs everything above in order.

## Code style

- Rust 2021 edition, `rustfmt` defaults, plain `clippy::all` with
  `-D warnings`. `clippy::pedantic` is not enabled.
- Commit messages: **Conventional Commits** — `feat:`, `fix:`, `docs:`,
  `test:`, `refactor:`, `chore:`, `ci:` (the `ci:` prefix is reserved for
  changes under `scripts/` and `.github/`). English, imperative mood,
  body wrapped at ~72 columns. The design-system plan uses scoped
  variants like `docs(design):` and `feat(brand):`; both are accepted.
- No emoji in code, comments, or commit messages. The single exception
  documented in `DESIGN.md` §9 is the Braille-pattern spinner glyph
  `⠋` (U+280B), which is a Unicode block-element character, not an
  emoji.
- Prefer `tracing::info!` / `tracing::debug!` over `println!`/`eprintln!`
  in library code. `stdout` is reserved for CLI data output (the list
  table, success lines); diagnostics go to `stderr` or to `tracing`.

## Reporting issues

The project's issue tracker is the standard place for bug reports, feature
requests, and security disclosures. *(A tracker URL will be added here once
the public tracker is provisioned; until then, open a draft PR or email
the maintainers listed in `memory-bank/`.)* Please include:

- The SupaZip version (`supazip-cli --version` for the CLI; the
  `About` dialog for the GUI).
- The OS and architecture.
- The exact command or UI action that reproduced the issue.
- For security reports, see the security note at the bottom of
  [README.md](../README.md) — do not open a public issue for undisclosed
  vulnerabilities.

## License

SupaZip is dual-licensed under **MIT** or **Apache-2.0**, at your option.
By submitting a contribution (code, docs, design tokens, i18n strings,
brand assets), you agree to license your contribution under both licenses
on the same terms. See `LICENSE-MIT` and `LICENSE-APACHE` at the repo
root for the full texts.

## Contact

The `AGENTS.md` file at the repo root documents the Cursor agent roles
(Stack, Architect, Code, Debug, Ask) and how to enable them in the
project rules picker. When you open a PR, **tag the relevant role** in
the PR description — for example, "Code" for a Rust change, "Architect"
for a boundary or token change, "Debug" for a regression. This routes
the PR to the right reviewer and keeps the review queue sane.
