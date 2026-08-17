# Publishing SupaZip to crates.io

This document is the runbook for publishing the `supazip-core` and
`supazip-cli` crates to [crates.io](https://crates.io). It exists because
the publish is the **only** distribution action in milestone 0.2.0 and the
team must be able to repeat it deterministically on a maintainer's laptop.

The GUI crate (`supazip-gui`) is **not** published: it is the desktop
application front-end, depends on `eframe` / `egui` / `rfd`, and is only
useful as a binary. Its `Cargo.toml` carries `publish = false` for that
reason. The `LICENSE` headers (dual MIT / Apache-2.0) are identical across
all three crates, so this is purely a packaging decision, not a licensing
one.

## Scope

- `supazip-core` (library; published).
- `supazip-cli` (binary; published; depends on `supazip-core`).
- `supazip-gui` (binary; **not** published; `publish = false`).

## Pre-publish checklist

A maintainer must be able to tick every box before running `cargo publish`.

- [ ] All four backends merged on `master`: ZIP, 7z, TAR, TAR.GZ (TAR.XZ is
      the bonus from milestone 0.2.0).
- [ ] `cargo test -p supazip-core -p supazip-cli` green on the pinned
      toolchain (`1.92`).
- [ ] `cargo clippy -p supazip-core -p supazip-cli --no-deps -- -D warnings`
      green.
- [ ] `cargo fmt --all -- --check` green.
- [ ] `cargo doc -p supazip-core -p supazip-cli --no-deps` produces no
      warnings. Internal links in the rustdoc resolve.
- [ ] `Cargo.lock` is committed and `Cargo.toml` workspace
      `members = [...]` matches reality.
- [ ] `LICENSE-MIT` and `LICENSE-APACHE-2.0` exist at the repo root and
      are referenced by the SPDX identifier `MIT OR Apache-2.0` in every
      crate's `[package] license = ...` field.
- [ ] `[package]` metadata in `supazip-core/Cargo.toml` and
      `supazip-cli/Cargo.toml` carries: `description`, `license`,
      `authors`, `repository`, `readme`, `keywords`, `categories`. See the
      **Metadata reference** section below.
- [ ] `CHANGELOG.md` has a fresh `## [X.Y.Z] - YYYY-MM-DD` entry at the
      top. Move the previous `[Unreleased]` content into the new section
      if it was empty.
- [ ] The `vX.Y.Z` Git tag has **not** been pushed yet; the publish and
      the tag push happen on the same commit, in that order, so the tag
      and the crates.io release are always in lockstep.
- [ ] The maintainer running the publish has a `crates.io` account that
      owns both crate names (`supazip-core` and `supazip-cli`). If the
      names are not yet reserved, claim them through
      <https://crates.io/me> before tagging.

## Metadata reference

The `Cargo.toml` of each published crate must carry the following
metadata. Anything missing will make `cargo publish` warn or refuse.

| Field          | Value                                                      |
|----------------|------------------------------------------------------------|
| `version`      | `"X.Y.Z"` (semver; no leading `v`).                        |
| `edition`      | `"2021"`.                                                  |
| `rust-version` | `"1.92"` (pinned toolchain; see `rust-toolchain.toml`).    |
| `description`  | One sentence, 50–150 chars; describes the crate's role.   |
| `license`      | `"MIT OR Apache-2.0"` (SPDX dual-licence identifier).      |
| `authors`      | `["SupaZip contributors"]` for now; expand when authors    |
|                | add their handles.                                         |
| `repository`   | `"https://github.com/your-org/supazip"` placeholder is OK  |
|                | for the first cut.                                         |
| `readme`       | `"../../README.md"` (relative to the crate root).          |
| `keywords`     | ≤ 5 from the [crates.io keyword list](https://crates.io/    |
|                | keywords). For SupaZip: `archive`, `7z`, `zip`, `tar`,     |
|                | `compression` (core) / `cli` (cli).                        |
| `categories`   | ≤ 5 from the [crates.io category list](https://crates.io/  |
|                | category_slugs). For SupaZip: `compression`,               |
|                | `command-line-utilities`, `filesystem`.                    |
| `publish`      | `false` on `supazip-gui/Cargo.toml`; absent on the other   |
|                | two crates (default `true`).                               |
| `exclude`      | `["/.github", "/.cursor", "/scripts", "/target", ...]` to  |
|                | keep the published tarball small.                          |

## The two-step publish

`supazip-core` and `supazip-cli` form a workspace. `supazip-cli` depends
on `supazip-core`, and at publish time the `path = "../supazip-core"`
dependency is **replaced** by the registry version. This means the two
crates must be published sequentially: core first, then cli, with enough
time for crates.io to re-index between the two uploads.

```bash
# One-time per machine:
cargo login                # paste the crates.io API token

# Confirm the metadata is acceptable without uploading:
cargo publish --dry-run --allow-dirty --no-verify -p supazip-core
cargo publish --dry-run --allow-dirty --no-verify -p supazip-cli

# Real publish — order matters:
cargo publish -p supazip-core
# Wait ~30 s while crates.io re-indexes the new 0.2.0 version of
# supazip-core; the cli dry-run / publish will fail with
# "no matching package named `supazip-core` found" if the cli is uploaded
# before core is visible on the index.
cargo publish -p supazip-cli
```

The `--allow-dirty` flag is only needed when validating the metadata in
the same working tree that just had `Cargo.toml` edits; in a clean tagged
commit the flag is unnecessary. `--no-verify` skips the `cargo build`
verification step (which requires the pinned rustc 1.92 toolchain to be
on `PATH`); the maintainer may omit it on a clean 1.92 toolchain.

## Why publish is manual, not CI

`cargo login` takes a long-lived API token. Embedding that token in a CI
secret, or worse in a public workflow, would expose the entire
`supazip-core` / `supazip-cli` namespace to anyone who can read the
runner logs. Crates.io does not support OIDC trust yet. Until it does,
the publish is a maintainer-only step run from a local machine after the
tag is decided.

A future workstream can add a `release.yml` workflow that *builds* the
source tarball and the SHA-256 sums and attaches them to the GitHub
Release; that flow does not need a crates.io token.

## Post-publish sanity check

Once both crates are on crates.io:

```bash
# Force a clean install from the registry to confirm the published
# metadata is consumable end-to-end:
cargo install --force supazip-cli
supazip-cli --version    # expected: supazip-cli 0.2.0
```

Then tag and push:

```bash
git tag v0.2.0            # the commit that bumped Cargo.toml
git push origin v0.2.0    # triggers the GitHub Actions release workflow
```

Downstream projects (the consumer side, not in this repo) bump their
`Cargo.toml` dependency to `supazip-core = "0.2.0"` and run
`cargo update` to pull the new release.

## Local verification commands

This is the same set the milestone's WS-F definition of done requires,
run from the workspace root `supazip/`:

```bash
cargo test  -p supazip-core -p supazip-cli
cargo clippy -p supazip-core -p supazip-cli --no-deps -- -D warnings
cargo doc   -p supazip-core -p supazip-cli --no-deps
cargo publish --dry-run --allow-dirty --no-verify -p supazip-core
cargo publish --dry-run --allow-dirty --no-verify -p supazip-cli
```

The `supazip-cli` dry-run is expected to fail with "no matching package
named `supazip-core` found" until `supazip-core 0.2.0` is actually on
crates.io. Treat that failure as a positive signal: the cli manifest
correctly resolves the core dependency from the registry, and is ready
to upload the moment core is indexed.

## Open questions for the next milestone

- Should the published crates opt in to a `[badges]` block pointing at
  the GitHub Actions workflow and a future `cargo llvm-cov` badge?
  Deferred to 0.4.0 per `ROADMAP.md` Track C.
- Should the repository URL move from `your-org/supazip` placeholder to
  the real GitHub owner? Yes — the first thing the maintainer does
  immediately before `cargo login` is update the `repository` field.
- Is `cargo install supazip-cli` enough for headless servers, or do we
  need a `.deb` / `.rpm` / Homebrew tap? Tracked under
  `ROADMAP.md` → Track E → 1.0-rc.1.
