# SupaZip — Design system v2

> **Status:** v2, dark + light. Canonical reference for every UI change in the GUI
> (`supazip-gui`) and every visual change in the CLI (`supazip-cli`).
> **Source of truth:** [`design/tokens.yaml`](design/tokens.yaml) and its JSON
> mirror [`design/tokens.json`](design/tokens.json). This document explains the
> *why*; the tokens file holds the *what*.
>
> Token `meta.version` is `2.0.0`. Colour lives under `themes.dark` and
> `themes.light`; shared scales stay at the document root. The GUI will consume
> tokens via `Style::from_tokens` (may land in a parallel commit). Until that
> mapping ships, literals in `supazip-gui` must still match the token file.

## Scope and audience

- **Surfaces in scope.**
  - GUI: `supazip-gui` (eframe/egui 0.34) — toolbar, status bar, central panel
    (entry grid), error and status text.
  - CLI: `supazip-cli` (clap) — `list` subcommand table, status lines, error
    output, progress messages on stderr.
  - Brand: SVG logo (mark + lockup) in `assets/logo.svg`, `assets/logo-mark.svg`.
- **In scope for 1.1 (this contract).** Light theme, embedded JetBrains Mono
  (OFL), monoline toolbar icons, `Style::from_tokens` as the runtime path, and
  driving `cmd_list` from [`design/cli-table.tera`](design/cli-table.tera).
- **Surfaces explicitly out of scope.** Archive editor / in-place mutate,
  Material / Tailwind / Aqua chrome. See "Out of scope" near the end.
- **Audience.** Power users who currently live in PeaZip, 7-Zip, or WinRAR.
  They open archives in the dozens per day, navigate them with the keyboard,
  pipe the CLI into shell scripts, and notice every dropped frame and every
  off-by-one column.

The current state of the application is visible in:

- `supazip/supazip-gui/src/main.rs` — toolbar (`Open…`, `Extract`, `Create…`,
  `Test`, `Cancel`), the status bar with `ui.spinner()` and `state.status`, the
  four-column striped entry grid (`#`, `Name`, `Size`, `Encrypted`), and the
  empty-state vertical-centred heading.
- `supazip/supazip-gui/src/lib.rs` — `AppState` defaults
  (`status = "Open an archive to get started."`), `EngineEvent` variants
  (`Listed`, `Done`, `Error`), and the `error: <msg>` prefix applied to every
  error.
- `supazip/supazip-cli/src/main.rs` — `IDX` / `METHOD` / `SIZE` / `COMPRESSED`
  / `CRYPT` / `NAME` table header in `cmd_list`, a `-`.repeat(72) rule line,
  the `"<N> entries"` footer, `"OK: <path>"` / `"extracted to <path>"` /
  `"created <path> (N entries)"` success lines, the `"error: <err>"` prefix
  on stderr, the `"interrupted"` cancellation message, and the
  `SUPAZIP_MAX_ARCHIVE_SIZE` warning text.

All visible strings in those three files will be replaced by the i18n table in
`assets/i18n/{en,ru}.toml` (Step 5 of the plan). This document fixes the
visual treatment that surrounds those strings.

---

## 1. Aesthetic direction

**One sentence.** A modern terminal: precise, dense, calm, no-nonsense.

**Reference points (positive).**

- *Sublime Text* — dense monospace UI, restrained colour, no chrome that
  does not earn its pixels.
- *kitty / Rio* — sharp text rendering, generous tab-stop discipline, light
  use of underline and box-drawing characters to communicate structure.
- *Windows Terminal* — clean dark background, single accent colour used
  sparingly for focus and progress.

**Anti-references (do not chase).**

- *macOS Aqua* — translucency, soft drop-shadows, large rounded controls.
  Wrong mood; our users consider them slow.
- *Material Design* — elevation through shadow, spring physics. We are not
  building a touch consumer app.
- *Web 2.0 glossy* — gradients, bevels, "shine on hover". Off-brand; will
  not survive contact with a power user judging you on millisecond latency.

**Mood board.** *Precise. Dense. Calm. Quiet. Honest.*

The GUI is one window: a thin toolbar at the top, a dense entry grid in the
middle, a one-line status bar at the bottom. No welcome screen, no
illustration, no marketing line. When the window is empty, the central
panel shows two lines of monospace text — the app name, then a single
instruction ("Open an archive (7z or ZIP) to view its contents."). No
animation, no ghost grid.

