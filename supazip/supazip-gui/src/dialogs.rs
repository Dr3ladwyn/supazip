//! Modal dialogs for the GUI.
//!
//! WS-E: password dialog. Used by `extract` and `create` flows when an
//! operation needs a password (encrypted archive). The dialog is a
//! single-`egui::Window` with a `TextEdit` in password mode, a show/hide
//! checkbox, and an OK / Cancel button row. Enter submits; Cancel clears
//! the buffer and closes.

use std::path::PathBuf;

use eframe::egui;

/// Complete extraction intent retained while the password dialog is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractRequest {
    pub archive: PathBuf,
    pub destination: PathBuf,
    /// Empty means "extract all", matching [`supazip_core::ArchiveFormat::extract`].
    pub entries: Vec<String>,
}

/// What triggered the password dialog. The dialog carries this so the
/// caller knows which worker to dispatch when the user submits a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordTarget {
    /// `Open` / list operation on a header-encrypted archive.
    Open(PathBuf),
    /// Extract from an encrypted archive, preserving the exact user request.
    Extract(ExtractRequest),
    /// Create a new encrypted archive.
    Create(PathBuf),
    /// Test an encrypted archive.
    Test(PathBuf),
}

impl PasswordTarget {
    /// Borrow the path the dialog is targeting, regardless of variant.
    pub fn path(&self) -> &std::path::Path {
        match self {
            PasswordTarget::Open(p) | PasswordTarget::Create(p) | PasswordTarget::Test(p) => p,
            PasswordTarget::Extract(request) => &request.archive,
        }
    }
}

/// One accepted password paired with the operation that requested it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordSubmission {
    pub target: PasswordTarget,
    pub password: String,
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

    fn take_submission(&mut self) -> Option<PasswordSubmission> {
        let target = self.target.take()?;
        let submission = PasswordSubmission {
            target,
            password: std::mem::take(&mut self.password),
        };
        self.close();
        Some(submission)
    }
}

/// Render the password dialog. Returns the password together with its retained
/// operation target on OK, or `None` otherwise. The dialog closes itself after
/// returning, so callers should treat the submission as a one-shot.
pub fn show_password_dialog(
    ctx: &egui::Context,
    state: &mut PasswordDialogState,
) -> Option<PasswordSubmission> {
    if !state.visible {
        return None;
    }
    let mut submitted = None;
    egui::Modal::new(egui::Id::new("password_dialog_modal"))
        .frame(crate::theme::dialog_frame(ctx))
        .show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.heading("Password required");
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
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            ui.horizontal(|ui| {
                let enter_pressed =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("OK").clicked() || enter_pressed {
                    submitted = state.take_submission();
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
        let t = PasswordTarget::Extract(ExtractRequest {
            archive: p.clone(),
            destination: "/tmp/out".into(),
            entries: vec!["secret.txt".into()],
        });
        assert_eq!(t.path(), p.as_path());
        assert_eq!(PasswordTarget::Open(p.clone()).path(), p.as_path());
        assert_eq!(PasswordTarget::Create(p.clone()).path(), p.as_path());
        assert_eq!(PasswordTarget::Test(p.clone()).path(), p.as_path());
    }

    #[test]
    fn open_clears_previous_state() {
        let mut s = PasswordDialogState {
            password: "stale".into(),
            show_password: true,
            error: Some("old error".into()),
            ..PasswordDialogState::default()
        };
        s.open(PasswordTarget::Extract(ExtractRequest {
            archive: "/a.7z".into(),
            destination: "/out".into(),
            entries: Vec::new(),
        }));
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
        let mut s = PasswordDialogState {
            visible: true,
            password: "hunter2".into(),
            target: Some(PasswordTarget::Create(std::path::PathBuf::from("/x.zip"))),
            error: Some("wrong".into()),
            ..PasswordDialogState::default()
        };
        s.close();
        assert!(!s.visible);
        assert!(s.password.is_empty());
        assert!(s.error.is_none());
        assert!(s.target.is_none());
    }

    #[test]
    fn submission_retains_target_before_dialog_state_is_cleared() {
        let mut state = PasswordDialogState::default();
        let request = ExtractRequest {
            archive: "/locked.7z".into(),
            destination: "/chosen/out".into(),
            entries: vec!["one.txt".into()],
        };
        state.open(PasswordTarget::Extract(request.clone()));
        state.password = "secret".into();

        let submission = state.take_submission().expect("submission");
        assert_eq!(submission.target, PasswordTarget::Extract(request));
        assert_eq!(submission.password, "secret");
        assert!(!state.visible);
        assert!(state.target.is_none());
        assert!(state.password.is_empty());
    }
}
