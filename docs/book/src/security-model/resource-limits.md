# Resource limits

SupaZip enforces configurable resource limits on every archive operation. These limits prevent zip-bomb attacks, memory exhaustion, and denial-of-service through crafted archives.

## The `Limits` struct

Defined in `supazip-core/src/traits.rs`:

```rust
pub struct Limits {
    pub max_archive_size: u64,
    pub max_entry_count: usize,
    pub max_entry_size: u64,
    pub max_compression_ratio: u32,
}
```

### Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_archive_size` | `u64` | 4 GiB (4,294,967,296) | Maximum bytes the reader may produce. Operations that would exceed this return `ArchiverError::TooLarge`. |
| `max_entry_count` | `usize` | 1,000,000 | Maximum number of entries a single archive may contain. Enforced at metadata time. |
| `max_entry_size` | `u64` | 1 GiB (1,073,741,824) | Maximum size of any single extracted or listed entry. Enforced at metadata time where possible; per-byte check during streaming otherwise. |
| `max_compression_ratio` | `u32` | 100 | Reject entries where `uncompressed_size > compressed_size × ratio`. Set to 0 to disable (not recommended). |

### Defaults

`Limits::default()` returns the values above. `Limits::new()` is the `const fn` equivalent with identical values. The two are always equal:

```rust
assert_eq!(Limits::new(), Limits::default());
```

### Unrestricted mode

`Limits::is_unrestricted()` returns `true` when all size fields are set to their type's maximum. This skips the per-byte read checks:

```rust
let limits = Limits {
    max_archive_size: u64::MAX,
    max_entry_count: usize::MAX,
    max_entry_size: u64::MAX,
    max_compression_ratio: 0,
};
assert!(limits.is_unrestricted());
```

Use unrestricted mode only when the input is trusted (e.g. testing with known-good archives).

## Environment variable override

The CLI supports overriding `max_archive_size` via the `SUPAZIP_MAX_ARCHIVE_SIZE` environment variable:

```bash
export SUPAZIP_MAX_ARCHIVE_SIZE=512M
supazip list large-archive.zip
```

Accepted formats:
- Decimal bytes: `1073741824`
- With suffix: `512K`, `512M`, `2G` (case-insensitive)

The value is parsed by `parse_size()` in `supazip-cli/src/main.rs`. Unparseable values emit a warning and use the default.

Other limits (`max_entry_count`, `max_entry_size`, `max_compression_ratio`) are not currently configurable via environment variables or CLI flags. Exposing them as flags is a planned addition.

## Compression ratio guard

The `max_compression_ratio` field is the primary defence against zip bombs. Classic zip bombs work by compressing highly repetitive data:

```
Compressed:   1 MB
Uncompressed: 10 GB
Ratio:        10,000×
```

With the default limit of 100×, this entry would be rejected:

```
error: Archive exceeds resource limit: compression ratio 10000 exceeds limit 100
```

### How it works

During extraction, each entry's uncompressed size (reported by the archive header) is compared against its compressed size:

```
if uncompressed_size > compressed_size × max_compression_ratio {
    return Err(ArchiverError::TooLarge(...));
}
```

The check happens before any bytes are decompressed, so no memory or disk is consumed.

### Disabling the check

Set `max_compression_ratio` to 0:

```rust
let mut limits = Limits::default();
limits.max_compression_ratio = 0;
```

This is **not recommended** for production use. It is intended for benchmarking and testing.

## How limits are enforced

| Backend | `max_archive_size` | `max_entry_count` | `max_entry_size` | `max_compression_ratio` |
|---------|-------------------|-------------------|-------------------|------------------------|
| ZIP | Checked during streaming | Checked at list time | Checked at metadata | Checked per entry |
| 7z | Checked during streaming | Checked at list time | Checked at metadata | Checked per entry |
| TAR | Checked during streaming | Checked at list time | Checked during streaming | Checked per entry |
| TAR.GZ | Checked during streaming | Checked at list time | Checked during streaming | Checked per entry |
| TAR.XZ | Checked during streaming | Checked at list time | Checked during streaming | Checked per entry |

Backends that cannot determine an entry's uncompressed size up front (e.g. TAR) enforce `max_entry_size` during streaming by tracking the number of bytes written and aborting if the limit is exceeded.

## GUI limits

The GUI uses `Limits::default()` unless overridden by the `AppController::with_limits()` constructor. The GUI does not currently expose a settings UI for limits; the values are hardcoded. A future settings window (milestone 0.5.0) will allow the user to configure limits via the GUI.