The CLI's `list` subcommand is a single fixed-width table printed once. No
animation, no progress bar (the `StderrProgress` sink in
`supazip/supazip-cli/src/main.rs` is intentionally quiet — high-frequency
updates would spam the terminal).

---

## 2. Brand voice

**Tone.** Short verbs in the imperative. No exclamation marks, no rhetorical
questions, no "we", no "thanks for trying SupaZip". When something works, say
what happened. When something fails, say what happened and stop.

**Toolbar labels (imperative, present tense).**

- `Open…` — the ellipsis is the platform's signal that a dialog will follow.
- `Extract` — verb, not "Extract files". The destination will be obvious from
  the file dialog that opens next.
- `Create…` — also a dialog. No "New archive" or "Archive…" — those read as
  nouns.
- `Test` — verb, no "Test integrity". The result line will be "OK" or an error.
- `Cancel` — only visible while a long operation is in flight.

**Status-line conventions (the bar at the bottom of the GUI).**

- Idle: `"Open an archive to get started."` (default in `AppState::default`).
- Loading: `"opening <path>…"`, `"extracting <archive> to <out>…"`,
  `"creating <target> (<n> entries)…"`, `"testing <archive>…"`. The trailing
  ellipsis is the in-flight marker; it disappears when the result lands.
- Result: `"loaded"`, `"OK: <path>"`, `"extracted to <path>"`,
  `"created <path>"`.
- Cancellation: `"operation cancelled"`.
- Error: `"error: <what went wrong>"` — lowercase prefix, never capitalised,
  never with an emoji, never with a marketing "Sorry!".

**CLI conventions.**

- Success: bare line, no prefix. `"extracted to /tmp/out"`,
  `"created /tmp/a.zip (3 entries)"`, `"OK: /tmp/a.zip"`, `"3 entries"`.
- Warning: `"warning: <msg>"` (e.g. unparseable `SUPAZIP_MAX_ARCHIVE_SIZE`).
- Error: `"error: <err>"` on stderr. Same prefix style as the GUI.
- Cancellation: `"interrupted"` (lowercase, no punctuation). Exit code 130.

**What we never write.** "Oops!", "Something went wrong", "Whoopsie",
"Uh oh", "Failed to". "Error: could not open file" is fine; "Sorry, we
could not open your file" is not.

**Voice in i18n.** Russian translation keeps the same imperatives in the
same register. We do not switch to formal "Вы"; the audience is the same
power users reading the same screen.

---

## 3. Layout principles

**Three regions, fixed edges, fluid middle.**

```
+----------------------------------------------------------+
| toolbar (32px, fixed top)                               |
+----------------------------------------------------------+
|                                                          |
|  central panel (flex)                                    |
|  - heading "<path> (<backend>)"                          |
|  - striped Grid with 4 columns                           |
|                                                          |
+----------------------------------------------------------+
| status bar (24px, fixed bottom)                          |
+----------------------------------------------------------+
```

**Mapping to `supazip-gui/src/main.rs`.**

- The top `egui::Panel::top("toolbar")` is the toolbar.
- `egui::Panel::bottom("statusbar")` is the status bar.
- `egui::CentralPanel::default()` is the central panel; inside it, a
  `ScrollArea::vertical()` holds a `Grid::new("entries").striped(true)`
  with four columns: index, name, size, encrypted.

**Sizing.**

- Toolbar height: **32 px** including 8 px vertical padding.
- Status bar height: **24 px** including 4 px vertical padding.
- Central panel: fills the remainder; never scrolls horizontally; only
  vertically, and only when the entry count exceeds the visible area.
- Window minimum size: 480 × 360. Below that, the central panel cannot
  show even a single entry row at a comfortable column width.

**Geometry.**

- Interactive elements (buttons, the file-dialog trigger) have a **4 px
  corner radius**. Panels (toolbar, status bar, central panel background)
  are square, **0 px corner radius** — they are the canvas, not the
  content.
- Window margin: **0 px** at top and bottom (the panels are pinned to the
  edge); **0 px** left/right inside the central panel; the toolbar and
  status bar handle their own padding.
- Toolbar and status bar are full-bleed; only their internal
  `item_spacing` creates the visual padding.
