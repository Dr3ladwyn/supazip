# Screen Reader Smoke Test Instructions — SupaZip

> **Date:** 2026-06-07
> **Version:** 0.5.0-dev
> **Companion doc:** [`a11y-audit.md`](a11y-audit.md)

---

## 1. Known Limitations

egui 0.34 does **not** expose an accessibility tree on desktop builds (Windows, macOS, Linux). The framework renders to a native GPU window via wgpu/winit, bypassing platform accessibility APIs (UIA/MSAA on Windows, NSAccessibility on macOS, AT-SPI on Linux).

Screen reader testing is therefore **best-effort** until egui adds native a11y support (targeted: egui 0.35+). The tests below document the expected behavior once a11y support lands, and verify that the current UI does not crash screen readers or produce confusing output.

---

## 2. Prerequisites

- SupaZip GUI built and runnable: `cargo run -p supazip-gui`
- At least one test archive: a plain `.zip` and an encrypted `.7z` (with known password)
- A second monitor or virtual desktop is recommended (screen reader announcements are easier to track when the terminal is visible alongside the GUI)

---

## 3. NVDA (Windows)

### Setup

1. Download NVDA from <https://www.nvaccess.org/download/>.
2. Install and launch NVDA. A system tray icon appears; NVDA starts speaking immediately.
3. NVDA key by default is `Insert`. Narrator (built into Windows 10/11) can be used as a fallback with `Win+Ctrl+Enter`.

### Smoke Test (10 steps)

| # | Action | Expected announcement | Status |
|---|--------|----------------------|--------|
| 1 | Launch SupaZip GUI (`cargo run -p supazip-gui`) | NVDA announces the window title "SupaZip" | TODO |
| 2 | Press `Tab` repeatedly | NVDA cycles through toolbar buttons: "Open…", "Recent", "Extract", "Create…", "Test" | TODO |
| 3 | Press `Tab` to reach the empty-state heading | NVDA announces "SupaZip" (heading) and the instruction label | TODO |
| 4 | Press `Ctrl+O` and open a `.zip` file | Status bar changes; NVDA may or may not announce the new status (egui limitation) | TODO |
| 5 | Press `Tab` to enter the entry grid | NVDA reads the first row: index, name, size, encrypted | TODO |
| 6 | Press `Down Arrow` to move through entries | NVDA reads each row's name and size | TODO |
| 7 | Right-click an entry | Context menu opens; NVDA reads "Extract here", "Extract to…", "Test entry", "Copy path" | TODO |
| 8 | Press `Esc` to close context menu | Menu closes; focus returns to the entry grid | TODO |
| 9 | Open an encrypted archive → password dialog appears | NVDA announces "Password required" dialog title and the text input field | TODO |
| 10 | Type a password and press `Enter` | Dialog closes; archive loads or error is announced in the status bar | TODO |

### Notes for NVDA

- NVDA may announce "unknown" for egui widgets that do not expose accessibility information. This is expected behavior given the egui limitation.
- If NVDA goes silent during navigation, press `NVDA+Tab` to read the current focus.
- Use `NVDA+Down Arrow` (read from cursor) to hear the full status bar text.

---

## 4. VoiceOver (macOS)

### Setup

1. Enable VoiceOver: `Cmd+F5` (or System Settings → Accessibility → VoiceOver).
2. VoiceOver key (`VO`) is `Ctrl+Option` by default.
3. Open SupaZip: `cargo run -p supazip-gui`.

### Smoke Test (10 steps)

