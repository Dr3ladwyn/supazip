//! Modal dialogs for the GUI.
//!
//! WS-E: password dialog. Used by `extract` and `create` flows when an
//! operation needs a password (encrypted archive). The dialog is a
//! single-`egui::Window` with a `TextEdit` in password mode, a show/hide
//! checkbox, and an OK / Cancel button row. Enter submits; Cancel clears
//! the buffer and closes.

use std::path::PathBuf;

use eframe::egui;

/// What triggered the password dialog. The dialog carries this so the
/// caller knows which worker to dispatch when the user submits a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordTarget {
    /// `Open` / list operation on a header-encrypted archive.
    Open(PathBuf),
    /// Extract from an encrypted archive.
    Extract(PathBuf),
    /// Create a new encrypted archive.
    Create(PathBuf),
}

impl PasswordTarget {
    /// Borrow the path the dialog is targeting, regardless of variant.
    pub fn path(&self) -> &std::path::Path {
        match self {
            PasswordTarget::Open(p) | PasswordTarget::Extract(p) | PasswordTarget::Create(p) => p,
        }
    }
}

/// Pure state for the password dialog. The GUI owns one of these; it is
/// hidden by default and shown by setting `visible = true` and `target`.
#[derive(Debug, Default, Clone)]
pub struct PasswordDialogState {
    pub visible: bool,
    pub password: String,
    pub show_password: bool,
    pub target: Option<PasswordTarget>,
    pub error: Option<String>,
}

impl PasswordDialogState {
    /// Open the dialog for the given target. Clears any previous input and
    /// error so the user always starts from a clean slate.
    pub fn open(&mut self, target: PasswordTarget) {
        self.password.clear();
        self.error = None;
        self.show_password = false;
        self.target = Some(target);
        self.visible = true;
    }

    /// Close the dialog and wipe the password buffer. Call after the
    /// caller has consumed the password.
    pub fn close(&mut self) {
        self.password.clear();
        self.error = None;
        self.target = None;
        self.visible = false;
    }
}

/// Render the password dialog. Returns `Some(password)` on OK and `None`
/// otherwise (including the not-visible case). The dialog closes itself
/// after returning — callers should treat the return value as a one-shot.
pub fn show_password_dialog(
    ctx: &egui::Context,
    state: &mut PasswordDialogState,
) -> Option<String> {
    if !state.visible {
        return None;
    }
    let mut submitted: Option<String> = None;
    egui::Window::new("Password required")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            if let Some(target) = &state.target {
                ui.label(format!("Archive: {}", target.path().display()));
            }
            let response = ui.add(
                egui::TextEdit::singleline(&mut state.password)
                    .password(!state.show_password)
                    .hint_text("Password")
                    .desired_width(280.0),
            );
            if state.visible && !response.has_focus() {
                response.request_focus();
            }
            ui.checkbox(&mut state.show_password, "Show password");
            if let Some(err) = &state.error {
                ui.colored_label(egui::Color32::RED, err);
            }
            ui.horizontal(|ui| {
                let enter_pressed =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("OK").clicked() || enter_pressed {
                    submitted = Some(state.password.clone());
                    state.close();
                }
                if ui.button("Cancel").clicked() {
                    state.close();
                }
            });
        });
    submitted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_dialog_state_default_invisible() {
        let s = PasswordDialogState::default();
        assert!(!s.visible);
        assert!(s.password.is_empty());
        assert!(!s.show_password);
        assert!(s.target.is_none());
        assert!(s.error.is_none());
    }

    #[test]
    fn password_target_holds_path() {
        let p = std::path::PathBuf::from("/tmp/secret.7z");
        let t = PasswordTarget::Extract(p.clone());
        assert_eq!(t.path(), p.as_path());
        assert_eq!(PasswordTarget::Open(p.clone()).path(), p.as_path());
        assert_eq!(PasswordTarget::Create(p.clone()).path(), p.as_path());
    }

    #[test]
    fn open_clears_previous_state() {
        let mut s = PasswordDialogState::default();
        s.password = "stale".into();
        s.error = Some("old error".into());
        s.show_password = true;
        s.open(PasswordTarget::Extract(std::path::PathBuf::from("/a.7z")));
        assert!(s.visible);
        assert!(s.password.is_empty());
        assert!(s.error.is_none());
        assert!(!s.show_password);
        assert_eq!(
            s.target.as_ref().unwrap().path(),
            std::path::Path::new("/a.7z")
        );
    }

    #[test]
    fn close_wipes_password_and_clears_target() {
        let mut s = PasswordDialogState::default();
        s.password = "hunter2".into();
        s.error = Some("wrong".into());
        s.target = Some(PasswordTarget::Create(std::path::PathBuf::from("/x.zip")));
        s.visible = true;
        s.close();
        assert!(!s.visible);
        assert!(s.password.is_empty());
        assert!(s.error.is_none());
        assert!(s.target.is_none());
    }
}