- The empty-state block is centred (`ui.vertical_centered`) but not pushed
  to a particular margin; it sits in the geometric centre of whatever
  window size the user picked.

**Grid layout for the central panel.**

- Column 1 (`#`): right-aligned, **4 characters** wide (`{:>4}` in
  `supazip-cli/src/main.rs`). Matches the CLI table exactly.
- Column 2 (`Name`): left-aligned, expands to fill the remainder. The grid
  should not truncate names — the row's full text must be available either
  visually (no horizontal scroll) or via clipboard.
- Column 3 (`Size`): right-aligned, **12 characters** wide
  (`{:>12}`). Same width as the CLI. Numbers formatted via
  `format_size` (see CLI styling) so 1 024 B renders as `1.0 KiB`, not
  `1024 B`.
- Column 4 (`Encrypted`): left-aligned, narrow, value is `yes` or `-`.
- Row height: **24 px** (compact). The grid is `striped(true)` so even rows use
  `color.bg.sunken` and odd rows use `color.bg.raised` from the **active
  theme** (`themes.dark` or `themes.light`). This is the spreadsheet look
  the audience expects.

**Toolbar layout.**

- `ui.horizontal` with `item_spacing = 8`.
- Buttons left-to-right in execution order: `Open…`, `Extract`, `Create…`,
  `Test`, then a flex spacer, then `Cancel` (only visible while busy).
- The `Cancel` button is the only element right-aligned in the toolbar. Its
  colour is the semantic `danger` token, not the accent, because cancel is a
  destructive interrupt.

**Status bar layout.**

- `ui.horizontal` with `item_spacing = 6`.
- When busy, the leading widget is `ui.spinner()` (a small indeterminate
  glyph, see motion). The text label is `&self.ctrl.state().status`.
- When idle, only the text label is present.

---

## 4. Density

**Compact, not crowded.** Power users want to see more rows; that does not
mean we squeeze pixels.

- `item_spacing` globally: **8 px**.
- Window margin: **12 px** (the gap between the window edge and the
  toolbar/central panel — we will allow 0 on the pinned edges and 12 on the
  central panel's left/right when the grid renders; central panel
  content is the only thing with a margin).
- Button padding: **(8, 4)** — 8 px horizontal, 4 px vertical.
- Button height: **24 px** for toolbar buttons; **20 px** for in-grid or
  dialog actions (the `Create…` and `Extract` confirmation buttons inside a
  dialog).
- Row height in the entry grid: **24 px**.
- Line height: **1.4** for body, **1.2** for headings, **1.6** for the
  empty-state subtitle so it reads as a paragraph.
- Cell padding in the grid: `(4, 0)` — 4 px horizontal only. The vertical
  rhythm comes from the 24 px row height.

**Why these numbers.** They match the `JetBrains Mono` metrics at 12 px
body: a 24 px row is exactly two body lines with 4 px of breathing room on
each side, and the 32 px toolbar fits a 14 px heading plus 8 px of
top/bottom padding. Deviations (a 28 px row, 10 px `item_spacing`) will
create a visibly inconsistent rhythm — a known smell in terminal-style
UI.

**Status bar density.** Single line, no wrapping. If the status message is
longer than the available width, truncate from the start with an ellipsis so
the most recent state ("extracting…") is always visible. Long path messages
keep the filename, drop the directory prefix. (This is implementation
detail; the design says "single line, never wrap, always show the verb".)

---

## 5. Motion

**Three durations, one easing.**

- `fast` (80 ms) — hover state changes (button background, link underline,
  icon swap).
- `normal` (160 ms) — panel open/close, dropdown expand/collapse, dialog
  fade-in.
- `slow` (320 ms) — progress-bar indeterminate animation only. Nothing else
  in v1 takes this long; the eye reads it as "actively working", not "loading
  a screen".

**Easing.**

- `standard: cubic-bezier(0.2, 0, 0, 1)` — the default for every state
  transition.
- `decel: cubic-bezier(0, 0, 0, 1)` — for things that come *from* off-screen
  (e.g. a future toast that slides up).
- `accel: cubic-bezier(0.3, 0, 1, 1)` — for things leaving the screen.

**What we never animate.**

- Bounce. No spring physics, no overshoot, no "elastic" easings. Power users
  read overshoot as jank.
- Colour flashes on error. The status bar text changes; that is enough. No
  red flash, no shake, no scale-pulse.
