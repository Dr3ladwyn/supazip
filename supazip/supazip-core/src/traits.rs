use crate::error::ArchiverError;
use chrono::{DateTime, Utc};
use std::io::{Read, Seek, Write};
use std::sync::mpsc::Sender;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

// design note:
// The `ArchiveFormat` trait takes `Box<dyn Read>` and `Box<dyn WriteSeek>` for
// its reader and writer parameters rather than the `<R: Read, W: Write + Seek>`
// generics that `plan.md` shows. The trade-off:
//
//   * The dyn form is object-safe, which is what the `BACKENDS: LazyLock<
//     HashMap<&str, &'static dyn ArchiveFormat>>` registry in
//     `formats/mod.rs` needs. Lookup, storage, and dispatch through a single
//     `&'static dyn ArchiveFormat` are then trivial.
//   * The generic form would monomorphise one copy of every method per
//     `(R, W)` pair, which can be faster and avoids the vtable call, but
//     breaks the registry and forces every caller to thread the concrete
//     types through. The CLI and GUI front-ends do not benefit from the
//     monomorphisation, so the dyn form wins on simplicity.
//
// `WriteSeek` is a local supertrait of `Write + Seek` (defined below) so the
// trait method can name it without colliding with std types.
//
// To switch to the generic form later: change the method signatures to
// `<R: Read, W: Write + Seek>`, change `BACKENDS` to use a static dispatch
// helper, and add `Send` bounds where the registry stores them. The rest of
// the codebase only depends on the trait through the `&dyn ArchiveFormat`
// view, so the migration is local.

// =============================================================================
// NoOpProgress - no-op implementation for CLI when progress is not needed
// =============================================================================

pub struct NoOpProgress;

impl ProgressCallback for NoOpProgress {
    fn set_progress(&self, _current: u64, _total: u64) {}
    fn set_message(&self, _message: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
}

// =============================================================================
// ProgressState - shared state for progress tracking across threads
// =============================================================================

pub struct ProgressState {
    pub current: AtomicU64,
    pub total: AtomicU64,
    pub message: std::sync::Mutex<String>,
    pub cancelled: AtomicBool,
}

impl ProgressState {
    pub fn new() -> Self {
        Self {
            current: AtomicU64::new(0),
            total: AtomicU64::new(0),
            message: std::sync::Mutex::new(String::new()),
            cancelled: AtomicBool::new(false),
        }
    }

    pub fn set_progress(&self, current: u64, total: u64) {
        self.current.store(current, Ordering::SeqCst);
        self.total.store(total, Ordering::SeqCst);
    }

    pub fn set_message(&self, msg: &str) {
        if let Ok(mut message) = self.message.lock() {
            *message = msg.to_string();
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

impl Default for ProgressState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCallback for Arc<ProgressState> {
    fn set_progress(&self, current: u64, total: u64) {
        ProgressState::set_progress(self, current, total);
    }

    fn set_message(&self, message: &str) {
        ProgressState::set_message(self, message);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

// =============================================================================
// ChannelProgress - sends progress updates through a channel to the GUI
// =============================================================================

pub struct ProgressUpdate {
    pub current: u64,
    pub total: u64,
    pub message: String,
}

pub struct ChannelProgress {
    tx: Sender<ProgressUpdate>,
}

impl ChannelProgress {
    pub fn new(tx: Sender<ProgressUpdate>) -> Self {
        Self { tx }
    }
}

impl ProgressCallback for ChannelProgress {
    fn set_progress(&self, current: u64, total: u64) {
        let _ = self.tx.send(ProgressUpdate {
            current,
            total,
            message: String::new(),
        });
    }

    fn set_message(&self, message: &str) {
        let _ = self.tx.send(ProgressUpdate {
            current: 0,
            total: 0,
            message: message.to_string(),
        });
    }

    fn is_cancelled(&self) -> bool {
        false // GUI handles cancellation differently
    }
}

pub struct ArchiveEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub compressed_size: u64,
    pub modified: Option<DateTime<Utc>>,
    pub compression_method: String,
    pub crc32: Option<u32>,
    pub encrypted: bool,
}

impl std::fmt::Debug for ArchiveEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArchiveEntry")
            .field("name", &self.name)
            .field("path", &self.path)
            .field("is_dir", &self.is_dir)
            .field("size", &self.size)
            .field("compressed_size", &self.compressed_size)
            .field("modified", &self.modified)
            .field("compression_method", &self.compression_method)
            .field("crc32", &self.crc32)
            .field("encrypted", &self.encrypted)
            .finish()
    }
}

pub struct CreateOptions {
    pub compression_method: String,
    pub compression_level: Option<u32>,
}

/// Resource limits applied to archive operations. Used to refuse zip-bomb-class
/// inputs and to keep memory usage bounded.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Maximum number of bytes the archive reader is allowed to produce. A
    /// `list`, `test`, or `extract` call that would consume more than this
    /// returns `ArchiverError::TooLarge` instead of buffering the whole input.
    pub max_archive_size: u64,
    /// Maximum number of entries a single archive may contain.
    pub max_entry_count: usize,
    /// Maximum size of any single extracted or listed entry, in bytes. Enforced
    /// at metadata time where possible; if the backend cannot determine size up
    /// front, the per-byte progress check enforces it during streaming.
    pub max_entry_size: u64,
}

impl Default for Limits {
    fn default() -> Self {
        // Defaults chosen for a desktop user opening normal archives on a
        // machine with comfortable RAM. CLI / GUI can override per call.
        Self {
            max_archive_size: 4 * 1024 * 1024 * 1024, // 4 GiB
            max_entry_count: 1_000_000,
            max_entry_size: 1 * 1024 * 1024 * 1024, // 1 GiB per entry
        }
    }
}

impl Limits {
    /// `true` when the limits describe an unrestricted run. Used by the backends
    /// to skip the per-byte read checks when no limit is in force.
    pub fn is_unrestricted(&self) -> bool {
        self.max_archive_size == u64::MAX
            && self.max_entry_count == usize::MAX
            && self.max_entry_size == u64::MAX
    }
}

pub trait ProgressCallback: Send {
    fn set_progress(&self, current: u64, total: u64);
    fn set_message(&self, message: &str);
    fn is_cancelled(&self) -> bool;
}

/// Trait combining Write and Seek for object-safe trait objects
pub trait WriteSeek: Write + Seek {}
impl<T: Write + Seek> WriteSeek for T {}

pub trait ArchiveFormat: Send + Sync {
    fn name(&self) -> &'static str;
    fn extensions(&self) -> &[&str];

