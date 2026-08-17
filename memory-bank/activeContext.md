# Active Context

## Current state

- **v1.0.0** tagged at `aff00b6` (2026-06-07). All 6 milestones complete.
- **1.0.1 Harden** is in progress on `master` (crate versions `1.0.1`, not tagged).
- GitHub identity placeholder is **`your-org/supazip`** everywhere until a real
  repository exists. Do not invent a handle.
- PeaZip is a vendored nested checkout: ignored via `/PeaZip/`, not a submodule.
- Local toolchain: rustc 1.88.0 (MSRV 1.92). Core + CLI compile with
  `--ignore-rust-version`. GUI crate cannot build locally; CI is the
  authoritative gate.

## Current blockers

- `git push` requires remote origin configured and credentials.
- crates.io publish requires an API token (not yet provided).
- Packaging SHA-256 hashes stay `PLACEHOLDER` until the first GitHub Release.
- `assets/minisign.pub` is still a placeholder; release.yml skips minisign
  when `MINISIGN_SECRET_KEY` is unset.

## Post-1.0.1 tasks

1. Replace `your-org` with the real GitHub handle in packaging, README,
   SECURITY.md, crate `repository` URLs, and issue templates.
2. Fill PLACEHOLDER SHA-256 hashes from the first GitHub Release.
3. Replace `assets/minisign.pub` with a real minisign public key.
4. `git push && git push --tags` when a remote exists.
5. Run `cargo publish -p supazip-core`, wait 30s, `cargo publish -p supazip-cli`.
6. Enable Codecov (`CODECOV_TOKEN`) and Apple Developer secrets if needed.