- The entry grid rows. Selection moves instantly. Hover state changes at
  `fast` (80 ms) — that is the only motion in the grid.
- The empty state. There is nothing to animate.

**What we do animate.**

- Button hover/press (`fast`).
- Toolbar button enable/disable — colour transition at `fast`, no scale.
- Indeterminate progress (the `ui.spinner()` in the status bar) at `slow`,
  `accel`. This is the only place the user expects to see something moving;
  it is the explicit "we are working" signal.
- Dialog open/close (`normal`, `standard`). Dialogs are rare (file/folder
  pickers are OS-native via `rfd`, so this only applies to the future
  password prompt and the create-archive confirm).

**Reduced-motion preference.** The OS-level "reduce motion" setting should
be honoured: when set, all durations collapse to `0` and the indeterminate
spinner is replaced by a static "Working…" label. Implementation is in
scope for 1.1 alongside `Style::from_tokens`.

---

## 6. Iconography

**v1 shipped text-only in the toolbar.** Buttons read `Open…`, `Extract`,
`Create…`, `Test`, `Cancel`. Text stays the primary label: it is
unambiguous, localises trivially, and renders identically across the font
fallback chain (JetBrains Mono, Cascadia Mono, Consolas, system mono).

**v2 / 1.1 (in scope).** Monoline SVG strokes in the toolbar — not PNG
skins, not emoji. 14 px height, 1.5 px stroke, five glyphs: open-folder,
extract-up, create-plus, test-check, cross-cancel. Icons sit beside the
existing labels; they do not replace them. Until the glyphs land, the
toolbar remains text-only and typography stays the brand.

**Where icons may appear.**

- The logo. `assets/logo-mark.svg` (24×24 viewBox, `currentColor` stroke,
  1.5 px stroke) and `assets/logo.svg` (192×48 lockup). The mark is a
  stylised zip-lock with a four-point spark; the lockup is the mark plus the
  wordmark in JetBrains Mono Bold 24 px.
- Status indicators. A small bullet (`•`) in the success status line; an
  em-dash (`—`) on idle. These are characters, not icons, but they carry
  semantic colour (success / muted). They are not used inside dense tables
  — tables stay pure text so the column width is predictable.

**Where icons never appear.**

- The entry grid. No per-row status icons; the `Encrypted` column is
  `yes` or `-` and that is the entire signal.
- The CLI. CLI is plain text only, fixed-width (optional TTY colour on
  stderr does not add glyphs).
- The empty state. The app name and one sentence; nothing else.

**Anti-pattern.** Do not paste an icon next to a label as decoration. Every
icon either carries semantic meaning (logo, status bullet, toolbar glyph)
or it is not there.

---

## 7. Accessibility commitments

**Conformance target.** WCAG 2.1 AA. Every visible string is checked at the
colour-token level for **both** themes (see §10). Dark tokens were audited
against `#0E1116` for 1.0. Light tokens meet AA on `#F6F8FA` at the YAML
level; a GUI re-audit is required after `Style::from_tokens` lands (see
[`docs/a11y-audit.md`](docs/a11y-audit.md)).

### Keyboard

- **Tab / Shift+Tab** — moves focus through interactive elements in DOM order.
  The order is: toolbar buttons left-to-right, then the central panel
  contents (entries, if any), then the status bar's actionable elements (a
  future "Copy status" button; nothing in v1).
- **Enter / Space** — activates the focused button or toggles the focused row.
- **Esc** — closes any open dialog (file picker, future password prompt) and
  cancels the in-flight operation when no dialog is open. The `Cancel`
  button is keyboard-equivalent to the toolbar `Cancel`.
- **Arrow keys (in the central panel)** — moves row selection up/down. v1.1
  adds this; v1 ships with click-only selection but the focus order is set up
  so that arrows only need to be wired in the controller, not the layout.
- **No keyboard trap.** Every focusable element can be left via Tab or Esc.

### Visible focus

- **Focus ring:** 2 px solid `color.border.focus` (same hex as
  `color.accent.primary` in the active theme), inset 2 px from the element
  edge. Implemented as a stroke *around* the element, not as a colour swap,
  so the focus state is visible against both the toolbar
  (`color.bg.surface`) and the central panel (`color.bg.base`).
- **Contrast of focus ring against any background it overlays:** ≥ 3:1
  (WCAG 1.4.11 non-text contrast). Verified at the token level.

