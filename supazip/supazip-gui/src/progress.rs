//! Modal view over the core's shared progress/cancellation handle.
//!
//! The controller stores one [`ProgressState`] per in-flight operation. The
//! worker passes the same `Arc<ProgressState>` to the archive backend, while
//! the UI reads it once per frame. This keeps progress and cancellation on a
//! single source of truth.

use std::sync::atomic::Ordering;

use eframe::egui;
use supazip_core::traits::ProgressState as CoreProgressState;

/// Public GUI name for the core's thread-safe progress handle.
pub type ProgressState = CoreProgressState;

fn fraction(state: &ProgressState) -> f32 {
    let total = state.total.load(Ordering::Relaxed);
    if total == 0 {
        0.0
    } else {
        (state.current.load(Ordering::Relaxed) as f32 / total as f32).clamp(0.0, 1.0)
    }
}

/// Render the modal for an operation started with
/// [`crate::AppController::begin_progress_operation`].
pub fn show_progress_modal(ctx: &egui::Context, state: &ProgressState) {
    let current = state.current.load(Ordering::Relaxed);
    let total = state.total.load(Ordering::Relaxed);
    let progress_text = if total == 0 {
        format!("{current} processed")
    } else {
        format!("{current}/{total}")
    };
    let message = state
        .message
        .lock()
        .map(|message| message.clone())
        .unwrap_or_default();

    egui::Modal::new(egui::Id::new("operation_progress_modal"))
        .frame(crate::theme::dialog_frame(ctx))
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.heading("Working…");
            ui.add(egui::ProgressBar::new(fraction(state)).text(progress_text));
            if !message.is_empty() {
                ui.label(format!("Current: {message}"));
            }
            if ui.button("Cancel").clicked() {
                state.cancel();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fraction_is_zero_until_total_is_known() {
        let state = ProgressState::new();
        state.set_progress(25, 0);
        assert_eq!(fraction(&state), 0.0);
    }

    #[test]
    fn fraction_tracks_and_clamps_core_progress() {
        let state = ProgressState::new();
        state.set_progress(5, 10);
        assert_eq!(fraction(&state), 0.5);
        state.set_progress(20, 10);
        assert_eq!(fraction(&state), 1.0);
    }

    #[test]
    fn cancel_updates_the_same_handle_the_worker_reads() {
        let state = ProgressState::new();
        assert!(!state.is_cancelled());
        state.cancel();
        assert!(state.is_cancelled());
    }
}
