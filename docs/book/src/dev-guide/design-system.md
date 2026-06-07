# Design system

SupaZip's design system is documented in [`DESIGN.md`](../../../DESIGN.md) at the repository root. This chapter provides an overview and links to the canonical reference.

## Aesthetic direction

**One sentence:** A modern terminal — precise, dense, calm, no-nonsense.

Reference points: Sublime Text (dense monospace, restrained colour), kitty/Rio (sharp text rendering, box-drawing), Windows Terminal (clean dark background, single accent colour).

The GUI is one window: a thin toolbar at the top, a dense entry grid in the middle, a one-line status bar at the bottom. No welcome screen, no illustration, no animation.

## Design tokens

The source of truth for all visual values is [`design/tokens.yaml`](../../../design/tokens.yaml) (human-readable) and [`design/tokens.json`](../../../design/tokens.json) (generated mirror). A CI script verifies the two files stay in sync.

### Colour

All colours are WCAG 2.1 AA compliant against `color.bg.base = #0E1116`:

| Token | Hex | Ratio | Use |
|-------|-----|-------|-----|
| `color.fg.primary` | `#E6EDF3` | 14.5:1 | Body text |
| `color.fg.secondary` | `#9DA7B3` | 6.2:1 | Captions, headers |
| `color.fg.muted` | `#6E7681` | 4.6:1 | Disabled controls |
| `color.accent.primary` | `#58A6FF` | 6.8:1 | Focus ring, primary action |
| `color.semantic.success` | `#3FB950` | 5.4:1 | Success status |
| `color.semantic.warning` | `#D29922` | 5.0:1 | Warnings |
| `color.semantic.danger` | `#F85149` | 5.1:1 | Cancel button, errors |

### Typography

- **Font:** JetBrains Mono (Latin + Cyrillic, box-drawing, Braille).
- **Fallback chain:** JetBrains Mono, Cascadia Mono, Consolas, `ui-monospace`.
- **Sizes:** body 12 px, body-large 14 px, heading 16 px, title 20 px.
- **Line heights:** body 1.4, heading 1.2, empty-state 1.6.

### Spacing

Linear scale: 0, 2, 4, 8, 12, 16, 24, 32, 48 px (indices 0–8). Half-steps are not allowed.

### Radius

- `none` (0) — panels (toolbar, status bar, central panel)
- `md` (4) — interactive elements (buttons, dialogs)
- `full` (9999) — reserved for pill shapes

### Motion

| Duration | Value | Use |
|----------|-------|-----|
| `instant` | 0 ms | State changes that must be frame-perfect |
| `fast` | 80 ms | Hover states, button enable/disable |
| `normal` | 160 ms | Panel open/close, dialog fade-in |
| `slow` | 320 ms | Indeterminate progress spinner only |

Easing: `standard` (default), `decel` (entering), `accel` (leaving). No bounce, no spring physics.

## Layout

Three regions: fixed toolbar (32 px), fluid central panel, fixed status bar (24 px). Window minimum: 480 × 360.

The central panel contains a `Grid::new("entries").striped(true)` with four columns: `#` (4 char, right), `Name` (flex, left), `Size` (12 char, right), `Encrypted` (8 char, left). Row height: 24 px.

## Brand voice

Imperative, present tense. No exclamation marks, no "we", no "thanks for trying SupaZip". When something works, say what happened. When something fails, say what happened and stop.

**Never write:** "Oops!", "Something went wrong", "Whoopsie", "Uh oh", "Failed to".

## CLI styling

The CLI is plain text, fixed-width, ASCII-safe, never coloured by default. The `list` table has six columns (IDX, METHOD, SIZE, COMPRESSED, CRYPT, NAME) with a `─` rule line and an `<N> entries` footer.

## Internationalisation

- **Languages:** English (`en`), Russian (`ru`), German (`de` — milestone 0.5.0).
- **Storage:** `assets/i18n/{en,ru,de}.toml`.
- **Placeholders:** `{path}`, `{n}`, `{raw}`.
- **Default:** English at first launch. Override with `SUPAZIP_LANG=ru`.

## Accessibility

- **Target:** WCAG 2.1 AA.
- **Focus ring:** 2 px solid `color.border.focus`, 3:1 contrast ratio.
- **Keyboard:** Tab/Shift+Tab through interactive elements, Enter/Space to activate, Esc to cancel/close.
- **Screen reader:** Every `egui::Button` gets a `WidgetInfo` label from the i18n table.

## Out of scope for v1

Light theme, real PNG icons, embedded TTF, egui_kittest snapshots, Tailwind/Fluent/Material theming. Each is tracked in the v1.1 backlog.
