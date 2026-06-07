# Security Policy

## Supported versions

| Version   | Supported          |
|-----------|--------------------|
| >= 1.0.0  | Yes                |
| 0.5.0     | Critical fixes only |
| < 0.5.0   | No                 |

Security patches are applied to the latest stable release. Older versions
receive critical fixes on a best-effort basis when a backport is
low-risk and the reporter provides a reproduction case.

## Reporting a vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, report vulnerabilities via email:

- **Email:** security@example.com (replace with your real address before publishing)
- **PGP key:** Available at https://keys.example.com/security@example.com (optional -- remove this line if no PGP key is configured)

Please include:

1. A description of the vulnerability and its potential impact.
2. Steps to reproduce, or a proof-of-concept.
3. The version(s) affected (output of `supazip --version`).
4. Any suggested fix, if you have one.

You should receive an acknowledgement within **72 hours**. We will work
with you to understand the issue and coordinate a fix before any public
disclosure.

## Disclosure policy

SupaZip follows a **90-day coordinated disclosure** window:

1. **Day 0** -- Vulnerability reported privately.
2. **Day 1-3** -- Acknowledgement sent to the reporter.
3. **Day 4-30** -- Investigation and fix development. The reporter is
   kept informed of progress.
4. **Day 31-60** -- Fix reviewed, tested, and merged. A security
   release is prepared.
5. **Day 61-90** -- Security release published. Reporter is credited
   (unless they prefer anonymity). CVE is requested if applicable.
6. **Day 90** -- Full disclosure on the project blog and Rust forum,
   regardless of whether a fix has been released.

If the reporter and maintainer agree, the timeline can be shortened.
Extensions beyond 90 days require mutual written agreement.

## Scope

The following components are **in scope** for security reports:

### Crates

- `supazip-core` -- archive engine (list, extract, create, test).
- `supazip-cli` -- command-line front-end.
- `supazip-gui` -- desktop GUI front-end.

### Archive formats

- ZIP (read + write, AES-256 AE-2 encryption).
- 7z (read + write, AES-256 + LZMA2 encryption).
- TAR, TAR.GZ, TAR.XZ (read + write).

### Infrastructure

- Packaging scripts under `scripts/` and `packaging/`.
- CI workflows under `.github/workflows/`.
- Release pipeline (signed checksums, SBOM generation).
- Nix flake, Dockerfile, and package manager manifests.

### What is in scope (vulnerability classes)

- **Path-traversal** attacks via crafted archive entry names
  (zip-slip, 7z-slip).
- **Zip-bomb / decompression bomb** attacks that bypass resource limits.
- **Memory exhaustion** through oversized entries or excessive entry counts.
- **Encryption weaknesses** in AES-256 key derivation or password handling.
- **Race conditions** in atomic write (tempfile + rename) logic.
- **Cancellation safety** issues where Ctrl-C leaves partial or corrupt output.
- **Denial of service** through malformed archive headers or infinite loops.
- **Arbitrary file overwrite** through symlink or hardlink entries.
- **Information leaks** through error messages, logs, or temp files.

## Out of scope

The following are **not** in scope:

- **Third-party dependencies.** Vulnerabilities in upstream crates
  (`zip`, `sevenz-rust`, `tar`, `flate2`, `lzma-rs`, `egui`, `eframe`,
  `clap`, `tokio`, etc.) should be reported directly to those projects.
  We will update our dependencies promptly once upstream fixes are
  available.
- **Social engineering** attacks (phishing, impersonation, etc.).
- **Physical access** attacks on the build or release infrastructure.
- **Availability** of the GitHub repository or package manager registries.
- **Denial of service via extremely large archives** beyond the
  configurable `Limits::max_archive_size` (default: 4 GiB). Users are
  expected to configure appropriate limits for their environment.
- **GUI-specific issues** in the egui/eframe framework (e.g., rendering
  bugs, input handling quirks). Report these upstream.

## Security-related design decisions

SupaZip's security posture is built on several deliberate design
decisions. Understanding these helps assess the risk of a reported
vulnerability.

### Resource limits

Every backend respects `Limits` to prevent resource exhaustion:

| Limit                  | Default   | Behaviour                                      |
|------------------------|-----------|-------------------------------------------------|
| `max_archive_size`     | 4 GiB     | `Read::take()`-bounded reader aborts at limit.  |
| `max_entry_count`      | 1,000,000 | Enumeration aborts if exceeded.                 |
| `max_entry_size`       | 4 GiB     | Single-entry write aborts if exceeded.           |
| `max_compression_ratio` | 100:1    | Decompression aborts if ratio exceeded.          |

The CLI honours `SUPAZIP_MAX_ARCHIVE_SIZE` (decimal bytes, with optional
`K`/`M`/`G` suffix).

### Path-traversal protection

- **ZIP:** The `zip` crate's `enclosed_name()` rejects absolute paths,
  `..` components, and entries with no name.
- **7z:** `SevenZBackend::safe_join` performs the same checks
  independently, rejecting empty names, absolute paths, and `..`
  components.
- **TAR/TAR.GZ/TAR.XZ:** Entry names are sanitised through the same
  `safe_join` path.
- Both guards are covered by property-based tests (proptest) that
  verify no entry name can escape the destination directory.

### Atomic writes

The CLI creates archives through `tempfile::NamedTempFile` in the same
directory as the target, then calls `persist()` (rename) to atomically
replace the target. If the process is interrupted (Ctrl-C, kill, power
loss), the temp file is cleaned up and the previous good archive is
preserved.

### Cancellation

Backends call `progress.is_cancelled()` between entries. The CLI maps
`ArchiverError::Cancelled` to exit code 130, matching the POSIX
convention for SIGINT. No partial output is written on cancellation
(atomic write handles this).

### Encryption

- **ZIP:** AES-256 via AE-2 vendor version (the same flavour 7-Zip
  produces). Password is zeroed from memory after use.
- **7z:** AES-256 + LZMA2 via `sevenz-rust::AesEncoderOptions`.

### Error handling

`ArchiverError` variants carry the original error through `#[source]`,
enabling proper error chain walking. Error messages are user-facing and
do not leak internal paths, stack traces, or memory addresses.

### Reproducible builds

Release builds use `SOURCE_DATE_EPOCH` and `--remap-path-prefix` to
produce deterministic binaries. The release pipeline verifies
reproducibility by rebuilding from source and comparing hashes.

## Acknowledgements

We thank the following security researchers for responsibly disclosing
vulnerabilities:

<!-- Add researchers here as reports are received and fixed. -->
- _No reports yet. Be the first!_

## Contact

For any questions about this security policy, contact:

- **Email:** security@example.com
- **GitHub:** https://github.com/your-org/supazip (for non-security issues)
