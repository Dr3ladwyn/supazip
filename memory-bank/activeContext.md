# Active Context

## Current state

- **v1.0.0** tagged at `aff00b6` (2026-06-07). All 6 milestones complete.
- Repository is **clean** (only PeaZip submodule marker is dirty — pre-existing, unrelated).
- 76 commits on `master`, 6 tags: `v0.2.0`, `v0.3.0`, `v0.4.0`, `v0.5.0`, `v1.0-rc.1`, `v1.0.0`.
- ~175 tracked files, ~190 unit tests.
- Local toolchain: rustc 1.88.0 (project requires 1.92). GUI crate cannot build locally; CI (windows-latest, ubuntu-latest, macos-latest) is the authoritative gate.

## Current blockers

- **No blockers** for the 1.0.0 release itself.
- `cargo push` requires a crates.io API token (not yet provided).
- `git push` requires remote origin configured and credentials.

## Post-release tasks

1. Replace `<owner>` placeholder in `packaging/`, `README.md`, `SECURITY.md`, `docs/release-process.md`, `docs/coverage.md`, `.github/workflows/release.yml` with real GitHub handle.
2. Replace PLACEHOLDER SHA-256 hashes in packaging manifests with real values from first GitHub Release.
3. Replace `assets/minisign.pub` placeholder with real minisign public key.
4. Run `cargo publish -p supazip-core`, wait 30s, `cargo publish -p supazip-cli`.
5. Enable Codecov integration (set `CODECOV_TOKEN` secret).
6. Configure Apple Developer secrets for macOS notarization.
