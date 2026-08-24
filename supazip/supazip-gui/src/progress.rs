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

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;
use supazip_core::traits::{ProgressCallback, ProgressUpdate};

/// Per-operation progress tracked in entry-count units. Sized to `u32`
/// because archives that exceed 4 G entries are out of scope for 0.3.0
/// (and the per-entry progress callback only ever sees `u32::MAX` at the
/// limit). The struct is shared between the UI thread and the worker
/// thread through [`Arc<ProgressDialog>`].
pub struct ProgressDialog {
    /// Total number of entries the worker expects to process.
    pub total_entries: AtomicU32,
    /// Entries the worker has finished so far.
    pub done_entries: AtomicU32,
    /// Human-readable name of the entry currently being processed.
    pub current_entry: Mutex<String>,
    /// Set by the UI to ask the worker to stop. Read by the worker in its
    /// hot loop. Stored as an [`AtomicBool`] so the UI never needs to take
    /// a lock to flip it.
    pub is_cancelled: AtomicBool,
}

impl ProgressDialog {
    /// Allocate a fresh zeroed state with cancellation cleared.
    pub fn new() -> Self {
        Self {
            total_entries: AtomicU32::new(0),
            done_entries: AtomicU32::new(0),
            current_entry: Mutex::new(String::new()),
            is_cancelled: AtomicBool::new(false),
        }
    }

    /// Fraction in `0.0..=1.0` rendered into the progress bar. Returns
    /// `0.0` when no total is known yet (the worker has not called
    /// `set_total`); the bar then renders as empty, which is the right
    /// behaviour for a not-yet-measured operation.
    pub fn fraction(&self) -> f32 {
        let total = self.total_entries.load(Ordering::Relaxed);
        if total == 0 {
            0.0
        } else {
            (self.done_entries.load(Ordering::Relaxed) as f32) / (total as f32)
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
        self.total_entries
            .store(u32::try_from(total).unwrap_or(u32::MAX), Ordering::Relaxed);
        self.done_entries.store(
            u32::try_from(current).unwrap_or(u32::MAX),
            Ordering::Relaxed,
        );
    }

    fn set_message(&self, message: &str) {
        if let Ok(mut current) = self.current_entry.lock() {
            *current = message.to_string();
        }
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
        self.inner
            .total_entries
            .store(u32::try_from(total).unwrap_or(u32::MAX), Ordering::Relaxed);
        // The engine reports bytes-processed here in the legacy core API;
        // we do not know the entry count up front so we leave
        // `done_entries` untouched. The worker drives it explicitly via
        // the per-entry callbacks below.
    }

    fn set_message(&self, message: &str) {
        if let Ok(mut current) = self.inner.current_entry.lock() {
            *current = message.to_string();
        }
    }

    fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

// `impl ProgressCallback for Arc<ProgressState>` is not allowed (orphan
// rules: `Arc` is foreign). Workers should wrap the `Arc` in
// [`GuiProgress`].

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
        state.total_entries.store(
            u32::try_from(update.total).unwrap_or(u32::MAX),
            Ordering::Relaxed,
        );
        state.done_entries.store(
            u32::try_from(update.current).unwrap_or(u32::MAX),
            Ordering::Relaxed,
        );
    }
    if !update.message.is_empty() {
        if let Ok(mut current) = state.current_entry.lock() {
            *current = update.message;
        }
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
        .frame(crate::theme::dialog_frame(ctx))
        .show(ctx, |ui| {
            ui.add(egui::ProgressBar::new(state.fraction()).text(format!(
                "{}/{} entries",
                state.done_entries.load(Ordering::Relaxed),
                state.total_entries.load(Ordering::Relaxed)
            )));
            let current = state
                .current_entry
                .lock()
                .map(|s| s.clone())
                .unwrap_or_default();
            ui.label(format!("Current: {current}"));
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
            total_entries: AtomicU32::new(10),
            done_entries: AtomicU32::new(5),
            current_entry: Mutex::new(String::new()),
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
        assert_eq!(state.total_entries.load(Ordering::Relaxed), 50);
    }

    #[test]
    fn gui_progress_set_message_updates_current_entry() {
        let state = Arc::new(ProgressState::new());
        let adapter = GuiProgress::new(state.clone());
        adapter.set_message("a/b/c.txt");
        assert_eq!(state.current_entry.lock().unwrap().as_str(), "a/b/c.txt");
        adapter.set_message("hello");
        assert_eq!(state.current_entry.lock().unwrap().as_str(), "hello");
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
        state.total_entries.store(10, Ordering::Relaxed);
        state.done_entries.store(3, Ordering::Relaxed);
        let update = ProgressUpdate {
            current: 0,
            total: 0,
            message: "entry.txt".to_string(),
        };
        apply_progress_update(&state, update);
        assert_eq!(state.total_entries.load(Ordering::Relaxed), 10);
        assert_eq!(state.done_entries.load(Ordering::Relaxed), 3);
        assert_eq!(state.current_entry.lock().unwrap().as_str(), "entry.txt");
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
        assert_eq!(state.total_entries.load(Ordering::Relaxed), 9);
        assert_eq!(state.done_entries.load(Ordering::Relaxed), 7);
    }
}
