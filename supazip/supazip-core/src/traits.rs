use crate::error::ArchiverError;
use chrono::{DateTime, Utc};
use serde::Serialize;
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

/// A [`ProgressCallback`] that ignores every progress event and never reports
/// cancellation. Use it in tests and in code paths that do not need to drive a
/// UI or surface cancellation.
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

/// Thread-safe shared progress handle. Cloneable via `Arc<ProgressState>`; the
/// GUI uses it to mirror progress into the UI thread, and the CLI uses it
/// through `Arc<ProgressState>` as a `ProgressCallback` implementation.
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

/// A single progress event. `current` and `total` are populated by
/// `set_progress`; `message` is populated by `set_message`. The GUI translates
/// this into egui progress bars and status line text.
pub struct ProgressUpdate {
    pub current: u64,
    pub total: u64,
    pub message: String,
}

/// Sends each progress event through an mpsc channel. The GUI receiver reads
/// the channel from the UI thread and repaints accordingly. Cancellation
/// through this callback is intentionally unsupported; the GUI flips
/// `ProgressState::cancel` instead.
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

/// Metadata for a single entry inside an archive. Returned by
/// [`ArchiveFormat::list`] and accepted by the GUI's file-list view.
#[derive(Debug, Clone, Serialize)]
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

/// Compression method requested for a `create` call.
///
/// Codec support matrix:
///
/// - [`CompressionMethod::Deflate`] — the classic ZIP default; supported
///   everywhere.
/// - [`CompressionMethod::Store`] — no compression; bytes are written
///   verbatim. Zero-cost: `compressed_size == size` for every entry.
/// - [`CompressionMethod::Zstd`] — Zstandard, fast dictionary-friendly
///   codec. **ZIP-only** — written through the `zip` crate's `zstd`
///   feature; ignored by 7z (7z uses LZMA2 internally).
/// - [`CompressionMethod::Brotli`] — Brotli, high-ratio text codec.
///   **ZIP-only** — the `zip` crate does not expose a Brotli codec, so
///   SupaZip stores Brotli-compressed bytes as a `Stored` entry with a
///   `.br` filename suffix and decompresses transparently on extract.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CompressionMethod {
    /// DEFLATE — the classic ZIP default. Also the `Default` variant;
    /// preserves bit-for-bit behaviour with pre-WS-B SupaZip archives (and
    /// is what every other PeaZip-style tool picks).
    #[default]
    Deflate,
    /// Brotli — high-ratio text compression. ZIP-only, custom on-disk layout
    /// (Stored entry with `.br` suffix, decompressed transparently).
    Brotli,
    /// Zstandard — fast dictionary-friendly codec. ZIP-only.
    Zstd,
    /// Store — no compression; the entry bytes are written verbatim.
    Store,
}

impl CompressionMethod {
    /// Stable string key for this codec, matching the historical
    /// `compression_method: String` values the CLI / GUI used to write.
    pub fn as_str(self) -> &'static str {
        match self {
            CompressionMethod::Deflate => "deflate",
            CompressionMethod::Brotli => "brotli",
            CompressionMethod::Zstd => "zstd",
            CompressionMethod::Store => "store",
        }
    }

    /// Map a free-form legacy key (whatever a CLI/GUI used to pass in
    /// `compression_method: String`) onto a typed enum. Unknown values
    /// collapse to `Deflate` so the engine never refuses to start a
    /// create call just because of a typo in the legacy field.
    pub fn from_legacy_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "deflate" | "deflated" => Self::Deflate,
            "store" | "none" | "stored" => Self::Store,
            "brotli" | "br" => Self::Brotli,
            "zstd" | "zstandard" => Self::Zstd,
            // "bzip2" and anything unrecognised fall back to Deflate; the
            // ZIP crate does not enable a bzip2 feature in this workspace.
            _ => Self::Deflate,
        }
    }
}

/// Tunables for a `create` call. `compression_method` is a free-form string
/// keyed on the backend (`"deflate"`, `"store"`, `"bzip2"`, `"zstd"` for ZIP;
/// ignored by 7z, which uses LZMA2 / AES-256 depending on password).
/// `compression_level` is the 0..=9 level some backends expose.
///
/// The newer typed [`Self::compression`] field supersedes the legacy
/// `compression_method` string. Backends that have been migrated read the
/// typed field; the legacy string is preserved so that older call sites
/// (and the GUI) keep working without any code change. New code should
/// populate both fields to stay forward-compatible.
pub struct CreateOptions {
    /// Legacy free-form codec key. Kept for backward compatibility with
    /// every caller that has not migrated to the typed enum yet.
    pub compression_method: String,
    /// 0..=9 level for backends that expose one. `None` means "engine
    /// default".
    pub compression_level: Option<u32>,
    /// Typed compression codec. When set to anything other than the
    /// default ([`CompressionMethod::Deflate`]) the ZIP backend uses it
    /// over the legacy string. Defaults to `Deflate`.
    pub compression: CompressionMethod,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            compression_method: CompressionMethod::default().as_str().to_string(),
            compression_level: None,
            compression: CompressionMethod::default(),
        }
    }
}

/// Resource limits applied to archive operations. Used to refuse zip-bomb-class
/// inputs and to keep memory usage bounded.
///
/// # Defaults
///
/// [`Limits::default`] is 4 GiB archive, 1M entries, 1 GiB per entry, and a
/// 100× compression ratio cap. CLI / GUI callers can pick a tighter policy
/// for the duration of one command, or use [`Limits::is_unrestricted`] to
/// skip the per-byte checks entirely when they know the input is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// Reject entry where `compressed_size * ratio < uncompressed_size` to
    /// defeat zip-bomb / quine-like archives. Set to 0 to disable the check
    /// (NOT recommended).
    pub max_compression_ratio: u32,
}