| # | Action | Expected announcement | Status |
|---|--------|----------------------|--------|
| 1 | Launch SupaZip | VoiceOver announces window name "SupaZip" | TODO |
| 2 | Press `VO+Right Arrow` to navigate toolbar | VoiceOver reads "Open… button", "Recent button", "Extract button", "Create… button", "Test button" | TODO |
| 3 | Navigate to the central panel | VoiceOver reads the heading "SupaZip" and the empty-state instruction | TODO |
| 4 | Press `Cmd+O` via the menu bar or `Ctrl+O` shortcut, open a `.zip` | Archive loads; VoiceOver may or may not announce the status change | TODO |
| 5 | Navigate into the entry grid | VoiceOver reads column headers: "#", "Name", "Size", "Encrypted" | TODO |
| 6 | `VO+Right Arrow` through rows | VoiceOver reads each entry's name and size | TODO |
| 7 | Open context menu on an entry (`VO+Shift+M` or right-click) | VoiceOver reads menu items: "Extract here", "Extract to…", "Test entry", "Copy path" | TODO |
| 8 | Close context menu with `Esc` | Menu closes | TODO |
| 9 | Open encrypted archive → password dialog | VoiceOver announces the dialog and the password input field | TODO |
| 10 | Submit password with `Enter` | Dialog closes; archive loads or error is spoken | TODO |

### Notes for VoiceOver

- VoiceOver interaction model requires `VO+Shift+Down Arrow` to "interact" with a group (e.g., the toolbar or grid). If arrow keys do not navigate within a group, try interacting first.
- VoiceOver may read egui canvas elements as "group" without further detail. This is the expected egui desktop limitation.

---

## 5. Orca (Linux)

### Setup

1. Install Orca: `sudo apt install gnome-orca` (GNOME) or use your distro's package manager.
2. Enable Orca: `Super+Alt+S` (GNOME default toggle).
3. Open SupaZip: `cargo run -p supazip-gui`.

### Smoke Test (10 steps)

| # | Action | Expected announcement | Status |
|---|--------|----------------------|--------|
| 1 | Launch SupaZip | Orca announces the window title | TODO |
| 2 | Press `Tab` through toolbar | Orca reads button labels | TODO |
| 3 | Navigate to empty-state area | Orca reads the heading and instruction | TODO |
| 4 | `Ctrl+O` to open archive | Archive loads; status bar may or may not be announced | TODO |
| 5 | `Tab` into the entry grid | Orca reads the first row | TODO |
| 6 | `Down Arrow` through entries | Orca reads each row | TODO |
| 7 | Right-click an entry | Context menu opens; Orca reads menu items | TODO |
| 8 | `Esc` closes menu | Focus returns to grid | TODO |
| 9 | Open encrypted archive → password dialog | Orca announces dialog and input field | TODO |
| 10 | `Enter` submits password | Dialog closes; result announced or error spoken | TODO |

### Notes for Orca

- Orca requires AT-SPI2 support in the application. egui desktop does not currently register with AT-SPI; Orca will likely read "unknown" or nothing for most widgets.
- If Orca does not announce the window at all, the egui window may need an explicit accessible name set via winit. File an upstream egui issue if this occurs.

---

## 6. Results

| Platform | Tester | Date | Result | Notes |
|----------|--------|------|--------|-------|
| NVDA (Windows) | TODO | — | — | — |
| VoiceOver (macOS) | TODO | — | — | — |
| Orca (Linux) | TODO | — | — | — |

> **Instructions for testers:** Fill in the "Status" column in each smoke test table above with one of:
> - **PASS** — behavior matches expected
> - **PARTIAL** — some elements announced, others silent
> - **FAIL** — screen reader crashes, hangs, or produces dangerously misleading output
> - **N/A** — not testable due to egui limitation
>
> Then fill in the Results table here with your findings.

---

## 7. Future Work (egui 0.35+)

When egui ships native accessibility support:

1. Re-run all three smoke test suites.
2. Verify that toolbar buttons, entry grid rows, and dialog controls are announced with correct names, roles, and states.
3. Add `aria-live="polite"` to the status bar widget for real-time state announcements.
4. Add `role="table"` / `role="row"` / `role="columnheader"` to the entry grid.
5. Test with additional screen readers: Narrator (Windows), ChromeVox (web build).
6. Update this document with actual test results and close the TODO items.