### Screen reader

- Every `egui::Button` gets a `WidgetInfo` label derived from the i18n
  table. The visible text and the announced text are the same string; we
  do not maintain a separate `aria-label` layer.
- The status bar's live region is `polite` (not `assertive`): a new
  error message is announced but does not interrupt the user mid-typing.
  The exact wiring lives in v1.1.
- The entry grid exposes row count and the focused row's name and size.

### Contrast (WCAG 2.1 AA)

Token paths below are relative to the active theme
(`themes.dark.color.*` / `themes.light.color.*`). Ratios are measured
against that theme's `color.bg.base`.

**Dark** (`themes.dark`, `bg.base` = `#0E1116`)

| Token                        | Hex      | Ratio  | Use                              |
|------------------------------|----------|--------|----------------------------------|
| `color.fg.primary`           | `#E6EDF3`| 14.5:1 | Body text, default label         |
| `color.fg.secondary`         | `#9DA7B3`| 6.2:1  | Captions, column headers         |
| `color.fg.muted`             | `#6E7681`| 4.6:1  | Disabled controls, idle hints    |
| `color.fg.inverse`           | `#0E1116`| n/a    | Text on accent backgrounds       |
| `color.accent.primary`       | `#58A6FF`| 6.8:1  | Focus ring, primary action       |
| `color.accent.primary_hover` | `#79B8FF`| 8.6:1  | Hover state of primary           |
| `color.accent.pressed`       | `#388BFD`| 5.4:1  | Pressed state                    |
| `color.semantic.success`     | `#3FB950`| 5.4:1  | "loaded", "OK", success status   |
| `color.semantic.warning`     | `#D29922`| 5.0:1  | Warnings (e.g. size limit)       |
| `color.semantic.danger`      | `#F85149`| 5.1:1  | "Cancel" button, error prefix   |
| `color.semantic.info`        | `#58A6FF`| 6.8:1  | Informational labels             |

**Light** (`themes.light`, `bg.base` = `#F6F8FA`)

| Token                        | Hex      | Ratio  | Use                              |
|------------------------------|----------|--------|----------------------------------|
| `color.fg.primary`           | `#1F2328`| 14.8:1 | Body text, default label         |
| `color.fg.secondary`         | `#59636E`| 5.7:1  | Captions, column headers         |
| `color.fg.muted`             | `#636C76`| 5.0:1  | Disabled controls, idle hints    |
| `color.fg.inverse`           | `#FFFFFF`| n/a    | Text on accent backgrounds       |
| `color.accent.primary`       | `#0969DA`| 4.9:1  | Focus ring, primary action       |
| `color.accent.primary_hover` | `#0550AE`| 7.1:1  | Hover state of primary           |
| `color.accent.pressed`       | `#033D8B`| 9.6:1  | Pressed state                    |
| `color.semantic.success`     | `#166C2E`| 6.1:1  | "loaded", "OK", success status   |
| `color.semantic.warning`     | `#8A5C00`| 5.5:1  | Warnings (e.g. size limit)       |
| `color.semantic.danger`      | `#CF222E`| 5.0:1  | "Cancel" button, error prefix   |
| `color.semantic.info`        | `#0969DA`| 4.9:1  | Informational labels             |

Large text (≥ 18 px or ≥ 14 px bold) is held to 3:1 minimum; all of the
above clears that bar with margin. Light ratios are token-level (from
`tokens.yaml` 2.0); they are not a substitute for a GUI walkthrough after
`Style::from_tokens` lands.

### Touch targets

- All toolbar buttons are **32 × 24** at minimum, exceeding the 24 × 24
  WCAG 2.5.5 target. The toolbar itself is 32 px tall, so vertical target
  is the row height.
- The status bar's interactive surface (in v1, none) must be **24 px
  tall**.

### What we explicitly do not promise in v2

- Full keyboard reconfiguration. Keybindings are not user-customisable.
- A separate high-contrast palette. Dark and light are the two shipped
  themes; a future "system" follow may derive a high-contrast variant from
  the same token shape, but that is not 1.1.
- Screen-magnifier optimisation beyond what `egui` already provides.

---

## 8. Internationalisation

**Languages in v1:** English (`en`) and Russian (`ru`).

