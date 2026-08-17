# Fonts — JetBrains Mono

SupaZip uses **JetBrains Mono** as its single typeface for body, heading, and
monospace UI text. This document explains why, where to get it, and how to
install it locally for development and GUI testing. The font binary itself is
**not** committed to this repository — every developer fetches it directly
from the upstream source.

> **Status (design system v2):** `supazip-gui` calls `theme::install_fonts` on
> startup. Embedding is **compile-time gated**: if
> `assets/fonts/JetBrainsMono-Regular.ttf` is present, `build.rs` sets
> `embed_jetbrains_mono` and the TTF is baked in via `include_bytes!`.
> The OFL TTF is **not** in this repository (and must not be committed
> without `OFL.txt` alongside it). Until that file is added, the GUI keeps
> egui's built-in / system fallback. See [§5](#5-code-integration).

---

## 1. Choice — why JetBrains Mono

- **Modern terminal aesthetic** — matches the product positioning ("a polished
  7z/ZIP tool that feels at home next to a developer's shell"). A monospaced
  face keeps table columns, byte counters, and progress bars perfectly
  aligned in every locale.
- **Monospaced by design** — every glyph occupies the same advance width, so
  file lists, CRC values, and timing readouts stay visually scannable.
- **Latin + Cyrillic + Greek coverage** — required for the planned EN+RU
  i18n target. JetBrains Mono ships full Cyrillic and Greek ranges, so
  Russian/Ukrainian menus and error messages render without tofu boxes.
- **Programmer-friendly hinting** — distinct `0`/`O`, `1`/`l`/`I`, and
  ligatures for `=>`, `!=`, `>=` (we keep ligatures **off** by default to
  preserve plain ASCII in log output).
- **SIL Open Font License 1.1** — permissive, allows redistribution and
  embedding; compatible with closed-source shipping of the SupaZip binary.

Alternatives considered and rejected:

| Candidate | Reason rejected |
|-----------|-----------------|
| Cascadia Code | Good fallback, but slightly heavier; no Cyrillic in older builds. |
| Fira Code | Nice ligatures; weaker Cyrillic hinting at small sizes. |
| Inter / Roboto | Not monospaced — breaks table alignment. |
| DejaVu Sans Mono | Too "old-school" for the target aesthetic. |

---

## 2. Download

Official source (do **not** mirror to this repo):

- Project page: <https://www.jetbrains.com/lp/mono/>
- Direct release archives: <https://github.com/JetBrains/JetBrainsMono/releases>

Recommended files (Static fonts, version 2.304 or newer):

- `JetBrainsMono-Regular.ttf` — body / default weight
- `JetBrainsMono-Bold.ttf`    — headings, active rows, keybindings

Optionally, when the same files are also needed in the browser (e.g. for
marketing screenshots):

- `JetBrainsMono-Regular.woff2` — variable-font web build (smaller).
  **Not used by `supazip-gui`** — egui's `set_fonts` expects TTF/OTF, not
  WOFF2. Keep WOFF2 around only for the docs site.

Verify the checksum against the release page after downloading.

---

## 3. License

JetBrains Mono is released under the **SIL Open Font License, version 1.1**
(OFL 1.1). Full text: <https://scripts.sil.org/OFL>.

Key points relevant to SupaZip:

- Free to use, embed, and redistribute (with the OFL notice preserved).
- Derivative works (e.g. our own hinting tweaks) are allowed and must
  themselves stay under OFL.
- The font may be bundled with a closed-source product.

**This repository does not redistribute the font.** Each contributor and
end-user installs it locally. The OFL notice in the GUI's "About" dialog
will credit JetBrains in a later release.

---

## 4. Install paths

Pick the procedure that matches your development OS. After installing,
restart the SupaZip GUI so it re-reads the system font list.

### Linux

```bash
mkdir -p ~/.local/share/fonts
cp JetBrainsMono-Regular.ttf JetBrainsMono-Bold.ttf ~/.local/share/fonts/
fc-cache -f
fc-list | grep -i jetbrains   # confirm registration
```

### macOS

1. Open Finder and navigate to the downloaded `.ttf` files.
2. Double-click `JetBrainsMono-Regular.ttf` → click **Install Font**.
3. Repeat for `JetBrainsMono-Bold.ttf`.
4. Open **Font Book** → search "JetBrains Mono" to verify.

The files land in `~/Library/Fonts/`.

### Windows

1. Right-click `JetBrainsMono-Regular.ttf` → **Install for all users**
   (writes to `C:\Windows\Fonts\`).
2. Repeat for `JetBrainsMono-Bold.ttf`.
3. Win + R → `control fonts` → confirm both weights appear.

> **Note:** "Install" (per-user) is fine for personal use. For SupaZip
> development, prefer **Install for all users** so the GUI picks it up
> regardless of the user the app is launched as.

---

## 5. Code integration

`supazip-gui` wires fonts in `theme::install_fonts`, called from
`eframe::CreationContext` in `main.rs`. `build.rs` probes
`assets/fonts/JetBrainsMono-Regular.ttf` (and optionally the Bold cut).
When the file exists, `include_bytes!` embeds it and both `Proportional`
and `Monospace` families put `jetbrains_mono` first.

**Embedding lands when the OFL TTF is added** to `assets/fonts/` together
with `OFL.txt`. Until then the helper is a no-op and egui keeps its
built-in fallback chain. Do not commit a TTF without the license file.

---

## 6. Verification

After installing the font and rebuilding `supazip-gui`:

1. Launch the app (`cargo run -p supazip-gui`).
2. Open an archive with a few entries — the file list and toolbar should
   render in JetBrains Mono (look for the distinctive `g` and `0`).
3. Switch the UI language to Russian (or any Cyrillic locale) and confirm
   the menu/toolbar/table headers render crisply — **no missing-glyph
   boxes**.
4. Take a screenshot and compare against the design system reference
   board (see `plan.md` for the latest mockups).
5. Optional: from the app's debug overlay (right-click → *Inspect* in dev
   builds), inspect `Context.fonts()` and confirm `jetbrains_mono` is
   listed first under both `Proportional` and `Monospace` families.

If the GUI still renders in the system default font, double-check the
install path in §4 and restart the app — egui caches the font list at
startup.

---

## 7. Fallback chain

If JetBrains Mono is missing on the host (e.g. a contributor who skipped
§4), the GUI should fall back through the following CSS-style stack before
landing on the platform monospace:

```
JetBrains Mono,
"Cascadia Code",
"Fira Code",
"SF Mono",
Menlo,
Consolas,
monospace
```

When `set_fonts` is wired up, the `FontDefinitions::fallback_fonts` list
will be populated from this chain. The order matters: most-platform on top
(Win/macOS/Linux dev machines usually have Cascadia or Fira), legacy
defaults at the bottom, and the generic `monospace` keyword as the last
resort so the app never falls back to a proportional face.

---

## 8. Updating the font

When a new JetBrains Mono release ships:

1. Download the new `Regular.ttf` and `Bold.ttf` (and `Italic` variants if
   added to the design system).
2. Locally replace the files you installed in §4.
3. Bump the version pin in the future `supazip-gui/Cargo.toml` metadata
   (planned alongside the wiring PR).
4. Re-run the verification in §6 and update the design system reference
   board.

Do **not** commit the TTF binaries without `OFL.txt` in the same
directory. When the licensed files are added, the next `supazip-gui`
build embeds Regular (and Bold if present) automatically.
