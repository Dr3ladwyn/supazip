# Progress

## Done

- [x] Phase 0: read project docs, memory-bank, and source; ran git
  recon; ran `cargo build --workspace` to establish a baseline.
  Findings logged in `activeContext.md`.
- [x] Phase 1: `.gitignore` at repo root, top-level `README.md`, full
  memory-bank refresh (`architect.md` rewritten from scratch; the other
  six files filled in with real current content).
- [x] Phase 1: fixed `supazip-core` baseline compile errors caused by
  `zip 2.4.2` API drift (`try_to_datetime` removed, `set_password`
  removed, `ZipError → io::Error` no longer direct).
- [x] Phase 2: `supazip-cli` implements `list`, `extract`, `create`,
  `test` with `clap` derive, password support, `--all`/`--entry` for
  extract, and a `--format` override for create.
- [x] Phase 2: integration test in `supazip-cli/tests/cli_round_trip.rs`
  creates a ZIP via the `zip` crate, then runs the CLI's `list` and
  `extract` against it and asserts on the output.
- [x] Phase 3: 7z encrypted create now uses `set_content_methods` with
  `Aes256Sha256` + LZMA2 — no more `let _pwd = password`.
- [x] Phase 3: `ArchiveEntry::encrypted` for 7z is now derived from
  the entry's coder type instead of hard-coded to `false`.
- [x] Phase 3: `list` and `test` stream the reader; one buffering pass
  in 7z for the upstream `Read + Seek` requirement, with a comment.
- [x] Phase 3: `extract` and `test` no longer clone the archive buffer
  a second time; single pass over the data.
- [x] Phase 3: `traits.rs` now has a `// design note:` block explaining
  the `dyn` vs generics choice.

## Doing

- [x] Phase 4: WS-F (native menu bar, milestone 0.3.0) on top of
  the in-flight WS-A/B/D/E changes. New `supazip-gui/src/menubar.rs`
  owns `MenuAction` (8 variants) and `MenuActionOutcome`; the headless
  `AppController::dispatch_menu_action` handles ToggleDebug / Close /
  About in place, and the GUI front-end handles the rfd / viewport
  side effects. `cargo fmt -p supazip-gui --check` is clean; local
  rustc is still 1.88 so a full `cargo check` is blocked until the
  toolchain is upgraded (see `activeContext.md`).

## Next

- [ ] Phase 4: split changes into logical commits and push.
- [ ] `supazip-gui`: actual PeaZip-style layout (separate effort;
  blocked locally by rustc version).
- [ ] CI / GitHub Actions for `cargo test -p supazip-core -p supazip-cli`.
- [ ] Virtual scrolling for very large archives (Phase 4 in `plan.md`).
- [ ] Drag-and-drop, recent files, keyboard shortcuts. (WS-A, WS-D,
  WS-E, WS-F in the 0.3.0 plan are in flight; WS-B context menu is
  already on `master`.)