**Storage.** `assets/i18n/en.toml` and `assets/i18n/ru.toml`. Keys in
dot-notation, 28 keys total. The full key list and the English source text
are in Step 5 of the plan; this document fixes the *conventions* the
translators must follow.

**Pluralisation.** Not used in v1. All countable strings use a single
placeholder (`{n}`) and the call site is responsible for picking the right
form. Example: the entry footer `"3 entries"` becomes `"3 записи"` for
`n % 10 == 1 && n % 100 != 11`; we sidestep this by keeping the count
display as `"3 entries"` always, until a v1.1 pluralisation helper lands.

**Placeholders.** Three forms appear in v1:

- `{path}`, `{archive}`, `{out}`, `{target}`, `{err}`, `{ext}` — single
  string, no formatting.
- `{n}` — integer count.
- `{raw}` — verbatim env-var value (the SUPAZIP warning).

Placeholders must match between `en.toml` and `ru.toml`; the i18n sync
script (`design/scripts/check_i18n.py`, Step 7) verifies this.

**Order of placeholders.** Placeholders appear in the source order; in
Russian they may need to be re-ordered for natural phrasing. The script
checks for *presence*, not *order*; reviewers are responsible for natural
Russian word order.

**Punctuation.** English uses ASCII period and comma. Russian uses
`U+00A0` (non-breaking space) after a short word and before an em-dash
where applicable, but in v1 we restrict visible punctuation to
`! ? , . : ; … -` (U+2026 horizontal ellipsis in both languages — the
trailing triple-dot is fine in English and Russian). The ellipsis in
`Open…` and `Create…` is U+2026.

**Fonts.** JetBrains Mono (see §10) ships with full Latin and Cyrillic
coverage, plus a generous set of box-drawing and Braille pattern glyphs.
v2 / 1.1 **embeds** the OFL TTF (Regular + Bold) so the GUI does not depend
on a system install. Source, license, and intended `set_fonts` shape stay
in [`assets/fonts/README.md`](assets/fonts/README.md). Fallback chain is
unchanged: JetBrains Mono, Cascadia Mono, Consolas, `ui-monospace`.

**Default language.** English at first launch. The user can switch via a
(hidden in v1) env-var `SUPAZIP_LANG=ru`. There is no language picker UI in
v1; the audience is bilingual and the env-var is the right surface for a
power user.

**Typography sizes in i18n.** Body 12 px, body-large 14 px, heading 16 px,
title 20 px. Russian text at 12 px is comfortably readable in JetBrains
Mono; the wider glyphs in Cyrillic do not cause line-wrap regressions
relative to the English text in the toolbar.

**What we explicitly do not ship in v1.** Right-to-left languages, locale
formats (dates, decimal separators), translated docs. These are deferred.

---

## 9. CLI styling

**The CLI is plain text. It is fixed-width, ASCII-safe, and never coloured
by default.**

### Output channels

- **stdout** — data: the list table, the `<N> entries` footer, and the
  success lines.
- **stderr** — diagnostics: the `warning:` line for unparseable
  `SUPAZIP_MAX_ARCHIVE_SIZE`, the `interrupted` cancellation message, and
  the `error: <err>` line.

This split is already in `supazip/supazip-cli/src/main.rs`: `println!` for
data, `eprintln!` for diagnostics. We do not change that.

### The `list` table

A fixed-width table with six columns, printed exactly as it is in
`cmd_list`:

```
  IDX  METHOD         SIZE       COMPRESSED  CRYPT    NAME
────────────────────────────────────────────────────────────────────────
    0  deflate            4 B            12  -        hello.txt
    1  deflate            4 B             8  -        data.bin

3 entries
```

Column rules (these are not aspirational — they are what the existing code
in `supazip-cli/src/main.rs` already does, and Step 3 of the plan freezes
them in [`design/cli-table.tera`](design/cli-table.tera)):

| Column      | Width | Align | Format                              |
|-------------|-------|-------|-------------------------------------|
| `IDX`       | 4     | right | decimal, 0-padded implicitly        |
| `METHOD`    | 12    | left  | truncate to 11 + `…` if longer      |
| `SIZE`      | 12    | right | `format_size` (B / KiB / MiB / GiB)  |
| `COMPRESSED`| 12    | right | `format_size`                       |
| `CRYPT`     | 8     | left  | `yes` or `-`                        |
| `NAME`      | rest  | left  | no truncation, no wrap              |

