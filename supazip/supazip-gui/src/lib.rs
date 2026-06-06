//! SupaZip GUI — a PeaZip-style archive manager for 7z and ZIP.
//!
//! The binary entry point in `main.rs` runs the eframe event loop. This
//! library crate exposes the headless state machine that drives the engine
//! so it can be unit-tested without spinning up a window.
//!
//! # State model
//!
//! [`AppState`] owns the visible state (current archive, status line, busy
//! flag, cancellation). Operations live on [`AppController`], which spawns
//! worker threads that call into `supazip-core` and push
//! [`EngineEvent`]s back through an `mpsc` channel.
//!
//! Tests construct an `AppController`, drive it through a public action
//! (e.g. `list_archive`), then poll the channel for the resulting event.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use supazip_core::traits::ProgressState;
use supazip_core::{formats, ArchiveEntry, Limits};

/// One opened archive: where it lives, which backend parsed it, and the
/// entries the GUI is currently showing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenArchive {
    pub path: PathBuf,
    pub backend_name: &'static str,
    pub entries: Vec<OpenEntry>,
}

/// One row in the entry grid. Mirrors the subset of [`ArchiveEntry`] the
/// GUI cares about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenEntry {
    pub name: String,
    pub size: u64,
    pub encrypted: bool,
}

impl From<ArchiveEntry> for OpenEntry {
    fn from(e: ArchiveEntry) -> Self {
        OpenEntry {
            name: e.name,
            size: e.size,
            encrypted: e.encrypted,
        }
    }
}

/// Pure state owned by the GUI thread. `busy` and `open_archive` are the
/// only mutable bits the GUI mutates between frames; everything else is
/// plumbing for the worker thread.
#[derive(Debug)]
pub struct AppState {
    pub open_archive: Option<OpenArchive>,
    pub status: String,
    pub busy: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            open_archive: None,
            status: "Open an archive to get started.".to_string(),
            busy: false,
        }
    }
}

/// Events a worker thread can send back to the GUI thread. The GUI drains
/// these once per frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    /// List succeeded: a fresh entry list is now active.
    Listed {
        path: PathBuf,
        backend_name: &'static str,
        entries: Vec<OpenEntry>,
    },
    /// An operation finished successfully; carries a free-form status line.
    Done(String),
    /// The engine produced an error; the GUI shows it in the status bar and
    /// re-enables the toolbar.
    Error(String),
}

/// Headless controller: state + channel. The eframe `App` in `main.rs`
/// owns one of these and routes UI events into it.
pub struct AppController {
    state: AppState,
    engine_tx: Sender<EngineEvent>,
    engine_rx: Receiver<EngineEvent>,
    cancel_flag: Arc<ProgressState>,
    limits: Limits,
}

impl Default for AppController {
    fn default() -> Self {
        let (engine_tx, engine_rx) = channel();
        Self {
            state: AppState::default(),
            engine_tx,
            engine_rx,
            cancel_flag: Arc::new(ProgressState::new()),
            limits: Limits::default(),
        }
    }
}