    fn list(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        limits: &Limits,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract(
        &self,
        reader: Box<dyn Read>,
        dest_dir: &std::path::Path,
        entries: &[&str],
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError>;

    fn create(
        &self,
        writer: Box<dyn WriteSeek>,
        entries: &[std::path::PathBuf],
        options: &CreateOptions,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError>;

    fn test(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<bool, ArchiverError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_progress_methods_are_total() {
        let p = NoOpProgress;
        p.set_progress(0, 0);
        p.set_message("");
        assert!(!p.is_cancelled());
    }

    #[test]
    fn progress_state_round_trip() {
        let s = ProgressState::new();
        s.set_progress(42, 100);
        s.set_message("hello");
        assert!(!s.is_cancelled());
        s.cancel();
        assert!(s.is_cancelled());
    }

    #[test]
    fn progress_state_default_matches_new() {
        let s = ProgressState::default();
        assert!(!s.is_cancelled());
    }

    #[test]
    fn arc_progress_state_dispatches_to_inner() {
        let s: Arc<ProgressState> = Arc::new(ProgressState::new());
        let cb: &dyn ProgressCallback = &s;
        cb.set_progress(7, 9);
        cb.set_message("msg");
        assert!(!cb.is_cancelled());
        s.cancel();
        assert!(cb.is_cancelled());
    }

    #[test]
    fn channel_progress_emits_structured_updates() {
        let (tx, rx) = std::sync::mpsc::channel();
        let p = ChannelProgress::new(tx);
        p.set_progress(1, 2);
        p.set_message("entry");
        let u1 = rx.recv().expect("recv 1");
        let u2 = rx.recv().expect("recv 2");
        assert_eq!(u1.current, 1);
        assert_eq!(u1.total, 2);
        assert_eq!(u2.message, "entry");
    }

    #[test]
    fn channel_progress_is_not_cancellable() {
        // `ChannelProgress::is_cancelled` is hard-coded `false`; the GUI drives
        // cancellation through the `ProgressState` path. Document the behaviour
        // here so a future refactor does not silently break the contract.
        let (tx, _rx) = std::sync::mpsc::channel();
        let p = ChannelProgress::new(tx);
        assert!(!p.is_cancelled());
    }

    #[test]
    fn write_seek_blanket_covers_files() {
        // Compile-time check: any `Write + Seek` type implements `WriteSeek`.
        fn _accepts_write_seek<W: WriteSeek>(_: &mut W) {}
        let mut c = std::io::Cursor::new(Vec::<u8>::new());
        _accepts_write_seek(&mut c);
    }

    #[test]
    fn limits_default_is_sane() {
        let l = Limits::default();
        assert!(l.max_archive_size >= 1024 * 1024);
        assert!(l.max_entry_count >= 1000);
        assert!(l.max_entry_size >= 1024 * 1024);
        assert!(!l.is_unrestricted());
    }

    #[test]
    fn limits_unrestricted_helper() {
        let l = Limits {
            max_archive_size: u64::MAX,
            max_entry_count: usize::MAX,
            max_entry_size: u64::MAX,
        };
        assert!(l.is_unrestricted());
    }
}
