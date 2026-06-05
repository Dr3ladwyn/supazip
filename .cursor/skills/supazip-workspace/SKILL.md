---
name: supazip-workspace
description: >-
  SupaZip Rust workspace layout, crate boundaries, and stack. Use when editing
  supazip-core, supazip-gui, or supazip-cli; when adding features or dependencies;
  or when library API details matter (egui, zip, sevenz-rust, tokio).
---

# SupaZip workspace

## Layout

- Workspace root: `supazip/Cargo.toml` — members `supazip-core`, `supazip-gui`, `supazip-cli`.
- **supazip-core**: archive engine only — no GUI. Formats (7z, ZIP), `ArchiverError`, `ArchiveFormat`, operations (list/extract/create/test), progress callbacks.
- **supazip-gui**: `eframe` + `egui` desktop UI; depends on `supazip-core` via path; `rfd` for native file dialogs.
- **supazip-cli**: CLI over the same core (optional path per project plan).

High-level design and traits: see repository root `plan.md`.

## Stack (verify in crate `Cargo.toml` before coding)

- Archives: `sevenz-rust`, `zip` (major versions matter for APIs).
- GUI: `eframe`, `egui` (immediate mode; version-sensitive).
- Async / FS: `tokio` where used in core or GUI.
- Errors: `thiserror` in core.

## Documentation

When generating or refactoring against external crates, prefer **Context7** MCP (`resolve-library-id`, `query-docs`) so examples match current library versions. Mention the crate version from `Cargo.toml` in the doc query when relevant.

## Conventions

- Keep **core free of egui/eframe** imports; UI calls into core APIs only.
- Match existing module names and error types in the repo before introducing new patterns.
