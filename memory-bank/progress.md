# Progress

## v1.0.0 Released (2026-06-07)

### Milestone chain

- [x] **v0.2.0** Engine solid (2026-06-06): tar/tar.gz/tar.xz backends, brotli/zstd for ZIP, `Limits::max_compression_ratio`, fuzz skeleton, i18n expansion (78 keys), de locale stub.
- [x] **v0.3.0** GUI feature-complete (2026-06-07): drag-and-drop, context menu, progress dialog, recent files, password dialog, native menu bar, proptest round-trips, criterion benchmarks, signed releases.
- [x] **v0.4.0** QA + Security hardening (2026-06-07): cargo-audit/deny (0 vulns), coverage 80%, OS matrix (win/ubuntu/macos), reproducible builds, CLI completions (5 shells), CLI `--output json|yaml|text`, safe_join fix, macOS code-signing, metadata proptests.
- [x] **v0.5.0** i18n + a11y + docs (2026-06-07): hand-rolled plural runtime, de full translation, WCAG 2.1 AA audit (17 PASS, 2 PARTIAL), mdbook (18 files), man-pages, GUI settings persistence + settings window.
- [x] **v1.0-rc.1** Distribution (2026-06-07): brew/aur/winget/scoop/nix manifests, Dockerfile (distroless), .deb/.rpm configs, 5 release targets, CycloneDX SBOM, finalized release workflow.
- [x] **v1.0.0** Production (2026-06-07): SECURITY.md, README production-ready, release announcement template.

### Pre-v0.2.0 work (initial sessions)

- [x] Phase 0: project assessment, memory-bank initialization.
- [x] Phase 1: `.gitignore`, `README.md`, memory-bank refresh, baseline compile fixes.
- [x] Phase 2: CLI (`list`/`extract`/`create`/`test`), integration tests.
- [x] Phase 3: 7z encrypted create, `ArchiveEntry::encrypted`, streaming, error chaining.
- [x] Phase 4: GUI skeleton (eframe/egui), AppController refactor, 10 unit tests.
- [x] Design system v1: `DESIGN.md`, tokens, logos, i18n, CLI table template, validation scripts.

## Done

All roadmap milestones completed. 76 commits, 6 tags (v0.2.0 through v1.0.0).

## Next (post-1.0.0)

- [ ] Replace `<owner>` placeholder in packaging manifests, README, SECURITY.md with real GitHub handle.
- [ ] `git push && git push --tags` to publish.
- [ ] Publish to crates.io (requires API token; see `docs/publishing.md`).
- [ ] **1.1**: light theme, self-update, RAR support, async backends.