The `format_size` helper renders integer bytes below 1 KiB (`"4 B"`), one
decimal place from 1 KiB up to 1 GiB (`"1.0 KiB"`, `"512.0 MiB"`), and two
decimals for sub-1 KiB values that we want to keep readable. Negative or
overflow values are rendered as `-`.

The rule line is `─` (U+2500, BOX DRAWINGS LIGHT HORIZONTAL) repeated 72
times, matching the table width. It is exactly one line; no double rule,
no box-drawing corners.

The footer is a blank line followed by `<N> entries`, where `<N>` is the
decimal count. No "Total: …" line in v1; v1.1 may add it.

### Success / error lines

- Success on stdout, no prefix, no colour:
  - `"extracted to <path>"`
  - `"created <path> (<n> entries)"`
  - `"OK: <path>"`
- Warning on stderr: `"warning: <msg>"` — used by
  `effective_limits()` in `supazip-cli/src/main.rs` when
  `SUPAZIP_MAX_ARCHIVE_SIZE` cannot be parsed.
- Error on stderr: `"error: <err>"` — every `ArchiverError` flows through
  this. The `eprintln!("error: {err}")` line in `main` is the single
  source for this prefix.
- Cancellation: `"interrupted"` on stderr, lowercase, exit code 130.

### Progress output

The `StderrProgress` sink prints messages via `set_message` as
`"  <message>"` on stderr (note the two-space indent to keep it visually
distinct from user-visible lines). The `set_progress` callback is
intentionally a no-op in v1 — high-frequency progress lines would spam the
terminal and serve no purpose for a CLI invocation that is short-lived.

The `⠋` Braille-pattern character (U+280B) is reserved for a future
indeterminate progress indicator in the CLI. The character is a
"Braille Pattern" symbol, not an emoji, and is allowed by the design
system's "no emoji" rule.

### i18n

Every visible CLI string — column headers, the `<N> entries` footer, the
`about` / `long_about` strings, the warning text — lives in
`assets/i18n/{en,ru}.toml` and is looked up via the same helper the GUI
uses. The CLI does not use clap's `gettext` integration in v1; it
reads the TOML file directly. (Implementation is in v1.1; the
contract — "all visible strings come from the i18n table" — is fixed in
v1.)

### Anti-patterns

- ANSI colour on stdout, or colour when stdout/stderr is not a TTY.
  Default remains plain text. v2 / 1.1 may emit semantic colour on
  **stderr** only when it is a TTY (`anstyle` / `supports-color`), driven
  by the active theme tokens. Piped output stays uncoloured.
- Spinners, ticks, or carriage-return overwrites on stdout. The CLI prints
  once and exits.
- Localised "Loading…" / "Please wait". The CLI does not pretend to be
  interactive.

---

## 10. Link to tokens

**Source of truth.**

- [`design/tokens.yaml`](design/tokens.yaml) — human-readable, hand-written.
- [`design/tokens.json`](design/tokens.json) — generated mirror, validated
  by `design/scripts/check_tokens.py` (Step 7).

**Why YAML is the source.** It is the format most natural to read in a
code review. JSON is generated, not authored, so a contributor never
edits the JSON by hand. The CI step will fail if the two diverge.

**What lives in the tokens file.**

- `meta` — name, version, generator marker. Bumped manually on breaking
  changes. Current: `2.0.0`.
- `themes.dark` / `themes.light` — each owns a full `color` tree:
  `bg`, `fg`, `accent`, `semantic`, `border`. Contrast is WCAG-AA checked
  against **that theme's** `color.bg.base` (`#0E1116` dark, `#F6F8FA`
  light). There is no top-level `color` map in 2.0; code that used
  `color.fg.primary` now resolves `themes.<name>.color.fg.primary`.
  Elevation strings interpolate `{color.border.subtle}` from the active
  theme.
- `typography` — `family`, `size`, `weight`, `line_height`. The `family`
  block is `JetBrains Mono` for `body`, `heading`, and `mono`. The
  fallback chain is system-defined in code: `JetBrains Mono`,
  `Cascadia Mono`, `Consolas`, `ui-monospace`.
- `spacing` — 0, 1, 2, 3, 4, 5, 6, 7, 8 → 0, 2, 4, 8, 12, 16, 24, 32, 48
  px. A linear scale; half-steps are not allowed.
