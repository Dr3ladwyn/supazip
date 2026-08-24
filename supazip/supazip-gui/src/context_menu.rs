//! Right-click context menu for an entry in the archive file list.
//!
//! The menu is rendered with `egui::Response::context_menu` from the
//! `main.rs` row builder; this module only owns the data types and the
//! per-action handlers. The handlers use `rfd::FileDialog::pick_folder` to
//! ask the user for a destination directory and `egui::Context::copy_text`
//! for the clipboard. None of those side effects are performed when the
//! menu is just opened — they only fire when a menu item is clicked.

#[allow(deprecated)]
use std::fmt;
use std::path::PathBuf;

use eframe::egui;

use crate::OpenEntry;

/// One concrete action the user picked from a row's context menu.
///
/// `entry_name` is captured at click time so the dispatcher can act on it
/// even if the entry list is reloaded between menu-open and click-apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryContextAction {
    pub entry_name: String,
    pub kind: EntryAction,
}

/// The four menu items exposed in milestone 0.3.0 WS-B.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryAction {
    /// Extract the entry into the chosen directory (the user already
    /// confirmed the destination through `rfd::FileDialog`).
    ExtractHere(PathBuf),
    /// Same as `ExtractHere`, but reserved for the "Extract to…" flow that
    /// picks a different destination than the archive's own directory. The
    /// milestone 0.3.0 GUI uses the same code path for both items; the
    /// distinction is kept in the type so the 0.4 release can diverge them
    /// (e.g. default to the archive's parent vs. always prompt).
    ExtractTo(PathBuf),
    /// Probe the entry for integrity. The current core API exposes
    /// `ArchiveFormat::test` (whole-archive) but not a per-entry variant;
    /// until that lands, the GUI records the intent in the status bar
    /// and moves on. A `TODO` comment in `App::dispatch_entry_action`
    /// captures the gap.
    TestEntry,
    /// Copy the entry's in-archive path to the system clipboard through
    /// `egui::Context::copy_text`.
    CopyPath,
}

impl fmt::Display for EntryAction {
    #[allow(deprecated)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryAction::ExtractHere(p) => write!(f, "ExtractHere({})", p.display()),
            EntryAction::ExtractTo(p) => write!(f, "ExtractTo({})", p.display()),
            EntryAction::TestEntry => f.write_str("TestEntry"),
            EntryAction::CopyPath => f.write_str("CopyPath"),
        }
    }
}

/// Render the four context-menu items into `ui` and return the action the
/// user picked, if any. The caller decides what to do with it; this
/// function does not touch the engine, the clipboard, or the file system
/// beyond asking `rfd` for a destination folder.
#[allow(deprecated)]
pub fn show_entry_context_menu(ui: &mut egui::Ui, entry: &OpenEntry) -> Option<EntryContextAction> {
    let mut action: Option<EntryContextAction> = None;

    if ui.button("Extract here\u{2026}").clicked() {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            action = Some(EntryContextAction {
                entry_name: entry.name.clone(),
                kind: EntryAction::ExtractHere(dir),
            });
        }
        ui.close_menu();
    }
    if ui.button("Extract to\u{2026}").clicked() {
        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
            action = Some(EntryContextAction {
                entry_name: entry.name.clone(),
                kind: EntryAction::ExtractTo(dir),
            });
        }
        ui.close_menu();
    }
    if ui.button("Test entry").clicked() {
        action = Some(EntryContextAction {
            entry_name: entry.name.clone(),
            kind: EntryAction::TestEntry,
        });
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Copy path").clicked() {
        ui.ctx().copy_text(entry.name.clone());
        action = Some(EntryContextAction {
            entry_name: entry.name.clone(),
            kind: EntryAction::CopyPath,
        });
        ui.close_menu();
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[allow(deprecated)]
    fn sample_entry() -> OpenEntry {
        OpenEntry {
            name: "src/main.rs".to_string(),
            size: 4096,
            encrypted: false,
        }
    }

    #[test]
    #[allow(deprecated)]
    fn entry_action_extract_here_serializes() {
        let action = EntryContextAction {
            entry_name: "a.bin".to_string(),
            kind: EntryAction::ExtractHere(PathBuf::from("/tmp/out")),
        };
        // Display: human-readable form, useful for log lines and status.
        assert_eq!(action.kind.to_string(), "ExtractHere(/tmp/out)");
        // Debug: must include the variant tag and the destination so a
        // panic message tells the operator which branch fired.
        let dbg = format!("{:?}", action.kind);
        assert!(dbg.contains("ExtractHere"), "Debug: {dbg}");
        assert!(dbg.contains("/tmp/out"), "Debug: {dbg}");
        // The entry name is preserved across Display/Debug.
        assert_eq!(action.entry_name, "a.bin");
    }

    #[test]
    #[allow(deprecated)]
    fn entry_action_copy_path_has_name() {
        let entry = sample_entry();
        let action = EntryContextAction {
            entry_name: entry.name.clone(),
            kind: EntryAction::CopyPath,
        };
        assert_eq!(action.entry_name, "src/main.rs");
        // The CopyPath variant carries no path payload, so Display is just
        // the tag.
        assert_eq!(action.kind.to_string(), "CopyPath");
    }

    #[test]
    #[allow(deprecated)]
    fn entry_action_test_entry_has_no_payload() {
        let action = EntryContextAction {
            entry_name: "deep/path/file.txt".to_string(),
            kind: EntryAction::TestEntry,
        };
        assert_eq!(action.kind.to_string(), "TestEntry");
        assert_eq!(action.entry_name, "deep/path/file.txt");
    }

    #[test]
    #[allow(deprecated)]
    fn entry_action_extract_to_keeps_distinct_variant() {
        let a = EntryAction::ExtractTo(PathBuf::from("/var/tmp"));
        let b = EntryAction::ExtractHere(PathBuf::from("/var/tmp"));
        // Same path, different variant: equality must reflect the variant.
        assert_ne!(a, b);
        assert_eq!(a.to_string(), "ExtractTo(/var/tmp)");
    }
}
