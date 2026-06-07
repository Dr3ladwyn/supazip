# Security Audits

SupaZip uses **cargo-audit** (RUSTSEC advisory database) and **cargo-deny**
(license, bans, advisories, sources) to keep the dependency tree safe and
compliant. Both run automatically in CI via `.github/workflows/audit.yml`.

## Running locally

### cargo-audit

```bash
cargo install cargo-audit
cd supazip
cargo audit
```

This checks every dependency against the [RUSTSEC advisory database](https://rustsec.org/).
Vulnerabilities are hard failures; unmaintained, yanked, and notice-level
advisories produce warnings.

### cargo-deny

```bash
cargo install cargo-deny
cd supazip
cargo deny check
```

This runs all four checks: **advisories**, **licenses**, **bans**, and
**sources**. You can also run them individually:

```bash
cargo deny check advisories
cargo deny check licenses
cargo deny check bans
cargo deny check sources
```

## Suppressing a RUSTSEC advisory

If a known advisory cannot be fixed yet (e.g. no upstream patch), add the
RUSTSEC ID to `deny.toml` under `[advisories].ignore` with a comment
explaining why:

```toml
[advisories]
ignore = [
    # Waiting for <crate> X.Y.Z which includes the fix; ETA 2026-Q3
    "RUSTSEC-2025-0001",
]
```

Every ignored ID **must** have a comment. CI will not reject the comment-less
form, but it will be rejected in code review.

## Checking licenses

```bash
cargo deny check licenses
```

Allowed licenses are listed in `deny.toml` under `[licenses].allow`. If a new
dependency introduces a license not on the list, the check fails. To fix:

1. Verify the license is compatible with MIT / Apache-2.0.
2. Add it to the `allow` list in `deny.toml` with a PR comment justifying it.

## Handling a ban hit

`cargo deny check bans` warns on duplicate crate versions and denies wildcard
version requirements. If a duplicate is intentional and unavoidable:

```toml
[bans]
skip = [
    # <crate> is pulled in by <parent-a> (vX) and <parent-b> (vY);
    # both versions are API-compatible and safe to coexist.
    { name = "<crate>", version = "=X.Y.Z" },
]
```

If you hit a wildcard denial, pin the dependency to a concrete version in
your `Cargo.toml`.

## CI integration

The `audit.yml` workflow runs on every push/PR to `master` and on a daily
cron schedule (`0 6 * * *` UTC). Both jobs must pass before a PR can be
merged.

| Job | Tool | Scope |
|-----|------|-------|
| `audit` | cargo-audit via `rustsec/audit-check@v2` | RUSTSEC advisories |
| `deny` | cargo-deny via `EmbarkStudios/cargo-deny-action@v2` | licenses, bans, advisories, sources |
