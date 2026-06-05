# Project Brief

## Purpose

SupaZip exists to give Windows / macOS / Linux users a modern, native,
open-source archive manager focused on the two open formats that actually
matter: **7z** and **ZIP**. It exists to be scriptable as well as
graphical — the same engine powers both `supazip-gui` and `supazip-cli`.

The longer-term ambition (per `plan.md`) is a PeaZip-class GUI with
virtual scrolling, drag-and-drop, and a job queue. The current code does
not deliver that yet; the engine and CLI do.

## Target users

- **Desktop power users** who currently use 7-Zip or PeaZip and want a
  modern alternative that does not depend on a fork of legacy C++.
- **Developers / sysadmins** who script archive operations and want a
  CLI whose behavior matches the GUI.
- **Contributors** who want a small, focused Rust codebase where the
  archive logic is in one crate, the GUI is in another, and the
  boundaries are crisp.
