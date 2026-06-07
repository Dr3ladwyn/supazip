# Threat model

This chapter describes what SupaZip defends against and the mitigation strategy for each threat class.

## Assets under protection

| Asset | Sensitivity | Why it matters |
|-------|-------------|----------------|
| User's filesystem | High | Archive extraction writes files to arbitrary paths |
| User's RAM and CPU | Medium | Malicious archives can cause excessive resource consumption |
| User's credentials | High | Passwords for encrypted archives must not leak |
| Archive integrity | Medium | Corrupted or tampered archives must be detected |

## Threat classes

### 1. Path traversal (zip-slip)

**Attack:** An archive entry contains `../` components (e.g. `../../etc/passwd`) that, when extracted, write outside the intended destination directory.

**Impact:** Arbitrary file overwrite. An attacker-controlled archive can overwrite system files, SSH keys, or application binaries.

**Mitigation:** The `safe_join(base, entry)` function in `supazip-core/src/formats/mod.rs` rejects any entry that:
- Is empty.
- Contains `..` as a path component.
- Escapes `base` once joined.

Every extract path in every backend calls `safe_join`. The function is tested with property-based tests that generate random path components.

See [Path traversal](./path-traversal.md) for implementation details.

### 2. Zip bombs

**Attack:** An archive has a tiny compressed size but decompresses to gigabytes or terabytes. Without limits, extraction fills the disk or exhausts RAM.

**Impact:** Denial of service — the system runs out of disk space or memory.

**Mitigation:** The `Limits` struct enforces four ceilings:

| Limit | Default | Effect |
|-------|---------|--------|
| `max_archive_size` | 4 GiB | Maximum bytes the reader may produce |
| `max_entry_count` | 1,000,000 | Maximum number of entries |
| `max_entry_size` | 1 GiB | Maximum size of any single entry |
| `max_compression_ratio` | 100× | Reject entries where uncompressed > 100× compressed |

The compression ratio check defeats classic zip bombs where a 1 MB file decompresses to 10 GB. Setting `max_compression_ratio` to 0 disables the check (not recommended).

See [Resource limits](./resource-limits.md) for configuration details.

### 3. Password brute-force

**Attack:** An attacker tries to crack an encrypted archive's password by brute force.

**Impact:** Exposure of encrypted archive contents.

**Mitigation:** SupaZip does not implement rate limiting (that is the OS / user's responsibility). However:
- Passwords are never stored on disk by SupaZip.
- Passwords are passed as `Option<&str>` through the engine and never copied into long-lived buffers.
- The GUI password dialog uses a masked input with a show/hide toggle.
- The CLI accepts passwords via `--password` flag only (not interactive prompt), so they may appear in shell history. Users should use environment variables or pipe from a password manager.

### 4. Malformed archive data

**Attack:** A crafted archive has corrupted headers, truncated data, or impossible metadata (e.g. a 10-byte file claiming to contain 1 billion entries).

**Impact:** Crashes, infinite loops, or panics in the parser.

**Mitigation:** All backends use the `thiserror`-based `ArchiverError` enum. Every parsing error is caught and converted to `ArchiverError::InvalidArchive` with a descriptive message. The `sevenz-rust` and `zip` crates are mature libraries with their own fuzzing campaigns. SupaZip adds a second layer of fuzzing via `cargo-fuzz` targets for `list`, `extract`, and `create` on every backend.

### 5. Supply-chain attacks

**Attack:** A dependency is compromised and introduces malicious code.

**Impact:** Arbitrary code execution when processing archives.

**Mitigation:**
- `cargo-audit` runs daily in CI, checking all dependencies against the RUSTSEC advisory database.
- `cargo-deny` enforces licence allowlists and bans unknown sources.
- All runtime dependencies are pure Rust (no C bindings on Windows), reducing the attack surface.
- Git dependencies are banned by `cargo-deny` except for allow-listed exceptions.

### 6. Atomic write corruption

**Attack:** A crash or power loss during archive creation leaves a half-written file.

**Impact:** Data loss if the partial file overwrites a previous good archive.

**Mitigation:** The CLI's `create` command writes to a `NamedTempFile` in the same directory, then `persist`s (atomic rename) it over the target path. A crash mid-write never corrupts the previous archive.

## Out of scope

The following are explicitly out of scope for 1.0:

- **RAR support** — proprietary format, deferred to 1.1.
- **Network-based attacks** — SupaZip operates on local files only.
- **Sandboxing** — SupaZip runs with the user's permissions. It does not sandbox itself.
- **Encrypted archive creation in GUI** — supported in the CLI via `--password`; GUI support is planned.
- **Secure memory wiping** — Rust does not guarantee memory zeroing. Passwords may remain in deallocated memory.

## Responsible disclosure

Security vulnerabilities should be reported via the project's security policy (to be published at `SECURITY.md` for 1.0). Do not open public issues for security bugs.
