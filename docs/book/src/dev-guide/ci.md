# CI

SupaZip uses GitHub Actions for continuous integration. Workflows live in `.github/workflows/`.

## Workflows

### `ci.yml` — Main CI

Runs on every push and pull request.

| Job | Matrix | What it does |
|-----|--------|-------------|
| `build` | `windows-latest`, `ubuntu-latest`, `macos-latest`, `macos-14` (arm64) | `cargo build --workspace` |
| `test` | Same OS matrix | `cargo test --workspace` |
| `clippy` | `ubuntu-latest` | `cargo clippy --workspace -- -D warnings` |
| `fmt` | `ubuntu-latest` | `cargo fmt --check` |
| `msrv` | `ubuntu-latest` | Build with Rust 1.92 (MSRV) |
| `deny` | `ubuntu-latest` | `cargo deny check advisories bans licenses sources` |

### `coverage.yml` — Test coverage

Runs on push to `main` and on pull requests.

- Uses `cargo llvm-cov` to measure coverage.
- Uploads a coverage badge to the repository.
- Enforces a minimum threshold (60% at 0.3.0, 80% at 0.4.0, 95% at 1.0.0).

### `audit.yml` — Security audit

Runs on a daily cron schedule and on push to `main`.

- `cargo audit` checks for known vulnerabilities in dependencies.
- Results are posted as a GitHub issue if advisories are found.

### `release.yml` — Release pipeline

Triggered by version tags (`v*`).

1. Build release binaries for all four platforms.
2. Generate signed checksums (minisign or cosign).
3. Create a GitHub Release with the checksums attached.
4. (Planned) Publish to crates.io, Homebrew, AUR, winget, scoop.

## Dependency policy

### cargo-deny

`deny.toml` enforces:

- **Advisories:** No known RUSTSEC advisories.
- **Bans:** No duplicate crate versions (except allow-listed exceptions).
- **Licences:** Only MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib, Unicode-DFS-2016.
- **Sources:** No git dependencies (except allow-listed exceptions).

### MSRV matrix

CI builds with both `stable` and `1.92` (the declared MSRV). If a dependency bumps its MSRV past 1.92, the CI job fails and the dependency version must be pinned or the MSRV bumped.

## Benchmarks in CI

The `bench` job runs `cargo bench -p supazip-core` and uploads the HTML report as an artefact. Benchmark baselines are stored per-commit so regressions can be traced.

## Reproducible builds

Milestone 0.4.0 introduces reproducible builds via:

- Pinned `SOURCE_DATE_EPOCH`.
- Sorted zip metadata (entries are written in deterministic order).
- The script `scripts/build-reproducible.sh` produces bit-for-bit identical archives.

The release pipeline includes a verification step that rebuilds the release binaries and compares checksums.

## Local CI reproduction

To run the full CI suite locally:

```bash
# Format check
cargo fmt --check

# Clippy
cargo clippy --workspace -- -D warnings

# Tests
cargo test --workspace

# Deny
cargo deny check

# Audit
cargo audit
```

## Design system CI

The script `scripts/ci-design.ps1` verifies that every literal number in the GUI source matches a value in `design/tokens.yaml`. The script `design/scripts/check_tokens.py` verifies that `tokens.yaml` and `tokens.json` are in sync. The script `design/scripts/check_i18n.py` verifies that `en.toml` and `ru.toml` have the same keys and placeholder sets.