- `radius` — `none` 0, `sm` 2, `md` 4, `lg` 8, `full` 9999. The
  GUI uses `md` (4 px) on interactive elements, `none` on panels.
- `motion` — `duration` (`instant` 0, `fast` 80, `normal` 160, `slow` 320
  ms) and `easing` (`standard`, `decel`, `accel`). See §5.
- `elevation` — `level_0` none, `level_1` a 1 px solid bottom border in
  `color.border.subtle`, `level_2` a 1 px solid border plus a faint drop
  shadow. The GUI uses `level_1` for the toolbar/status bar separator and
  `level_2` for dialogs (password, progress, settings). Do not use
  Material-style stacked shadows.
- `z_index` — `base` 0, `dropdown` 100, `modal` 200, `tooltip` 300. egui
  does not surface a z-index knob to us; the values are reserved for the
  future.

**How the GUI uses the tokens.** The intended runtime path is
`Style::from_tokens(theme)` mapping YAML into `egui::Visuals` /
`egui::Style` (settings already persist a theme preference; 1.1 adds
`Dark | Light | System`). That helper may land in a parallel commit —
do not assume it is present in every checkout that has tokens 2.0.
Until it lands, the contract is that *every literal number and hex in
the GUI source matches a value in this tokens file* — verified by
`scripts/ci-design.ps1`. Do not pull egui into `supazip-core`; keep the
mapper in `supazip-gui` (or a thin `supazip-theme` crate without egui).

**How the CLI uses the tokens.** Column widths stay a CLI-formatting
concern. 1.1 drives `cmd_list` from [`design/cli-table.tera`](design/cli-table.tera)
instead of a hand-kept format string. Optional TTY colour on stderr may
read semantic tokens from the active theme; stdout stays plain.

**Bumping the version.** `meta.version` is `2.0.0` for this contract.
Breaking changes to the palette or to the `themes.*` shape bump the
minor. Adding a new token without changing an existing one bumps the
patch. The token sync script checks that the JSON mirror reports the
same version.

---

## Out of scope (explicit)

Light theme, embedded JetBrains Mono, and monoline toolbar icons are **in
scope for 1.1** — they are no longer backlog. The items below stay out.
Adding them to a 1.1 PR will be rejected.

- **Archive editor.** In-place add / delete / rename of entries. `ArchiveFormat`
  does not mutate; that is a 1.2 architectural decision.
- **Material / Tailwind / Aqua / Fluent chrome.** No elevation-through-shadow
  as a design language, no translucency, no spring physics, no glossy
  gradients. `elevation.level_2` is a 1 px border plus a faint dialog
  shadow — not Material stacking.
- **Aesthetic reset.** We are not becoming a conventional desktop 7-Zip
  clone. Precise / dense / calm stays.
- **Product 1.2 features.** RAR (external helper only, never default in
  core), async backends, CLI self-update, file associations, text-entry
  preview. See [`docs/milestones/m1.1.0-design-v2.md`](docs/milestones/m1.1.0-design-v2.md).
- **egui_kittest snapshots.** Optional after the 1.1 visual review; not a
  gate for `Style::from_tokens`.
- **High-contrast theme.** Not a third palette in 1.1.

---

## Cross-references

- Plan source (v2): `c:\Users\Admin\.cursor\plans\post-1.0_code_and_design_35058910.plan.md`
- Milestone: [`docs/milestones/m1.1.0-design-v2.md`](docs/milestones/m1.1.0-design-v2.md)
- GUI source: `supazip/supazip-gui/src/main.rs`, `supazip/supazip-gui/src/lib.rs`
- CLI source: `supazip/supazip-cli/src/main.rs`
- Token files: `design/tokens.yaml`, `design/tokens.json` (`meta.version` 2.0.0)
- CLI table layout: `design/cli-table.tera`
- Brand assets: `assets/logo.svg`, `assets/logo-mark.svg`
- i18n table: `assets/i18n/en.toml`, `assets/i18n/ru.toml`, `assets/i18n/de.toml`
- Font documentation: `assets/fonts/README.md`
- A11y: [`docs/a11y-audit.md`](docs/a11y-audit.md)
- Sync scripts: `design/scripts/check_tokens.py`, `design/scripts/check_i18n.py`
- CI hook: `scripts/ci-design.ps1`
