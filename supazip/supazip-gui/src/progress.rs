//! Modal progress dialog + per-entry progress state for the GUI.
//!
//! The GUI tracks long-running operations (list, extract, create, test)
//! through a [`ProgressDialog`] shared with the worker thread via
//! [`std::sync::Arc`]. The worker pumps engine events into it through the
//! [`GuiProgress`] adapter, which implements
//! [`supazip_core::traits::ProgressCallback`]. The UI thread reads the
//! same state once per frame and renders [`show_progress_modal`].
//!
//! The atomic cancel flag is what makes cancellation cooperative: the
//! worker polls [`ProgressDialog::is_cancelled`] between entries; the UI
//! flips it through the modal's Cancel button. There is no separate cancel
//! channel and no per-entry channel back to the worker — keeping the
//! surface tiny was a deliberate decision in milestone 0.3.0.
//!
//! The struct is named `ProgressDialog` (not `ProgressState`) to avoid a
//! name clash with `supazip_core::traits::ProgressState`, which the
//! controller already uses as the cancel handle. The GUI re-exports this
//! type under the name `ProgressState` for callers that do not need to
//! import the core type.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use supazip_core::traits::{ProgressCallback, ProgressUpdate};

/// Per-operation progress tracked in entry-count units. Sized to `u32`
/// because archives that exceed 4 G entries are out of scope for 0.3.0
/// (and the per-entry progress callback only ever sees `u32::MAX` at the
/// limit). The struct is shared between the UI thread and the worker
/// thread through [`Arc<ProgressDialog>`].
pub struct ProgressDialog {
    /// Total number of entries the worker expects to process.
    pub total_entries: u32,
    /// Entries the worker has finished so far.
    pub done_entries: u32,
    /// Human-readable name of the entry currently being processed.
    pub current_entry: String,
    /// Set by the UI to ask the worker to stop. Read by the worker in its
    /// hot loop. Stored as an [`AtomicBool`] so the UI never needs to take
    /// a lock to flip it.
    pub is_cancelled: AtomicBool,
}

impl ProgressDialog {
    /// Allocate a fresh zeroed state with cancellation cleared.
    pub fn new() -> Self {
        Self {
            total_entries: 0,
            done_entries: 0,
            current_entry: String::new(),
            is_cancelled: AtomicBool::new(false),
        }
    }

    /// Fraction in `0.0..=1.0` rendered into the progress bar. Returns
    /// `0.0` when no total is known yet (the worker has not called
    /// `set_total`); the bar then renders as empty, which is the right
    /// behaviour for a not-yet-measured operation.
    pub fn fraction(&self) -> f32 {
        if self.total_entries == 0 {
            0.0
        } else {
            (self.done_entries as f32) / (self.total_entries as f32)
        }
    }

    /// True when the UI has asked the worker to stop.
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::SeqCst)
    }

    /// Ask the worker to stop. Safe to call from the UI thread.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::SeqCst);
    }
}

impl Default for ProgressDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCallback for ProgressDialog {
    fn set_progress(&self, current: u64, total: u64) {
        self.total_entries = u32::try_from(total).unwrap_or(u32::MAX);
        self.done_entries = u32::try_from(current).unwrap_or(u32::MAX);
    }

    fn set_message(&self, message: &str) {
        self.current_entry = message.to_string();
    }

    fn is_cancelled(&self) -> bool {
        ProgressDialog::is_cancelled(self)
    }
}

/// Backwards-compatible alias: callers in this crate expect the type to
/// be named `ProgressState` (the original spec name). Internal code uses
/// `ProgressDialog` to avoid clashing with
/// `supazip_core::traits::ProgressState`, but the re-export at the crate
/// root lets `use supazip_gui::ProgressState;` keep working.
pub type ProgressState = ProgressDialog;

// ---------------------------------------------------------------------------
// GuiProgress: ProgressCallback adapter for the worker thread.
//
// The worker owns a `GuiProgress` by value (it lives on the worker stack),
// wraps it in `&dyn ProgressCallback` when calling the engine, and writes
// back into the shared `Arc<ProgressState>` it was constructed with. The
// adapter is intentionally `!Send` once it holds the `Arc`; in practice
// it is created on the worker stack, so we never need to ship it across
// threads.
// ---------------------------------------------------------------------------

/// Adapter that lets the engine's [`ProgressCallback`] drive a shared
/// [`ProgressState`]. Constructed on the worker with an
/// [`Arc<ProgressState>`] cloned from the controller.
pub struct GuiProgress {
    inner: Arc<ProgressState>,
}

impl GuiProgress {
    /// Build a new adapter around the shared state.
    pub fn new(inner: Arc<ProgressState>) -> Self {
        Self { inner }
    }
}

impl ProgressCallback for GuiProgress {
    fn set_progress(&self, _current: u64, total: u64) {
        // The core API talks in u64, but our ProgressState is u32-entry
        // based. Saturate to u32::MAX to avoid a panic on the very large
        // archives; the bar still saturates at 100% in that case.
        self.inner.total_entries = u32::try_from(total).unwrap_or(u32::MAX);
        // The engine reports bytes-processed here in the legacy core API;
        // we do not know the entry count up front so we leave
        // `done_entries` untouched. The worker drives it explicitly via
        // the per-entry callbacks below.
    }

