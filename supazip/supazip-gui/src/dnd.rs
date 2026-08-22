//! Drag-and-drop handler for the main window.
//!
//! The handler reads egui's raw input each frame: hovered files drive a
//! transient overlay ("Drop archive to open") and a successful drop
//! forwards the first path that has an OS handle to
//! [`AppController::open_archive`](crate::AppController::open_archive).
//!
//! `egui::Context` is required for both the input read and the
//! `is_hovering_drop` flag stored in `egui::Memory`. The pure-function
//! helpers ([`opened_paths_from_dropped`], [`hovered_flag_from_hovered`])
//! are exposed so they can be unit-tested without a window.
//!
//! ## Wayland / platform fallback
//!
//! The decision recorded on 2026-06-06 defers full Wayland DnD support
//! to 1.1. On platforms where the system DnD is filtered the user sees
//! the hover overlay but the drop event itself never arrives; the call
//! site in `main.rs` detects that by observing `opened_paths.is_empty()`
//! while `hovered_files` was non-empty on a prior frame and surfaces a
//! status-bar hint pointing the user at the `File -> Open` shortcut.

use std::path::PathBuf;

use eframe::egui;

use crate::AppController;

/// `egui::Memory` key used to communicate "the cursor is currently
/// holding a file over the window" from the input layer to the view
/// layer. The view layer reads it during the same frame and renders the
/// drop overlay; the flag is cleared at the start of every frame so a
/// drop target that stops hovering is hidden within one frame.
pub fn hovering_id() -> egui::Id {
    egui::Id::new("supazip.is_hovering_drop")
}

/// Drain the current frame's dropped files and forward the first path that
/// carries an OS handle to [`AppController::open_archive`]. Also updates
/// the `is_hovering_drop` flag based on the current `hovered_files`.
///
/// Returns the path that was actually forwarded to the controller. At most
/// one path is accepted because the controller enforces a single in-flight
/// operation; any additional paths in the same drop are ignored. The returned
/// `Vec` is empty when nothing was dropped on this frame.
///
/// This function is the single integration point with egui's raw
/// input; it intentionally performs no drawing — the view layer is
/// responsible for the visual overlay.
pub fn handle_dropped_files(ctx: &egui::Context, ctrl: &mut AppController) -> Vec<PathBuf> {
    let dropped = ctx.input(|i| i.raw.dropped_files.clone());
    let hovered = ctx.input(|i| i.raw.hovered_files.clone());

    // Update the hover flag *every* frame so the overlay reflects the
    // current state instead of sticking around after the cursor leaves.
    let hovering = !hovered.is_empty();
    ctx.data_mut(|d| d.insert_temp(hovering_id(), hovering));

    opened_paths_from_dropped(&dropped, ctrl)
}

/// Pure helper: offer dropped paths to the controller until its single worker
/// slot is reserved. Extracted so it can be unit-tested without a window.
pub fn opened_paths_from_dropped(
    dropped: &[egui::DroppedFile],
    ctrl: &mut AppController,
) -> Vec<PathBuf> {
    let mut opened = Vec::with_capacity(dropped.len());
    for df in dropped {
        if let Some(path) = df.path.as_ref() {
            if ctrl.open_archive(path.clone()) {
                opened.push(path.clone());
            }
        }
    }
    opened
}

/// Pure helper: translate the current `hovered_files` slice into the
/// boolean flag the view layer reads. Extracted for unit testing.
pub fn hovered_flag_from_hovered(hovered: &[egui::HoveredFile]) -> bool {
    !hovered.is_empty()
}

/// Render the "Drop archive to open" overlay on top of the current
/// frame. Called by the view layer when the hover flag is set. The
/// overlay covers the whole screen rectangle at `Order::Foreground`
/// so it sits above all panels.
pub fn render_drop_overlay(ctx: &egui::Context) {
    let screen = ctx.content_rect();
    egui::Area::new(egui::Id::new("dnd_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
                .show(ui, |ui| {
                    ui.set_min_size(screen.size());
                    ui.centered_and_justified(|ui| {
                        ui.heading("Drop archive to open");
                    });
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dropped-files slice with no entries forwards nothing to the
    /// controller and returns an empty `Vec`.
    #[test]
    fn dropped_files_empty_returns_empty() {
        let mut ctrl = AppController::default();
        let opened = opened_paths_from_dropped(&[], &mut ctrl);
        assert!(opened.is_empty());
        // The controller stays idle: nothing was opened.
        assert!(!ctrl.state().busy);
    }

    /// When egui reports at least one hovered file, the flag helper
    /// returns `true` (the actual `egui::Context` round-trip is covered
    /// by integration tests, not by unit tests).
    #[test]
    fn is_hovering_drop_set_when_hovered() {
        let hovered: Vec<egui::HoveredFile> = vec![egui::HoveredFile {
            path: None,
            mime: "text/plain".to_string(),
        }];
        assert!(hovered_flag_from_hovered(&hovered));
        assert!(!hovered_flag_from_hovered(&[]));
    }

    /// A dropped file without a `path` (e.g. a browser drag where egui
    /// only saw the MIME type) is silently ignored — the controller is
    /// not entered into the busy state and nothing is returned to the
    /// caller.
    #[test]
    fn dropped_file_without_path_is_ignored() {
        let mut ctrl = AppController::default();
        let dropped = vec![egui::DroppedFile {
            path: None,
            name: "browser-only.txt".to_string(),
            mime: "text/plain".to_string(),
            last_modified: None,
            bytes: None,
        }];
        let opened = opened_paths_from_dropped(&dropped, &mut ctrl);
        assert!(opened.is_empty());
        assert!(!ctrl.state().busy);
    }

    #[test]
    fn dropped_files_accept_only_one_operation_while_busy() {
        let mut ctrl = AppController::default();
        let dropped = ["first.zip", "second.zip"].map(|name| egui::DroppedFile {
            path: Some(PathBuf::from(name)),
            name: name.to_owned(),
            mime: String::new(),
            last_modified: None,
            bytes: None,
        });

        let opened = opened_paths_from_dropped(&dropped, &mut ctrl);

        assert_eq!(opened, vec![PathBuf::from("first.zip")]);
        assert!(ctrl.state().busy);
    }

    /// `open_archive` rejects an empty path even when the caller
    /// somehow gets one in (defence in depth; the `Path` import in
    /// egui's `DroppedFile` is `Option<PathBuf>` and we already filter
    /// `None` above).
    #[test]
    fn open_archive_rejects_empty_path() {
        let mut ctrl = AppController::default();
        assert!(!ctrl.open_archive(PathBuf::new()));
        assert!(!ctrl.state().busy);
    }
}
