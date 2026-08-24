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

use supazip_core::traits::ProgressState as CoreProgressState;
use supazip_core::{formats, ArchiveEntry, Limits};

pub mod progress;

pub use progress::{show_progress_modal, GuiProgress, ProgressState};

pub mod dialogs;
pub use dialogs::{PasswordDialogState, PasswordTarget};

pub mod dnd;

pub use dnd::handle_dropped_files;

pub mod recent;

pub use recent::{RecentEntry, MAX_ENTRIES};

pub mod context_menu;
pub use context_menu::{EntryAction, EntryContextAction};

pub mod menubar;
pub use menubar::{MenuAction, MenuActionOutcome};

pub mod icons;
pub mod settings;
pub mod settings_window;
pub mod theme;

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
    /// Persistent recent-files list, most-recent first, capped at
    /// [`MAX_ENTRIES`]. Loaded on startup and rewritten every time a new
    /// entry is pushed (see [`AppController::apply`]).
    pub recent: Vec<RecentEntry>,
    /// WS-F: when `true`, the GUI renders a debug overlay that dumps the
    /// current `AppState`. Toggled through the `View` menu.
    pub show_debug: bool,
    /// WS-F: when `true`, the GUI renders the About modal. Set by
    /// `dispatch_menu_action(MenuAction::About)` and cleared when the
    /// modal is closed.
    pub show_about: bool,
    /// WS-F/WS-G: persistent settings loaded from `settings.json`.
    pub settings: settings::Settings,
    /// WS-G: when `true`, the GUI renders the settings window.
    pub show_settings: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            open_archive: None,
            status: "Open an archive to get started.".to_string(),
            busy: false,
            recent: recent::load(),
            show_debug: false,
            show_about: false,
            settings: settings::Settings::load(),
            show_settings: false,
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
    /// The GUI accepted an extract request from the user (e.g. a context
    /// menu item). The controller records the intent in state — the worker
    /// that actually performs the extraction is dispatched by the GUI
    /// front-end, mirroring the toolbar `Extract` button.
    Extract {
        entries: Vec<String>,
        dest: PathBuf,
        password: Option<String>,
    },
    /// An operation finished successfully; carries a free-form status line.
    Done(String),
    /// The engine produced an error; the GUI shows it in the status bar and
    /// re-enables the toolbar.
    Error(String),
    /// The engine needs a password to continue. The controller opens the
    /// password dialog; the GUI then re-dispatches the matching operation
    /// (see [`PasswordOpKind`]).
    PasswordRequired { path: PathBuf, kind: PasswordOpKind },
}

/// Distinguishes the operation that hit a password prompt. The GUI uses
/// this to know which worker to re-spawn after the user submits a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordOpKind {
    /// `Open` / list on a header-encrypted archive.
    Open,
    /// Extract from an encrypted archive.
    Extract,
    /// Create a new encrypted archive.
    Create,
    /// Test (integrity check) on an encrypted archive.
    Test,
}

impl PasswordOpKind {
    /// Map the kind to the matching [`PasswordTarget`] variant.
    pub fn into_target(self, path: PathBuf) -> PasswordTarget {
        match self {
            PasswordOpKind::Open | PasswordOpKind::Test => PasswordTarget::Open(path),
            PasswordOpKind::Extract => PasswordTarget::Extract(path),
            PasswordOpKind::Create => PasswordTarget::Create(path),
        }
    }
}

/// Headless controller: state + channel. The eframe `App` in `main.rs`
/// owns one of these and routes UI events into it.
pub struct AppController {
    state: AppState,
    engine_tx: Sender<EngineEvent>,
    engine_rx: Receiver<EngineEvent>,
    cancel_flag: Arc<CoreProgressState>,
    /// WS-C: in-flight progress surfaced as the modal progress dialog.
    /// `Some` while a long-running operation is in flight; `None` when
    /// the controller is idle. The same `Arc` is handed to the worker
    /// through [`Self::start_progress`] so the worker can push per-entry
    /// updates through the [`crate::GuiProgress`] adapter.
    progress: Option<Arc<ProgressState>>,
    limits: Limits,
    /// WS-E: modal password dialog state. Hidden by default; the GUI
    /// opens it when an `EngineEvent::PasswordRequired` arrives.
    password_dialog: PasswordDialogState,
}

