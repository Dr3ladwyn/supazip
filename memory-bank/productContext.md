# Product Context

## Overview

SupaZip is a desktop archive manager for Windows / macOS / Linux, written in
Rust. The user-facing product is a PeaZip-style graphical application for
browsing, creating, extracting, and testing archives in 7z and ZIP formats.
A CLI front-end ships alongside, built on the same engine, for headless and
scripted use.

## Core features (current and planned)

- Archive **list** (name, size, compressed size, encrypted flag, modified
  time, compression method).
- Archive **extract** — all entries or a named subset, with optional
  password.
- Archive **create** — from a list of files on disk, in 7z or ZIP, with
  optional password (AES-256 for 7z).
- Archive **test** — verify integrity (CRCs / checksums).
- GUI: a single window with toolbar, address bar, file list, status bar,
  progress overlay, password dialog. **Not yet built** — placeholder
  window only.
- Drag-and-drop, recent files, virtual scrolling, keyboard shortcuts —
  **planned, not started** (see `plan.md` Phase 4).

## Technical stack

See `README.md` "Tech stack" for pinned versions. Summary:

- Engine (`supazip-core`): `sevenz-rust 0.6.1` + `zip 2.4.2` + `thiserror`.
- GUI (`supazip-gui`): `egui` / `eframe 0.34.1` + `rfd 0.17.2`.
- CLI (`supazip-cli`): `clap 4.6` derive.
- Cross-cutting: `tokio 1.50`, `tracing 0.1`, `chrono 0.4`, `tempfile 3`
  (dev).

## Target users

- Power users who already use 7-Zip / PeaZip and want a modern, native,
  open-source alternative focused on the two dominant open formats.
- Developers and sysadmins scripting archive work from a terminal.
- Anyone who wants the same operations available in a GUI and in scripts,
  with no behavioral drift between the two.
