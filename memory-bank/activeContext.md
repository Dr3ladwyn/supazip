# Active Context

## Current focus

- 2026-06-05: Three-phase roadmap kicked off.
  - **Phase 0** complete. Findings: no git repo, no rustup, `cargo 1.88.0`
    on the machine; `eframe 0.34.1` needs rustc 1.92 so `supazip-gui` is
    build-blocked locally until the toolchain is upgraded. `supazip-core`
    has stale `zip 2.x` API calls and does not build either; fixing them
    is part of Phase 1 hygiene.
  - **Phase 1** (hygiene) in progress: `.gitignore`, top-level `README.md`,
    memory-bank refresh, plus baseline compile fix for `supazip-core`.
  - **Phase 2** (CLI) and **Phase 3** (core quality) queued.

## Current blockers

- No git repository at the repo root → commits in Phase 4 will require
  `git init` first. Will do that before any commit step and call it out in
  the final report.
- Local rustc is 1.88.0, egui 0.34.1 wants 1.92 → GUI crate is not
  buildable here. Will be left alone (it is already a stub) and CI / a
  newer toolchain will be the path forward for it.
