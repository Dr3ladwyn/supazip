# Criterion baselines

Criterion baselines for `supazip-core`'s engine bench live as a sibling
directory. The default workflow is:

```text
cargo bench -p supazip-core --bench engine -- --save-baseline m0.3.0
```

which writes `target/criterion/<group>/<id>/m0.3.0.json` and a copy in
`target/criterion/<group>/<id>/m0.3.0/`. Those files are intentionally
excluded from git because criterion regenerates them on every run; the
stable, reviewable copy lives in this directory.

## Why this directory is empty

The first baseline for milestone `0.3.0` will be captured on the first
successful CI run of the new `bench` job (defined in
`docs/benchmarks.md`). Local development environments with rustc < 1.92
cannot capture a baseline — see the `rust-toolchain.toml` for the
required toolchain — and the CI fleet is the only place we can pin a
machine-class-specific number.

Once the CI job produces its first `m0.3.0.json`, the JSON is committed
here so it travels with the source tree. PRs that regress a median by
> 10 % are flagged but **not** a hard gate during the 0.3.0 cycle
(see risk #5 in the milestone plan).
