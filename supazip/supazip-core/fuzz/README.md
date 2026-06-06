# `supazip-core` fuzz harnesses

This sub-crate is the `cargo-fuzz` skeleton for [`supazip-core`](../).
It is **not** a member of the top-level Cargo workspace (see the empty
`[workspace]` table at the bottom of `fuzz/Cargo.toml`); the main
`cargo build` and `cargo test` runs on stable Rust do not touch it.

## Targets

Each backend × operation pair is its own fuzz target, totalling **15
harnesses** (4 backends × 3 ops + 3 for the tar/tar.gz/tar.xz split
counted as separate backends per `docs/milestones/m0.2.0-engine-solid.md`).
The harness count matches WS-C of the 0.2.0 milestone plan.

| Backend   | `list`         | `extract`         | `create`         |
|-----------|----------------|-------------------|------------------|
| `zip`     | `zip_list`     | `zip_extract`     | `zip_create`     |
| `7z`      | `sevenz_list`  | `sevenz_extract`  | `sevenz_create`  |
| `tar`     | `tar_list`     | `tar_extract`     | `tar_create`     |
| `tar.gz`  | `targz_list`   | `targz_extract`   | `targz_create`   |
| `tar.xz`  | `tarxz_list`   | `tarxz_extract`   | `tarxz_create`   |

Each harness looks up its backend through the `supazip_core::BACKENDS`
registry. If the backend has not been registered yet (the `tar*`
backends land in WS-B, which is in flight), the harness returns
immediately and is a **no-op** rather than a panic. This keeps the
skeleton compilable today and ready to run as soon as each backend is
wired up.

## Requirements

`cargo-fuzz` requires a **nightly** Rust toolchain because of the
`-Z` flags it needs (`-Z` sanitizer support, `-Z` build-std for
coverage, etc.). The fuzz directory's `Cargo.toml` is a normal
manifest and the targets themselves use only stable Rust; only the
`cargo fuzz` driver needs nightly.

Install both on a developer machine:

```bash
rustup install nightly
cargo install cargo-fuzz
```

The CI smoke job installs nightly and the `cargo-fuzz` crate in its
own environment — see the `fuzz-smoke` job in
[`../../.github/workflows/ci.yml`](../../.github/workflows/ci.yml).

> The local toolchain on the build machine that produced this
> skeleton is **stable 1.92 only** (no nightly, no `cargo-fuzz`).
> This is fine for compiling the rest of the workspace, and the
> `fuzz/` sub-crate is excluded from the workspace precisely so the
> stable build is unaffected.

## Running a single target

From `supazip/supazip-core/`:

```bash
cargo +nightly fuzz run zip_list
```

This will:

1. Build the harness binary with sanitizer + coverage instrumentation.
2. Use the corpus at `fuzz/corpus/zip_list/` (created on first run;
   commit useful seed inputs here as you find them).
3. Save crashing inputs to `fuzz/artifacts/zip_list/`.

A standard run never exits on its own; use `-- -max_total_time=N` for
a time-bounded session (see CI smoke below).

## 60-second CI smoke

The CI job runs two representative harnesses for **60 seconds each**:

```bash
cargo +nightly fuzz run zip_list       -- -max_total_time=60
cargo +nightly fuzz run sevenz_extract -- -max_total_time=60
```

These two are the only targets that exercise backends which already
existed before 0.2.0 (the `tar*` backends are in flight under WS-B and
their harnesses are no-ops until the registry knows about them). The
job is **not a hard gate** during 0.2.0: it is `continue-on-error: true`
and acts as a regression alarm only. Promotion to a hard gate is a
0.4.0 deliverable per `ROADMAP.md` → Track C.

## Adding a new target

1. Copy the closest existing `*_<op>.rs` file under `fuzz_targets/`.
2. Replace the extension string in the `get_backend("...")` call with
   the new backend's primary extension.
3. Add a row to the table above.
4. Update the smoke job in `.github/workflows/ci.yml` if the target
   should be exercised in CI.

## Triage

When a crash is found:

1. The reproducer lives at `fuzz/artifacts/<target>/crash-<sha>.bin`.
2. Minimise it: `cargo +nightly fuzz tmin <target> <crash-file>`.
3. Decode it with the harness to confirm the panic site.
4. File a regression in the milestone issue tracker and link the
   minimised reproducer in the issue body.

The full triage runbook is in `docs/fuzzing.md` (added by WS-D in
0.4.0); for 0.2.0 the steps above are sufficient.