impl Default for Limits {
    fn default() -> Self {
        // Defaults chosen for a desktop user opening normal archives on a
        // machine with comfortable RAM. CLI / GUI can override per call.
        Self {
            max_archive_size: 4 * 1024 * 1024 * 1024, // 4 GiB
            max_entry_count: 1_000_000,
            max_entry_size: 1024 * 1024 * 1024, // 1 GiB per entry
            max_compression_ratio: 100,         // 100×
        }
    }
}

impl Limits {
    /// `pub const fn new` — all-defaults. Mirrors [`Limits::default`] but is
    /// callable in `const` contexts (default impls are not `const fn`).
    pub const fn new() -> Self {
        Self {
            max_archive_size: 4 * 1024 * 1024 * 1024, // 4 GiB
            max_entry_count: 1_000_000,
            max_entry_size: 1024 * 1024 * 1024, // 1 GiB per entry
            max_compression_ratio: 100,         // 100×
        }
    }

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

/// Trait combining Write and Seek for object-safe trait objects. Implemented
/// for every `T: Write + Seek` via the blanket impl below; the local name
/// exists so the trait method can name a single supertrait without colliding
/// with `std::io::Write` / `std::io::Seek`.
pub trait WriteSeek: Write + Seek {}
impl<T: Write + Seek> WriteSeek for T {}

/// Engine surface for one archive format. Every method takes the resources it
/// needs (a reader / writer / progress callback) and never reaches outside
/// its arguments. The backends live in [`crate::formats`]; the trait is the
/// contract.
///
/// # Errors
///
/// All methods return [`crate::error::ArchiverError`]. The `extract` and
/// `list` paths can also return `ArchiverError::PasswordRequired` /
/// `ArchiverError::WrongPassword` for encrypted entries whose password is
/// missing or incorrect.
///
/// # Resource limits
///
/// The `limits: &Limits` parameter is the per-call ceiling. Backends refuse to
/// read past `limits.max_archive_size`, refuse to enumerate more than
/// `limits.max_entry_count` entries, and (where the format reports an entry's
/// uncompressed size up front) refuse to write past `limits.max_entry_size`.
///
/// # Progress and cancellation
///
/// Long-running methods take a `progress: &dyn ProgressCallback`. Backends
/// call `is_cancelled` between entries; returning `true` makes the backend
/// short-circuit with `ArchiverError::Cancelled`. The CLI ships
/// [`NoOpProgress`] and the GUI uses [`ChannelProgress`] or
/// [`ProgressState`].
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
            max_compression_ratio: 0,
        };
        assert!(l.is_unrestricted());
    }

    #[test]
    fn default_limits_has_compression_ratio() {
        // Documenting test: the default `Limits` must set a non-zero
        // `max_compression_ratio` so zip-bomb defence is on out of the box.
        assert_eq!(Limits::default().max_compression_ratio, 100);
    }

    #[test]
    fn limits_new_matches_default() {
        // `Limits::new` is the const-friendly counterpart of `Limits::default`;
        // they must produce the same value so callers can pick either.
        assert_eq!(Limits::new(), Limits::default());
    }

    #[test]
    fn limits_max_compression_ratio_zero_disables() {
        // Documenting test (no runtime enforcement here — that lives in the
        // 7z backend's extract loop). Setting the ratio to 0 is the documented
        // way to disable the check.
        let mut l = Limits::default();
        l.max_compression_ratio = 0;
        assert_eq!(l.max_compression_ratio, 0);
    }

    #[test]
    fn compression_method_from_legacy_str() {
        // Deflate aliases & case insensitivity
        assert_eq!(
            CompressionMethod::from_legacy_str("deflate"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("deflated"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("DEFLATE"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("Deflated"),
            CompressionMethod::Deflate
        );

        // Store aliases & case insensitivity
        assert_eq!(
            CompressionMethod::from_legacy_str("store"),
            CompressionMethod::Store
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("none"),
            CompressionMethod::Store
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("stored"),
            CompressionMethod::Store
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("STORE"),
            CompressionMethod::Store
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("NONE"),
            CompressionMethod::Store
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("Stored"),
            CompressionMethod::Store
        );

        // Brotli aliases & case insensitivity
        assert_eq!(
            CompressionMethod::from_legacy_str("brotli"),
            CompressionMethod::Brotli
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("br"),
            CompressionMethod::Brotli
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("BROTLI"),
            CompressionMethod::Brotli
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("Br"),
            CompressionMethod::Brotli
        );

        // Zstd aliases & case insensitivity
        assert_eq!(
            CompressionMethod::from_legacy_str("zstd"),
            CompressionMethod::Zstd
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("zstandard"),
            CompressionMethod::Zstd
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("ZSTD"),
            CompressionMethod::Zstd
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("ZStandard"),
            CompressionMethod::Zstd
        );

        // Fallbacks for unknown or unsupported inputs
        assert_eq!(
            CompressionMethod::from_legacy_str("bzip2"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("lzma"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str("unknown"),
            CompressionMethod::Deflate
        );
        assert_eq!(
            CompressionMethod::from_legacy_str(""),
            CompressionMethod::Deflate
        );
    }
}
