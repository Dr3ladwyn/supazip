//! Shared helpers for the WS-G property-based round-trip tests.
//!
//! The five backend-specific files (`round_trip_*.rs`) all need the same
//! thing: a way to ask proptest for a random list of `(filename, bytes)`
//! pairs and a way to materialise those pairs on disk under a fresh
//! `tempfile::TempDir`. This module centralises both so a tweak to the
//! generation strategy only needs to land in one place.
//!
//! ## Strategy
//!
//! Filenames are *flat* (no path separators and no `.` traversal tokens).
//! The reason is asymmetry across the backends:
//!
//! * `ZipBackend::create` stores the entry name as
//!   `entry_path.display()` with backslashes swapped to forward slashes, so
//!   it preserves subdirectory structure.
//! * `SevenZBackend::create` stores `entry_name =
//!   path.file_name().unwrap_or("unknown")` — the basename only.
//! * `TarBackend::create` / `TarGzBackend::create` / `TarXzBackend::create`
//!   all use `path.file_name()` for the same reason.
//!
//! Generating names with embedded `/` would therefore collide in the
//! tar-family archives (two distinct proptest entries could collapse to the
//! same `file_name`), and the bytes-equal invariant would become
//! ill-defined. Flat names keep all five backends' invariants identical
//! and avoid the collision class entirely.
//!
//! The character set is the conservative subset `[a-zA-Z0-9_.-]` from the
//! strategy suggested in the plan, minus the `/` we are dropping.

use proptest::prelude::*;
use std::path::PathBuf;

/// A pair of `(archive entry name, payload bytes)`.
///
/// We keep the same name for the file on disk and the archive entry — the
/// tar backends use the on-disk `file_name()` to derive the archive entry
/// name, so the two must be identical.
pub type EntryInput = (String, Vec<u8>);

/// proptest `PROPTEST_CASES` env override (matches the convention used by
/// the milestone plan). When unset, falls back to the per-backend count.
pub fn cases(default: u32) -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Strategy: a small, fast list of `(flat_name, bytes)` pairs.
///
/// Defaults (per the milestone plan):
/// - 0..=8 entries (most generated cases have 0–8 files; the lower bound
///   catches the empty-archive case).
/// - Names: 1..=40 chars from `[a-zA-Z0-9_.-]` plus a 2-char hex
///   collision-avoidance prefix. Any two consecutive `.` characters are
///   normalised to `--` so the strategy does not produce a name that
///   `formats::safe_join` over-rejects (the helper treats *any* `..`
///   substring as a path-traversal attempt, which is too strict for
///   ordinary filenames like `foo..bar`).
/// - Bytes: 0..=4 KiB per entry.
///
/// We return a `BoxedStrategy` so callers in sibling modules get a
/// `Sized` value type. The opaque `impl Strategy<Value = …>` form trips
/// proptest's macro expansion on a `let`-binding size check; `BoxedStrategy`
/// hides the value type behind a `Box` and the macro is happy.
pub fn arb_entries() -> proptest::strategy::BoxedStrategy<Vec<EntryInput>> {
    proptest::collection::vec(
        (
            // Flat filename: no `/`, just safe characters. `..` as a
            // substring is fine in principle but `safe_join` would
            // reject it; the post-process step below rewrites
            // consecutive dots to dashes so the generated name passes
            // the existing safety check.
            "[a-zA-Z0-9_.-]{1,40}",
            proptest::collection::vec(any::<u8>(), 0..4096),
        ),
        0..8,
    )
    .prop_map(|entries| {
        entries
            .into_iter()
            .enumerate()
            .map(|(i, (name, content))| {
                // Prefix with the 2-char hex index so the tar backends
                // (which collapse on `file_name()`) cannot dedupe two
                // proptest entries that happen to share a basename.
                let mut prefixed = format!("{i:02x}_{name}");
                // The flat-naming strategy can produce `..` inside the
                // body (e.g. `foo..bar`). The existing `safe_join`
                // helper treats *any* `..` substring as a traversal
                // attempt; that is over-strict for an embedded double
                // dot. Normalise the substring to `--` so the round
                // trip succeeds without touching the backend.
                while let Some(pos) = prefixed.find("..") {
                    prefixed.replace_range(pos..pos + 2, "--");
                }
                (prefixed, content)
            })
            .collect()
    })
    .boxed()
}

/// Materialise a list of `(name, content)` pairs on disk and chdir into
/// the tempdir for the duration of a guard object. Returns:
///
/// * `_guard` — an RAII handle that holds the tempdir alive *and*
///   restores the previous working directory on drop. Hold this for the
///   entire test.
/// * `paths` — relative `PathBuf`s; each one is just the entry name. Use
///   these when calling the backends so the archive entry name and the
///   on-disk file name match exactly. The relative layout means the
///   `ZipBackend` stores the basename (not a host absolute path) in the
///   archive, which is what the existing unit tests rely on.
///
/// ## Concurrency
///
/// `chdir` is process-wide. The five round-trip proptest functions run
/// in parallel by default under `cargo test`, so the chdir would clobber
/// itself across threads. We hold a process-wide mutex around the
/// critical section (write source files + drive the backend + read back)
/// — see the test bodies for the lock acquisition pattern.
pub fn materialize_entries(entries: &[EntryInput]) -> (CwdGuard, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("create tempdir for proptest");
    let guard = CwdGuard::new(dir).expect("chdir to tempdir");
    let mut paths = Vec::with_capacity(entries.len());
    for (name, content) in entries {
        // Defensive: refuse any generated name that would try to escape
        // the tempdir. The flat-naming strategy cannot produce a name
        // with `/` in it, so the only way to escape the tempdir is the
        // literal name `..` (which, joined onto a `Path`, refers to the
        // parent). Any other ".." substring — e.g. `foo..bar` — is a
        // perfectly valid filename and is left to the backends'
        // `safe_join` to handle.
        if name.is_empty() || name == ".." {
            panic!("unsafe generated name: {name:?}");
        }
        let p = std::path::PathBuf::from(name);
        if let Some(parent) = p.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).expect("mkdir parent");
            }
        }
        std::fs::write(&p, content).expect("write proptest entry");
        paths.push(p);
    }
    (guard, paths)
}

/// Process-wide mutex that serialises the chdir in
/// [`materialize_entries`]. The five proptest test functions each chdir
/// into their own tempdir; without this lock, two threads chdiring
/// simultaneously would leave one of them looking at the wrong tree.
pub fn chdir_lock() -> &'static std::sync::Mutex<()> {
    use std::sync::OnceLock;
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// RAII guard that owns a `tempfile::TempDir` (so it is not deleted
/// while the test runs) and restores the previous working directory on
/// drop. Construction chdirs into the tempdir; destruction chdirs back.
/// The lock guard is held for the lifetime of the test so no other
/// proptest function can clobber cwd while we are using it.
pub struct CwdGuard {
    _dir: tempfile::TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
    previous: std::path::PathBuf,
}

impl CwdGuard {
    fn new(dir: tempfile::TempDir) -> std::io::Result<Self> {
        let lock = chdir_lock().lock().unwrap_or_else(|e| e.into_inner());
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(dir.path())?;
        Ok(Self {
            _dir: dir,
            _lock: lock,
            previous,
        })
    }
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        // Best-effort restore. We can't return an error from `drop`, so
        // silently ignore failures — the test process is about to exit
        // anyway, and a stuck chdir would only affect the next test in
        // the same process, which the lock is also holding so it can
        // retry the chdir.
        let _ = std::env::set_current_dir(&self.previous);
    }
}