impl AppController {
    /// Construct a controller with explicit resource limits. The GUI does
    /// this today; tests use it to exercise the limits plumbing.
    pub fn with_limits(limits: Limits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// Borrow the current state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Borrow the sender so the GUI can spawn workers that report back.
    pub fn engine_sender(&self) -> Sender<EngineEvent> {
        self.engine_tx.clone()
    }

    /// Drain all currently-queued engine events into the state. Returns
    /// the number of events consumed (useful for tests).
    pub fn drain(&mut self) -> usize {
        let mut n = 0;
        while let Ok(ev) = self.engine_rx.try_recv() {
            self.apply(ev);
            n += 1;
        }
        n
    }

    /// Apply one event to the state. Exposed for tests that want to feed
    /// events one at a time and assert between steps.
    pub fn apply(&mut self, ev: EngineEvent) {
        match ev {
            EngineEvent::Listed {
                path,
                backend_name,
                entries,
            } => {
                self.state.open_archive = Some(OpenArchive {
                    path,
                    backend_name,
                    entries,
                });
                self.state.busy = false;
                self.state.status = "loaded".to_string();
                self.cancel_flag = Arc::new(ProgressState::new());
            }
            EngineEvent::Done(msg) => {
                self.state.busy = false;
                self.state.status = msg;
                self.cancel_flag = Arc::new(ProgressState::new());
            }
            EngineEvent::Error(msg) => {
                self.state.busy = false;
                self.state.status = format!("error: {msg}");
                self.cancel_flag = Arc::new(ProgressState::new());
            }
        }
    }

    /// Mark the controller as busy with the given status line. The GUI
    /// calls this immediately before spawning a worker; tests can call it
    /// to set up the precondition for a `Done`/`Error` event.
    pub fn mark_busy(&mut self, status: impl Into<String>) {
        self.state.busy = true;
        self.state.status = status.into();
    }

    /// Cancel any in-flight engine operation. The cancellation flag is
    /// shared with the worker thread via the `Arc<ProgressState>` returned
    /// by `cancel_handle`.
    pub fn cancel(&self) {
        self.cancel_flag.cancel();
    }

    /// Borrow the shared cancellation handle for handing to a worker.
    pub fn cancel_handle(&self) -> Arc<ProgressState> {
        self.cancel_flag.clone()
    }

    /// Current resource limits. Workers pass this into the engine.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// List the entries in `archive_path` synchronously (no worker thread)
    /// and return the resulting `EngineEvent`. Useful for tests and for
    /// CLI-style callers that don't need a window.
    ///
    /// Errors are mapped to `EngineEvent::Error`. The controller's state
    /// is **not** mutated; the caller can apply the event with [`Self::apply`].
    pub fn list_archive_blocking(&self, archive_path: &Path) -> EngineEvent {
        let ext = archive_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let Some(backend) = formats::get_backend(ext) else {
            return EngineEvent::Error(format!("unsupported: {ext}"));
        };
        let file = match std::fs::File::open(archive_path) {
            Ok(f) => f,
            Err(e) => return EngineEvent::Error(format!("open: {e}")),
        };
        match backend.list(Box::new(std::io::BufReader::new(file)), None, &self.limits) {
            Ok(list) => EngineEvent::Listed {
                path: archive_path.to_path_buf(),
                backend_name: backend.name(),
                entries: list.into_iter().map(OpenEntry::from).collect(),
            },
            Err(e) => EngineEvent::Error(format!("list: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    fn build_small_zip(path: &Path) {
        let file = std::fs::File::create(path).expect("create zip");
        let mut zw = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zw.start_file("hello.txt", opts).expect("start hello");
        zw.write_all(b"hi\n").expect("write hello");
        zw.start_file("data.bin", opts).expect("start data");
        zw.write_all(&[1, 2, 3, 4]).expect("write data");
        zw.finish().expect("finish");
    }

    #[test]
    fn default_state_is_idle_with_no_archive() {
        let ctrl = AppController::default();
        assert!(!ctrl.state().busy);
        assert!(ctrl.state().open_archive.is_none());
        assert!(!ctrl.state().status.is_empty());
    }

    #[test]
    fn list_unknown_extension_yields_error_event() {
        let tmp = tempdir().expect("tempdir");
        let bogus = tmp.path().join("archive.rar");
        std::fs::write(&bogus, b"not a real rar").expect("write");

        let ctrl = AppController::default();
        let ev = ctrl.list_archive_blocking(&bogus);
        match ev {
            EngineEvent::Error(msg) => assert!(msg.contains("unsupported"), "got: {msg}"),
            other => panic!("expected Error, got {other:?}"),
        }
        // State must be unchanged after a blocking call: callers decide.
        assert!(!ctrl.state().busy);
    }

    #[test]
    fn list_nonexistent_file_yields_error_event() {
        let tmp = tempdir().expect("tempdir");
        let missing = tmp.path().join("does-not-exist.zip");

        let ctrl = AppController::default();
        let ev = ctrl.list_archive_blocking(&missing);
        assert!(matches!(ev, EngineEvent::Error(_)));
    }

    #[test]
    fn list_zip_round_trip_loads_archive_into_state() {
        let tmp = tempdir().expect("tempdir");
        let zip = tmp.path().join("sample.zip");
        build_small_zip(&zip);

        let mut ctrl = AppController::default();
        ctrl.mark_busy("listing…");
        assert!(ctrl.state().busy);

        let ev = ctrl.list_archive_blocking(&zip);
        ctrl.apply(ev);
        ctrl.drain();

        let state = ctrl.state();
        assert!(!state.busy, "list should clear busy");
        let oa = state.open_archive.as_ref().expect("archive loaded");
        assert_eq!(oa.backend_name, "zip");
        assert_eq!(oa.entries.len(), 2);
        let names: Vec<&str> = oa.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"hello.txt"));
        assert!(names.contains(&"data.bin"));
    }

    #[test]
    fn apply_done_clears_busy_and_keeps_archive() {
        let mut ctrl = AppController::default();
        ctrl.mark_busy("test…");
        ctrl.apply(EngineEvent::Done("OK".into()));
        let s = ctrl.state();
        assert!(!s.busy);
        assert_eq!(s.status, "OK");
    }

    #[test]
    fn apply_error_prefixes_status() {
        let mut ctrl = AppController::default();
        ctrl.apply(EngineEvent::Error("boom".into()));
        let s = ctrl.state();
        assert!(!s.busy);
        assert!(s.status.starts_with("error: "));
        assert!(s.status.contains("boom"));
    }

    #[test]
    fn cancel_marks_handle_cancelled() {
        let ctrl = AppController::default();
        let handle = ctrl.cancel_handle();
        assert!(!handle.is_cancelled());
        ctrl.cancel();
        assert!(handle.is_cancelled());
    }

    #[test]
    fn listed_event_resets_cancel_flag() {
        // After a cancel, a fresh `Listed` should clear the flag so the
        // next operation isn't pre-cancelled.
        let mut ctrl = AppController::default();
        ctrl.cancel();
        assert!(ctrl.cancel_handle().is_cancelled());
        ctrl.apply(EngineEvent::Listed {
            path: PathBuf::from("/tmp/a.zip"),
            backend_name: "zip",
            entries: vec![],
        });
        assert!(!ctrl.cancel_handle().is_cancelled());
    }

    #[test]
    fn drain_consumes_all_pending_events() {
        let mut ctrl = AppController::default();
        let tx = ctrl.engine_sender();
        tx.send(EngineEvent::Done("first".into())).expect("send");
        tx.send(EngineEvent::Done("second".into())).expect("send");
        tx.send(EngineEvent::Error("third".into())).expect("send");
        let n = ctrl.drain();
        assert_eq!(n, 3);
        // The most recent event wins on the status line.
        assert!(ctrl.state().status.contains("third"));
    }

    #[test]
    fn with_limits_uses_provided_limits() {
        let limits = Limits::default();
        let ctrl = AppController::with_limits(limits);
        // We only assert that the limits round-trip; their inner fields
        // are not part of the public API yet.
        assert_eq!(ctrl.limits().max_archive_size, limits.max_archive_size);
    }
}