impl Default for AppController {
    fn default() -> Self {
        let (engine_tx, engine_rx) = channel();
        Self {
            state: AppState::default(),
            engine_tx,
            engine_rx,
            cancel_flag: Arc::new(CoreProgressState::new()),
            progress: None,
            limits: Limits::default(),
            password_dialog: PasswordDialogState::default(),
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

    /// Mutably borrow the state. The eframe `App` uses this for fields
    /// that are owned by the controller but mutated by the GUI (e.g.
    /// the about-modal flag in [`crate::menubar`]).
    pub fn state_mut(&mut self) -> &mut AppState {
        &mut self.state
    }

    /// Mutably borrow the password dialog state. The eframe `App` uses
    /// this in its `ui` method to render the modal and pull a submitted
    /// password out of the returned `Option`.
    pub fn password_dialog(&mut self) -> &mut PasswordDialogState {
        &mut self.password_dialog
    }

    /// Borrow the password dialog state immutably (e.g. for tests that
    /// only assert the visibility / target).
    pub fn password_dialog_ref(&self) -> &PasswordDialogState {
        &self.password_dialog
    }

    /// Open the password dialog for `target`. Convenience wrapper used by
    /// both the `PasswordRequired` event handler and the `ExtractTo`
    /// context-menu action when the archive is already known to be
    /// encrypted.
    pub fn request_password(&mut self, target: PasswordTarget) {
        self.password_dialog.open(target);
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
                    path: path.clone(),
                    backend_name,
                    entries,
                });
                self.state.busy = false;
                self.state.status = "loaded".to_string();
                self.cancel_flag = Arc::new(CoreProgressState::new());
                self.finish_progress();
                // WS-D: successful open → push to recent files (cap 10,
                // dedup, JSON-persisted). Best-effort: an I/O error on
                // `save` is logged and swallowed so the open still
                // succeeds from the user's point of view.
                recent::push(&mut self.state.recent, RecentEntry::now(path));
                if let Err(e) = recent::save(&self.state.recent) {
                    log::warn!("failed to persist recent files: {e}");
                }
            }
            EngineEvent::Extract {
                entries,
                dest,
                password: _,
            } => {
                // The GUI front-end turns this into a worker thread that
                // calls into the engine. The controller records the
                // intent in the status line so tests can assert on state
                // without a window.
                self.state.busy = true;
                self.state.status = format!(
                    "extracting {} entr{} to {}…",
                    entries.len(),
                    if entries.len() == 1 { "y" } else { "ies" },
                    dest.display()
                );
            }
            EngineEvent::Done(msg) => {
                self.state.busy = false;
                self.state.status = msg;
                self.cancel_flag = Arc::new(CoreProgressState::new());
                self.finish_progress();
            }
            EngineEvent::Error(msg) => {
                self.state.busy = false;
                self.state.status = format!("error: {msg}");
                self.cancel_flag = Arc::new(CoreProgressState::new());
                self.finish_progress();
            }
            EngineEvent::PasswordRequired { path, kind } => {
                self.state.busy = false;
                self.state.status = "password required".to_string();
                self.cancel_flag = Arc::new(CoreProgressState::new());
                self.finish_progress();
                self.password_dialog.open(kind.into_target(path));
            }
        }
    }

    /// Handle one right-click context-menu action. Pure state mutation:
    /// the worker thread (if any) is the caller's responsibility. Each
    /// branch mirrors what `main.rs` does for the toolbar buttons.
    pub fn dispatch_entry_action(&mut self, action: EntryContextAction) {
        match action.kind {
            EntryAction::ExtractHere(dest) => {
                self.apply(EngineEvent::Extract {
                    entries: vec![action.entry_name],
                    dest,
                    password: None,
                });
            }
            EntryAction::ExtractTo(dest) => {
                // WS-E: if the open archive is encrypted, do not extract
                // directly — open the password dialog so the user can
                // supply the key. The GUI's `ui` loop reads the dialog's
                // submission and re-dispatches the extract worker with
                // the entered password.
                if let Some(oa) = &self.state.open_archive {
                    if oa.entries.iter().any(|e| e.encrypted) {
                        let path = oa.path.clone();
                        self.request_password(PasswordTarget::Extract(path.clone()));
                        self.state.status = format!(
                            "encrypted archive — password required for {}",
                            path.display()
                        );
                        return;
                    }
                }
                // Same code path as ExtractHere in 0.3.0; the variant
                // is kept distinct so 0.4 can split them.
                self.apply(EngineEvent::Extract {
                    entries: vec![action.entry_name],
                    dest,
                    password: None,
                });
            }
            EntryAction::TestEntry => {
                // TODO(gui): `ArchiveFormat` does not expose a per-entry
                // test today. The toolbar "Test" button runs the whole
                // archive through `ArchiveFormat::test`; until the core
                // API gains `test_entry`, the context-menu item records
                // the intent in the status line and does not spawn a
                // worker.
                self.state.status =
                    format!("Test entry: {} (TODO: backend support)", action.entry_name);
            }
            EntryAction::CopyPath => {
                // The actual `egui::Context::copy_text` call lives in
                // `context_menu::show_entry_context_menu` (where the
                // egui context is available). The status line is the
                // visible confirmation.
                self.state.status = format!("Copied: {}", action.entry_name);
            }
        }
    }

    /// WS-F: handle one menu-bar action that can be applied without a
    /// window. Returns the variants that the GUI front-end still has to
    /// execute (file pickers, `ViewportCommand::Close`) — those need a
    /// live `egui::Context` and are not part of the headless controller.
    ///
    /// The split mirrors `dispatch_entry_action`: the controller records
    /// the intent and the GUI performs the side effects.
    pub fn dispatch_menu_action(&mut self, action: MenuAction) -> MenuActionOutcome {
        match action {
            MenuAction::ToggleDebug => {
                self.state.show_debug = !self.state.show_debug;
                let s = self.state.show_debug;
                self.state.status = format!("debug overlay: {}", if s { "on" } else { "off" });
                MenuActionOutcome::Done
            }
            MenuAction::Close => match self.close_archive() {
                true => MenuActionOutcome::Done,
                false => MenuActionOutcome::Noop,
            },
            MenuAction::About => {
                self.state.show_about = true;
                MenuActionOutcome::Done
            }
            MenuAction::Settings => {
                self.state.show_settings = true;
                MenuActionOutcome::Done
            }
            // The remaining variants need rfd dialogs or a viewport
            // command. The GUI front-end handles them after collecting
            // the action list from `menubar::show_menu_bar`.
            MenuAction::Open
            | MenuAction::Extract
            | MenuAction::Create
            | MenuAction::Test
            | MenuAction::Quit => MenuActionOutcome::Gui,
        }
    }

    /// Close the currently open archive: drop the entry list and update
    /// the status line. Returns `true` if there was an open archive to
    /// close, `false` if the menu fired on an empty window.
    pub fn close_archive(&mut self) -> bool {
        if self.state.open_archive.is_some() {
            self.state.open_archive = None;
            self.state.busy = false;
            self.state.status = "closed".to_string();
            self.cancel_flag = Arc::new(CoreProgressState::new());
            self.finish_progress();
            true
        } else {
            self.state.status = "no archive open".to_string();
            false
        }
    }

    /// Mark the controller as busy with the given status line. The GUI
    /// calls this immediately before spawning a worker; tests can call it
    /// to set up the precondition for a `Done`/`Error` event.
    pub fn mark_busy(&mut self, status: impl Into<String>) {
        self.state.busy = true;
        self.state.status = status.into();
    }

    /// Begin opening `path` for listing. The controller enters the busy
    /// state with a status line; the caller (the eframe `App`) is expected
    /// to spawn a worker that calls [`Self::list_archive_blocking`] and
    /// feeds the result back through [`Self::apply`]. Returns `true` when
    /// the request was accepted (i.e. the path is non-empty).
    ///
    /// This is the entry point used by drag-and-drop and any future
    /// "open this file" path that does not go through [`rfd::FileDialog`].
    pub fn open_archive(&mut self, path: PathBuf) -> bool {
        if path.as_os_str().is_empty() {
            return false;
        }
        self.mark_busy(format!("opening {}…", path.display()));
        true
    }

    /// Borrow the current status line. Used by the DnD handler and the
    /// view layer to surface non-blocking hints to the user.
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.state.status = status.into();
    }

    /// Cancel any in-flight engine operation. The cancellation flag is
    /// shared with the worker thread via the `Arc<CoreProgressState>`
    /// returned by `cancel_handle`.
    pub fn cancel(&self) {
        self.cancel_flag.cancel();
    }

    /// Borrow the shared cancellation handle for handing to a worker.
    pub fn cancel_handle(&self) -> Arc<CoreProgressState> {
        self.cancel_flag.clone()
    }

    /// WS-C: enter the "in-flight" progress state. Allocates a fresh
    /// [`ProgressState`] (the GUI re-export, distinct from the
    /// core's `CoreProgressState`), installs it on the controller, and
    /// returns a clone for the worker to write to. The next call to
    /// [`Self::finish_progress`] (or any terminal `apply` event) drops
    /// the reference and the modal dialog disappears.
    ///
    /// The cancel flag is left pointing at the core `CoreProgressState`,
    /// which is what the engine accepts as a `&dyn ProgressCallback`.
    /// The GUI's [`ProgressState`] is what the modal dialog reads. The
    /// two share no state directly — the modal is driven by
    /// `set_progress` / `set_message` callbacks from the engine, while
    /// the cancel flag is set directly by the GUI's Cancel button.
    pub fn start_progress(&mut self) -> Arc<ProgressState> {
        let state = Arc::new(ProgressState::new());
        self.progress = Some(state.clone());
        state
    }

    /// WS-C: drop the in-flight progress state. Idempotent — calling it
    /// when no operation is running is a no-op.
    pub fn finish_progress(&mut self) {
        self.progress = None;
    }

    /// WS-C: borrow the in-flight progress state, if any. The GUI calls
    /// this once per frame to decide whether to render the modal.
    pub fn progress(&self) -> Option<&Arc<ProgressState>> {
        self.progress.as_ref()
    }

    /// Current resource limits. Workers pass this into the engine.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Empty the recent-files list and rewrite the on-disk JSON. Used by
    /// the toolbar "Clear recent" menu item. Returns the I/O error from
    /// `save` if the file could not be written; the in-memory list is
    /// cleared regardless.
    pub fn clear_recent(&mut self) -> std::io::Result<()> {
        recent::clear(&mut self.state.recent);
        recent::save(&self.state.recent)
    }

    /// Drop any recent entries whose file is missing on disk, then
    /// persist. Returns the number of pruned entries. Used by the toolbar
    /// "Recent" menu when it opens, for lazy pruning without a background
    /// thread.
    pub fn prune_recent_missing(&mut self) -> usize {
        let removed = recent::prune_missing(&mut self.state.recent);
        if removed > 0 {
            if let Err(e) = recent::save(&self.state.recent) {
                log::warn!("failed to persist recent files after prune: {e}");
            }
        }
        removed
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
    fn listed_event_pushes_to_recent_files() {
        // WS-D: a successful open must record the path in the recent
        // list, dedup, and respect the cap. We use real tempdir files
        // so the `size_bytes` field is non-None and `RecentEntry::now`
        // does not silently fall back to a `None` size.
        let tmp = tempdir().expect("tempdir");
        let a = tmp.path().join("a.zip");
        let b = tmp.path().join("b.zip");
        std::fs::write(&a, b"x").expect("write a");
        std::fs::write(&b, b"yy").expect("write b");

        let mut ctrl = AppController::default();
        ctrl.clear_recent().ok();

        ctrl.apply(EngineEvent::Listed {
            path: a.clone(),
            backend_name: "zip",
            entries: vec![],
        });
        assert_eq!(ctrl.state().recent.len(), 1);
        assert_eq!(ctrl.state().recent[0].path, a);

        // Re-opening the same path must dedup, not duplicate.
        ctrl.apply(EngineEvent::Listed {
            path: a.clone(),
            backend_name: "zip",
            entries: vec![],
        });
        assert_eq!(ctrl.state().recent.len(), 1);

        ctrl.apply(EngineEvent::Listed {
            path: b.clone(),
            backend_name: "zip",
            entries: vec![],
        });
        assert_eq!(ctrl.state().recent.len(), 2);
        // Most-recent first.
        assert_eq!(ctrl.state().recent[0].path, b);
        assert_eq!(ctrl.state().recent[1].path, a);
    }

    #[test]
    fn clear_recent_empties_list_and_persists() {
        let tmp = tempdir().expect("tempdir");
        let a = tmp.path().join("a.zip");
        std::fs::write(&a, b"x").expect("write a");
        let mut ctrl = AppController::default();
        ctrl.apply(EngineEvent::Listed {
            path: a,
            backend_name: "zip",
            entries: vec![],
        });
        assert!(!ctrl.state().recent.is_empty());
        ctrl.clear_recent().expect("clear_recent");
        assert!(ctrl.state().recent.is_empty());
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

    #[test]
    fn dispatch_extract_here_marks_busy_with_status() {
        let mut ctrl = AppController::default();
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "hello.txt".into(),
            kind: EntryAction::ExtractHere(PathBuf::from("/tmp/out")),
        });
        let s = ctrl.state();
        assert!(s.busy);
        assert!(s.status.contains("1 entry"), "status: {}", s.status);
        assert!(s.status.contains("/tmp/out"), "status: {}", s.status);
    }

    #[test]
    fn dispatch_extract_to_takes_same_path_as_extract_here() {
        let mut ctrl = AppController::default();
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "data.bin".into(),
            kind: EntryAction::ExtractTo(PathBuf::from("/var/tmp")),
        });
        let s = ctrl.state();
        assert!(s.busy);
        assert!(s.status.contains("1 entry"));
        assert!(s.status.contains("/var/tmp"));
    }

    #[test]
    fn dispatch_test_entry_records_todo_in_status() {
        let mut ctrl = AppController::default();
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "deep/path.txt".into(),
            kind: EntryAction::TestEntry,
        });
        let s = ctrl.state();
        assert!(!s.busy, "TestEntry is a no-op until core gains test_entry");
        assert!(s.status.contains("Test entry"));
        assert!(s.status.contains("deep/path.txt"));
        assert!(s.status.contains("TODO"));
    }

    #[test]
    fn dispatch_copy_path_sets_status_and_does_not_busy() {
        let mut ctrl = AppController::default();
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "src/main.rs".into(),
            kind: EntryAction::CopyPath,
        });
        let s = ctrl.state();
        assert!(!s.busy);
        assert!(s.status.contains("Copied"));
        assert!(s.status.contains("src/main.rs"));
    }

    #[test]
    fn apply_extract_event_marks_busy() {
        let mut ctrl = AppController::default();
        ctrl.apply(EngineEvent::Extract {
            entries: vec!["a.txt".into(), "b.txt".into()],
            dest: PathBuf::from("/tmp/out"),
            password: None,
        });
        let s = ctrl.state();
        assert!(s.busy);
        assert!(s.status.contains("2 entries"));
        assert!(s.status.contains("/tmp/out"));
    }

    #[test]
    fn password_dialog_starts_invisible_and_empty() {
        let ctrl = AppController::default();
        let d = ctrl.password_dialog_ref();
        assert!(!d.visible);
        assert!(d.password.is_empty());
        assert!(d.target.is_none());
    }

    #[test]
    fn apply_password_required_opens_dialog() {
        let mut ctrl = AppController::default();
        ctrl.mark_busy("opening…");
        ctrl.apply(EngineEvent::PasswordRequired {
            path: PathBuf::from("/tmp/secret.7z"),
            kind: PasswordOpKind::Open,
        });
        let d = ctrl.password_dialog_ref();
        assert!(d.visible, "PasswordRequired should open the dialog");
        assert!(!ctrl.state().busy, "controller should not stay busy");
        let target = d.target.as_ref().expect("dialog has target");
        assert_eq!(target.path(), PathBuf::from("/tmp/secret.7z").as_path());
    }

    #[test]
    fn request_password_opens_dialog_for_target() {
        let mut ctrl = AppController::default();
        ctrl.request_password(PasswordTarget::Extract(PathBuf::from("/a.zip")));
        assert!(ctrl.password_dialog_ref().visible);
        assert!(matches!(
            ctrl.password_dialog_ref().target,
            Some(PasswordTarget::Extract(_))
        ));
    }

    #[test]
    fn dispatch_extract_to_on_encrypted_archive_opens_password_dialog() {
        let mut ctrl = AppController::default();
        // Simulate a loaded archive with one encrypted entry.
        ctrl.apply(EngineEvent::Listed {
            path: PathBuf::from("/tmp/locked.7z"),
            backend_name: "7z",
            entries: vec![OpenEntry {
                name: "secret.txt".into(),
                size: 42,
                encrypted: true,
            }],
        });
        assert!(!ctrl.password_dialog_ref().visible);
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "secret.txt".into(),
            kind: EntryAction::ExtractTo(PathBuf::from("/tmp/out")),
        });
        assert!(
            ctrl.password_dialog_ref().visible,
            "ExtractTo must open dialog on encrypted archive"
        );
        assert!(!ctrl.state().busy, "no extract worker should be queued yet");
    }

    #[test]
    fn dispatch_extract_to_on_plain_archive_skips_password_dialog() {
        let mut ctrl = AppController::default();
        ctrl.apply(EngineEvent::Listed {
            path: PathBuf::from("/tmp/plain.zip"),
            backend_name: "zip",
            entries: vec![OpenEntry {
                name: "readme.txt".into(),
                size: 10,
                encrypted: false,
            }],
        });
        ctrl.dispatch_entry_action(EntryContextAction {
            entry_name: "readme.txt".into(),
            kind: EntryAction::ExtractTo(PathBuf::from("/tmp/out")),
        });
        assert!(!ctrl.password_dialog_ref().visible);
        assert!(ctrl.state().busy);
    }

    #[test]
    fn password_op_kind_into_target_maps_each_variant() {
        let p = PathBuf::from("/x.7z");
        assert!(matches!(
            PasswordOpKind::Open.into_target(p.clone()),
            PasswordTarget::Open(_)
        ));
        assert!(matches!(
            PasswordOpKind::Extract.into_target(p.clone()),
            PasswordTarget::Extract(_)
        ));
        assert!(matches!(
            PasswordOpKind::Create.into_target(p.clone()),
            PasswordTarget::Create(_)
        ));
        // Test and Open both surface as Open for the dialog (test reuses
        // the open listing code path).
        assert!(matches!(
            PasswordOpKind::Test.into_target(p),
            PasswordTarget::Open(_)
        ));
    }

    // -----------------------------------------------------------------------
    // WS-C: progress-dialog wiring on the controller.
    // -----------------------------------------------------------------------

    #[test]
    fn start_progress_installs_state() {
        let mut ctrl = AppController::default();
        assert!(ctrl.progress().is_none());
        let handle = ctrl.start_progress();
        assert!(ctrl.progress().is_some(), "progress should be set");
        assert!(Arc::ptr_eq(ctrl.progress().unwrap(), &handle));
    }

    #[test]
    fn finish_progress_drops_state() {
        let mut ctrl = AppController::default();
        let _h = ctrl.start_progress();
        assert!(ctrl.progress().is_some());
        ctrl.finish_progress();
        assert!(ctrl.progress().is_none());
    }

    #[test]
    fn apply_done_clears_progress_state() {
        let mut ctrl = AppController::default();
        let _h = ctrl.start_progress();
        assert!(ctrl.progress().is_some());
        ctrl.apply(EngineEvent::Done("ok".into()));
        assert!(ctrl.progress().is_none());
    }

    #[test]
    fn apply_error_clears_progress_state() {
        let mut ctrl = AppController::default();
        let _h = ctrl.start_progress();
        assert!(ctrl.progress().is_some());
        ctrl.apply(EngineEvent::Error("boom".into()));
        assert!(ctrl.progress().is_none());
    }

    #[test]
    fn start_progress_does_not_clobber_cancel_handle() {
        // The cancel flag stays on `CoreProgressState`; `start_progress`
        // only manages the GUI's `ProgressState`. A prior cancel must
        // remain observable to the in-flight worker, and a new
        // `start_progress` must not reset the cancel flag (it is up to
        // the GUI to do that explicitly, since the worker may not have
        // observed the prior cancel yet).
        let mut ctrl = AppController::default();
        ctrl.cancel();
        assert!(ctrl.cancel_handle().is_cancelled());
        let _h = ctrl.start_progress();
        assert!(ctrl.cancel_handle().is_cancelled());
    }
}
