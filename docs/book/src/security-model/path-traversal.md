# Path traversal

Path traversal (also known as "zip-slip") is an attack where a crafted archive entry contains `../` components that cause the extracted file to write outside the intended destination directory. SupaZip defends against this with the `safe_join` function and comprehensive tests.

## The attack

A malicious archive might contain entries like:

```
../../../etc/cron.d/backdoor
../../.bashrc
```

When extracted to `/tmp/output/`, naive code would write to:

```
/tmp/output/../../../etc/cron.d/backdoor  →  /etc/cron.d/backdoor
/tmp/output/../../.bashrc                  →  /home/user/.bashrc
```

This allows an attacker to overwrite arbitrary files on the system.

## The defence: `safe_join`

Every extract path in every backend calls `safe_join(base, entry)` before writing to disk. The function is defined in `supazip-core/src/formats/mod.rs`:

```rust
pub(crate) fn safe_join(base: &Path, entry: &str) -> Result<PathBuf, ArchiverError> {
    // 1. Reject empty entry names
    if entry.is_empty() {
        return Err(ArchiverError::invalid("empty entry name"));
    }

    // 2. Reject any path component that is ".."
    let p = Path::new(entry);
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ArchiverError::invalid(format!(
            "unsafe path: path traversal in {entry:?}"
        )));
    }

    // 3. Join and verify the result is still under base
    let p = base.join(entry);
    if !p.starts_with(base) {
        return Err(ArchiverError::invalid(format!(
            "unsafe path: escape attempt in {entry:?}"
        )));
    }

    Ok(p)
}
```

### Three-layer defence

1. **Empty name check.** An empty entry name would produce a path identical to `base` itself, which is not a valid extraction target.

2. **Component-level `..` rejection.** The function iterates over every `std::path::Component` in the entry and rejects any that are `Component::ParentDir`. This catches `../escape`, `sub/../../etc/passwd`, and similar patterns.

   Note: `..` as a *substring* of a filename (e.g. `foo..bar.txt`) is **not** rejected. Only a complete `..` path component triggers the check.

3. **Post-join prefix check.** After joining, the function verifies that the result still starts with `base`. This is a defence-in-depth check that catches any edge cases the component-level check might miss (e.g. platform-specific path parsing quirks).

### Error message

When `safe_join` rejects a path, the error message includes the word "path traversal" or "unsafe path":

```
Invalid archive: unsafe path: path traversal in "../escape"
```

This makes it easy to diagnose the issue from CLI or GUI error output.

## Test coverage

The `safe_join` function is tested in `supazip-core/src/formats/mod.rs`:

| Test | Input | Expected |
|------|-------|----------|
| `safe_join_accepts_normal_relative_path` | `"sub/file.txt"` | `Ok` |
| `safe_join_rejects_dotdot` | `"../escape"` | `Err` |
| `safe_join_rejects_dotdot_nested` | `"sub/../../etc/passwd"` | `Err` |
| `safe_join_rejects_empty` | `""` | `Err` |
| `safe_join_accepts_dotdot_in_filename` | `"foo..bar.txt"` | `Ok` |
| `safe_join_error_message_mentions_traversal` | `"../escape"` | Error contains "path traversal" or "unsafe path" |

### Property-based tests

Proptest generates random path components and asserts that `safe_join` never returns a path that escapes `base`:

```rust
proptest! {
    #[test]
    fn safe_join_never_escapes_base(
        base in arb_absolute_path(),
        entry in arb_entry_name(),
    ) {
        if let Ok(result) = safe_join(&base, &entry) {
            prop_assert!(result.starts_with(&base));
        }
    }
}
```

### Fuzz testing

The `cargo-fuzz` targets exercise the extract paths with arbitrary input, which includes random entry names. Any path traversal that bypasses `safe_join` would be caught by the fuzzer's coverage-guided exploration.

## Platform considerations

### Windows

On Windows, path separators are `\` and `/` both work. `safe_join` uses `std::path::Path::components()` which normalises both separators. An entry like `..\\escape` is correctly parsed as a `ParentDir` component.

Additionally, Windows has reserved names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`). SupaZip does not currently filter these; the `sevenz-rust` and `zip` crates handle them at the OS level.

### Unix

On Unix, `safe_join` works with forward-slash paths. Absolute paths in entries (e.g. `/etc/passwd`) are handled correctly by the prefix check: `base.join("/etc/passwd")` produces `/etc/passwd` which does not start with `base` (assuming `base` is not `/`).

## Backend integration

Every backend's `extract` method calls `safe_join` for each entry:

```rust
fn extract(...) -> Result<(), ArchiverError> {
    for entry in archive_entries {
        let dest = safe_join(dest_dir, entry.name())?;
        // Write to `dest`
    }
}
```

If `safe_join` returns an error, the entire extract operation is aborted. This is a deliberate choice: a single traversal attempt in an archive is considered a security violation, not a warning to skip.
