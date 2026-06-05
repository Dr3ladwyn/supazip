use std::io::{Read, Write, Seek};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::sync::mpsc::Sender;
use chrono::{DateTime, Utc};
use crate::error::ArchiverError;

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
    fn is_cancelled(&self) -> bool { false }
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
        false  // GUI handles cancellation differently
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

pub struct CreateOptions {
    pub compression_method: String,
    pub compression_level: Option<u32>,
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

    fn list(&self, reader: Box<dyn Read>, password: Option<&str>)
        -> Result<Vec<ArchiveEntry>, ArchiverError>;

    fn extract(
        &self, reader: Box<dyn Read>, dest: Box<dyn WriteSeek>, entries: &[&str],
        password: Option<&str>, progress: &dyn ProgressCallback
    ) -> Result<(), ArchiverError>;

    fn create(
        &self, writer: Box<dyn WriteSeek>, entries: &[std::path::PathBuf],
        options: &CreateOptions, password: Option<&str>,
        progress: &dyn ProgressCallback
    ) -> Result<(), ArchiverError>;

    fn test(
        &self, reader: Box<dyn Read>, password: Option<&str>,
        progress: &dyn ProgressCallback
    ) -> Result<bool, ArchiverError>;
}
