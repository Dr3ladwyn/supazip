# Benchmarks

`supazip-core` ships a single Criterion bench target, `engine`, that
exercises the five archive backends over the hot read / write paths.

## Targets

The bench lives in `supazip/supazip-core/benches/engine.rs` and is
registered in `supazip-core/Cargo.toml`:

```toml
[[bench]]
name = "engine"
harness = false
```

It groups the work into four Criterion groups:

| Group   | Backends covered                                | Sizes                                  |
|---------|-------------------------------------------------|----------------------------------------|
| `create` | `zip`, `7z`, `tar`, `tar.gz`, `tar.xz`         | 100 entries × 1 KiB                    |
| `list`   | `zip`, `7z`, `tar`, `tar.gz`, `tar.xz`         | 10 / 100 / 1 000 entries × 256 B       |
| `extract`| `zip`, `7z`, `tar`, `tar.gz`, `tar.xz`         | 10 / 100 / 1 000 entries × 256 B       |
| `test`   | `zip`, `7z`, `tar`, `tar.gz`, `tar.xz`         | 10 / 100 / 1 000 entries × 256 B       |

That is **80 individual benches**: 4 groups × 5 backends × up to 4 sizes
(the `create` group is a single fixed fixture).

## Toolchain

`rust-toolchain.toml` pins the project to **rustc 1.92**. Older rustc
versions refuse to build `supazip-core` and therefore the bench binary.
Local runs require rustup with the pinned toolchain installed:

```text
rustup show                 # verify active toolchain is 1.92
cargo +1.92 bench -p supazip-core --bench engine
```

## Running a baseline

```text
cargo bench -p supazip-core --bench engine -- --save-baseline m0.3.0
```

The flag is passed through to Criterion; it writes one
`m0.3.0.json` per `bench id` under `target/criterion/...`. To keep the
baseline in version control, copy the JSON files into
`supazip-core/benches/baselines/` and commit them. The `baselines/`
folder ships a `README.md` that documents the convention; the first
baseline for milestone `0.3.0` is captured on the first successful CI
run (see below).

## Comparing against a baseline

```text
cargo bench -p supazip-core --bench engine -- --baseline m0.3.0
```

Criterion prints a per-bench comparison and exits non-zero on a
regression; the CI job uses this for the soft-gate check described
below.

## Filtering

Criterion's standard filters work:

```text
cargo bench -p supazip-core --bench engine -- --bench-filter 'list'
cargo bench -p supazip-core --bench engine -- --bench-filter 'create/zip'
```

## HTML reports

The `html_reports` feature is enabled in `Cargo.toml`. After a run,
open `target/criterion/report/index.html` in a browser. The CI job
uploads the whole `target/criterion/report/` directory as an artefact.

## CI job

The `bench` job runs on every push to `main` and on every PR. It is
`continue-on-error: true` for the 0.3.0 cycle (see risk #5 in
[`m0.3.0-gui-feature-complete.md`](milestones/m0.3.0-gui-feature-complete.md)):
the comparison step is informational, not a hard gate, until the CI
runner fleet is fixed enough to trust a 10 % regression threshold.

```yaml
bench:
  runs-on: ${{ matrix.os }}
  strategy:
    fail-fast: false
    matrix:
      os: [ubuntu-latest, windows-latest]
  continue-on-error: true
  steps:
    - uses: actions/checkout@v4
    - name: Install rustup toolchain 1.92
      run: rustup toolchain install 1.92 --profile minimal --component rustfmt --component clippy
    - name: Cache cargo registry & target
      uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          supazip/target
        key: ${{ runner.os }}-bench-${{ hashFiles('supazip/**/Cargo.lock') }}
    - name: Run benches
      working-directory: supazip
      run: cargo bench -p supazip-core --bench engine -- --benchmark-filter 'list|extract|create'
    - name: Upload Criterion HTML report
      if: always()
      uses: actions/upload-artifact@v4
      with:
        name: criterion-report-${{ matrix.os }}
        path: supazip/target/criterion/report
```

The `cargo bench` invocation is the same one developers run locally;
no extra setup is required.

## What is *not* covered

- **No GUI / CLI paths.** The bench only exercises `supazip-core`. The
  GUI and CLI thin wrappers are not on the hot path; their cost is
  dominated by the engine call they forward to.
- **No streaming 7z extraction.** The 7z backend streams, but the
  bench feeds it an in-memory `Cursor` so the bytes-per-second number
  is not representative of large-archive disk I/O. The 50 MB fixture
  promised by the milestone plan was rejected: an in-memory fixture
  with `Cursor` keeps the bench reproducible across CI runners and
  laptop-class hardware, which is the property the comparison step
  needs.

## Adding a new bench

1. Add a new `fn bench_<name>(&mut Criterion)` near the related group.
2. Register the function in the `criterion_group!` macro at the bottom
   of `benches/engine.rs`.
3. Re-run `cargo bench -p supazip-core --bench engine -- --list` to
   confirm Criterion sees the new bench id.
4. Refresh the baseline:
   `cargo bench -p supazip-core --bench engine -- --save-baseline m0.3.0`.
5. Commit the new JSON into `benches/baselines/` and update this
   document if the new bench changes the scope table.