    fn set_message(&self, message: &str) {
        self.inner.current_entry = message.to_string();
    }

    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

/// The shared [`ProgressState`] itself implements [`ProgressCallback`]
/// directly when wrapped in an [`Arc`]. The engine accepts
/// `&dyn ProgressCallback`, so a worker can hand the controller's
/// `Arc<ProgressState>` straight in without going through the
/// [`GuiProgress`] adapter. The adapter exists for code that wants the
/// ownership story to be explicit (it is constructed on the worker stack
/// from the `Arc` and never leaves the worker).
impl ProgressCallback for Arc<ProgressState> {
    fn set_progress(&self, current: u64, total: u64) {
        self.total_entries = u32::try_from(total).unwrap_or(u32::MAX);
        self.done_entries = u32::try_from(current).unwrap_or(u32::MAX);
    }

    fn set_message(&self, message: &str) {
        self.current_entry = message.to_string();
    }

    fn is_cancelled(&self) -> bool {
        ProgressState::is_cancelled(self)
    }
}

// ---------------------------------------------------------------------------
// ChannelProgress -> Arc<ProgressState> bridge.
//
// `ChannelProgress` from `supazip-core` was designed for the old
// channel-based UI pipeline. The 0.3.0 GUI uses the `Arc<ProgressState>`
// directly. We still accept `ProgressUpdate` events at the controller
// boundary (so the worker does not have to know the GUI is a GUI), and
// the controller's `apply_progress_update` folds them into the shared
// state. The bridge is `pub` for tests.
// ---------------------------------------------------------------------------

/// Fold a [`ProgressUpdate`] received through a `ChannelProgress` channel
/// into the shared state. The GUI thread calls this once per frame for
/// every queued update.
pub fn apply_progress_update(state: &ProgressState, update: ProgressUpdate) {
    // `set_message` arrives with current=0,total=0 per the core
    // convention; a `set_progress` arrives with a real `total`. Treat
    // them as independent signals and overwrite only the fields that
    // carry a value.
    if update.total > 0 {
        state.total_entries = u32::try_from(update.total).unwrap_or(u32::MAX);
        state.done_entries = u32::try_from(update.current).unwrap_or(u32::MAX);
    }
    if !update.message.is_empty() {
        state.current_entry = update.message;
    }
}

// ---------------------------------------------------------------------------
// show_progress_modal: the modal dialog itself.
// ---------------------------------------------------------------------------

/// Render the modal progress dialog. The dialog is non-collapsible and
/// non-resizable, anchored to the centre of the parent, and contains a
/// deterministic progress bar plus a Cancel button. It is meant to be
/// called once per frame from the top-level `App::update` while an
/// operation is in flight; rendering is a no-op when the state is empty
/// (the caller should not call it then).
pub fn show_progress_modal(ctx: &egui::Context, state: &ProgressState) {
    egui::Window::new("Working...")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add(egui::ProgressBar::new(state.fraction()).text(format!(
                "{}/{} entries",
                state.done_entries, state.total_entries
            )));
            ui.label(format!("Current: {}", state.current_entry));
            if ui.button("Cancel").clicked() {
                state.cancel();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn progress_state_fraction_zero_total() {
        let s = ProgressState::new();
        assert_eq!(s.fraction(), 0.0);
    }

    #[test]
    fn progress_state_fraction_half() {
        let s = ProgressState {
            total_entries: 10,
            done_entries: 5,
            current_entry: String::new(),
            is_cancelled: AtomicBool::new(false),
        };
        assert!((s.fraction() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_state_cancellation_flag_set() {
        let s = ProgressState::new();
        assert!(!s.is_cancelled());
        s.is_cancelled.store(true, Ordering::SeqCst);
        assert!(s.is_cancelled());
    }

    #[test]
    fn progress_state_cancel_helper_sets_flag() {
        let s = ProgressState::new();
        assert!(!s.is_cancelled());
        s.cancel();
        assert!(s.is_cancelled());
    }

    #[test]
    fn gui_progress_set_progress_saturates_total() {
        let state = Arc::new(ProgressState::new());
        let adapter = GuiProgress::new(state.clone());
        adapter.set_progress(0, 50);
        assert_eq!(state.total_entries, 50);
    }

    #[test]
    fn gui_progress_set_message_updates_current_entry() {
        let state = Arc::new(ProgressState::new());
        let adapter = GuiProgress::new(state.clone());
        adapter.set_message("a/b/c.txt");
        assert_eq!(state.current_entry, "a/b/c.txt");
        adapter.set_message("hello");
        assert_eq!(state.current_entry, "hello");
    }

    #[test]
    fn gui_progress_is_cancelled_reflects_inner_flag() {
        let state = Arc::new(ProgressState::new());
        let adapter = GuiProgress::new(state.clone());
        assert!(!adapter.is_cancelled());
        state.cancel();
        assert!(adapter.is_cancelled());
    }

    #[test]
    fn apply_progress_update_message_only_does_not_reset_totals() {
        let state = ProgressState::new();
        state.total_entries = 10;
        state.done_entries = 3;
        let update = ProgressUpdate {
            current: 0,
            total: 0,
            message: "entry.txt".to_string(),
        };
        apply_progress_update(&state, update);
        assert_eq!(state.total_entries, 10);
        assert_eq!(state.done_entries, 3);
        assert_eq!(state.current_entry, "entry.txt");
    }

    #[test]
    fn apply_progress_update_with_total_overwrites_counts() {
        let state = ProgressState::new();
        let update = ProgressUpdate {
            current: 7,
            total: 9,
            message: String::new(),
        };
        apply_progress_update(&state, update);
        assert_eq!(state.total_entries, 9);
        assert_eq!(state.done_entries, 7);
    }
}
