//! SupaZip GUI — eframe/egui desktop front-end.
//!
//! The headless state machine and tests live in `lib.rs`; this file is a
//! thin shim that boots a tokio runtime (so the GUI can use `signal` and
//! `rt-multi-thread` features) and hands an `App` to eframe.

use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use supazip_core::traits::ProgressState;
use supazip_core::{formats, Limits};

use supazip_gui::context_menu::{EntryAction, EntryContextAction};
use supazip_gui::icons::{ToolbarButton, ToolbarIcon};
use supazip_gui::{
    dialogs, dnd, theme, AppController, EngineEvent, ExtractRequest, OpenEntry, PasswordSubmission,
    PasswordTarget, RecentEntry,
};

// ---------------------------------------------------------------------------
// App: thin eframe wrapper around AppController.
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct App {
    ctrl: AppController,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecentPopupAction {
    Open(PathBuf),
    Clear,
}

struct RecentPopupOutput {
    action: Option<RecentPopupAction>,
    #[cfg(test)]
    open_rects: Vec<(egui::Rect, egui::LayerId)>,
    #[cfg(test)]
    clear_rect: Option<(egui::Rect, egui::LayerId)>,
}

fn show_recent_popup_contents(ui: &mut egui::Ui, recent: &[RecentEntry]) -> RecentPopupOutput {
    ui.set_min_width(320.0);
    #[cfg(test)]
    let mut open_rects = Vec::with_capacity(recent.len());
    #[cfg(test)]
    let mut clear_rect = None;

    if recent.is_empty() {
        ui.label("(no recent files)");
        return RecentPopupOutput {
            action: None,
            #[cfg(test)]
            open_rects,
            #[cfg(test)]
            clear_rect,
        };
    }

    let mut action = None;
    for entry in recent {
        let response = ui.button(entry.path.display().to_string());
        #[cfg(test)]
        open_rects.push((response.rect, response.layer_id));
        if response.clicked() {
            ui.close();
            action = Some(RecentPopupAction::Open(entry.path.clone()));
            break;
        }
    }
    if action.is_none() {
        ui.separator();
        let response = ui.button("Clear recent");
        #[cfg(test)]
        {
            clear_rect = Some((response.rect, response.layer_id));
        }
        if response.clicked() {
            ui.close();
            action = Some(RecentPopupAction::Clear);
        }
    }
    RecentPopupOutput {
        action,
        #[cfg(test)]
        open_rects,
        #[cfg(test)]
        clear_rect,
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // eframe 0.34: `update` is deprecated; apply tokens before paint.
        theme::apply(ctx, self.ctrl.state().settings.theme);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Re-apply in `ui` so a theme change from Settings takes effect
        // this frame even if `logic` was skipped by a host.
        theme::apply(ui.ctx(), self.ctrl.state().settings.theme);

        // Drain engine events first so the UI reflects the latest state.
        self.ctrl.drain();

        // Drag-and-drop: forward dropped paths to the controller before
        // drawing the panels. The returned list carries the paths that
        // were actually opened; we spawn one worker per path so the
        // standard open pipeline handles it.
        let opened = dnd::handle_dropped_files(ui.ctx(), &mut self.ctrl);
        for path in opened {
            self.spawn_reserved_list_worker(path, None);
        }

        // Menu bar — renders the native-style bar and returns the
        // actions the user triggered this frame (keyboard + clicks).
        let menu_actions = supazip_gui::menubar::show_menu_bar(ui, &mut self.ctrl);
        for action in menu_actions {
            self.dispatch_menu_action(action);
        }

        // Visual hint: a full-window overlay while the cursor is
        // carrying a file over the window. Rendered after the panels
        // so it sits on top of them.
        let is_hovering_drop = ui
            .ctx()
            .data(|d| d.get_temp::<bool>(dnd::hovering_id()).unwrap_or(false));
        if is_hovering_drop {
            dnd::render_drop_overlay(ui.ctx());
        }

        egui::Panel::top("toolbar")
            .frame(theme::chrome_frame(ui.ctx()))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    let operation_blocked = !self.ctrl.can_start_operation();
                    if ui
                        .add_enabled(
                            !operation_blocked,
                            ToolbarButton::new(ToolbarIcon::Open, "Open…"),
                        )
                        .clicked()
                    {
                        self.open_dialog();
                    }
                    // WS-D: "Recent" dropdown — 10 most-recent paths.
                    // Disabled when the persisted list is empty. Text label
                    // stays next to the monoline icon (a11y).
                    let recent = self.ctrl.state().recent.clone();
                    let recent_label = if recent.is_empty() {
                        "Recent".to_string()
                    } else {
                        format!("Recent ({})", recent.len())
                    };
                    let recent_btn = ui.add_enabled(
                        !operation_blocked && !recent.is_empty(),
                        ToolbarButton::new(ToolbarIcon::Recent, recent_label),
                    );
                    let recent_action = egui::Popup::from_toggle_button_response(&recent_btn)
                        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                        .show(|ui| show_recent_popup_contents(ui, &recent))
                        .and_then(|response| response.inner.action);
                    match recent_action {
                        Some(RecentPopupAction::Open(path)) if path.exists() => {
                            self.spawn_list(path);
                        }
                        Some(RecentPopupAction::Open(path)) => {
                            self.ctrl
                                .apply(EngineEvent::Error(format!("missing: {}", path.display())));
                        }
                        Some(RecentPopupAction::Clear) => self.clear_recent_clicked(),
                        None => {}
                    }
                    let can_extract =
                        self.ctrl.state().open_archive.is_some() && !operation_blocked;
                    if ui
                        .add_enabled(
                            can_extract,
                            ToolbarButton::new(ToolbarIcon::Extract, "Extract"),
                        )
                        .clicked()
                    {
                        self.extract_clicked();
                    }
                    if ui
                        .add_enabled(
                            !operation_blocked,
                            ToolbarButton::new(ToolbarIcon::Create, "Create…"),
                        )
                        .clicked()
                    {
                        self.create_clicked();
                    }
                    let can_test = self.ctrl.state().open_archive.is_some() && !operation_blocked;
                    if ui
                        .add_enabled(can_test, ToolbarButton::new(ToolbarIcon::Test, "Test"))
                        .clicked()
                    {
                        self.test_clicked();
                    }
                    if self.ctrl.progress().is_some()
                        && ui
                            .add(ToolbarButton::new(ToolbarIcon::Cancel, "Cancel"))
                            .clicked()
                    {
                        self.ctrl.cancel();
                    }
                });
            });

        egui::Panel::bottom("statusbar")
            .frame(theme::chrome_frame(ui.ctx()))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.ctrl.state().busy {
                        ui.spinner();
                    }
                    ui.label(&self.ctrl.state().status);
                });
            });

        let mut pending_entry_action = None;
        let content_enabled = self.ctrl.can_start_operation();
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_enabled_ui(content_enabled, |ui| {
                match &self.ctrl.state().open_archive {
                    Some(oa) => {
                        ui.heading(format!("{} ({})", oa.path.display(), oa.backend_name));
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("entries")
                                .num_columns(4)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("#");
                                    ui.label("Name");
                                    ui.label("Size");
                                    ui.label("Encrypted");
                                    ui.end_row();
                                    for (i, e) in oa.entries.iter().enumerate() {
                                        ui.monospace(i.to_string());
                                        let name_response = ui.add(
                                            egui::Label::new(&e.name)
                                                .selectable(false)
                                                .sense(egui::Sense::click()),
                                        );
                                        name_response.context_menu(|ui| {
                                            if let Some(action) =
                                                supazip_gui::context_menu::show_entry_context_menu(
                                                    ui, e,
                                                )
                                            {
                                                pending_entry_action = Some(action);
                                            }
                                        });
                                        ui.monospace(format!("{} B", e.size));
                                        ui.label(if e.encrypted { "yes" } else { "-" });
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                    None => {
                        ui.vertical_centered(|ui| {
                            ui.heading("SupaZip");
                            ui.label("Open an archive (7z or ZIP) to view its contents.");
                        });
                    }
                }
            });
        });
        if let Some(action) = pending_entry_action {
            self.dispatch_entry_action(action);
        }

        // WS-E: render the modal password dialog. The dialog state lives
        // on the controller; the GUI owns the egui::Context the modal
        // needs. If the user submits a value we re-dispatch the operation
        // that opened the dialog.
        if let Some(submission) =
            dialogs::show_password_dialog(ui.ctx(), self.ctrl.password_dialog())
        {
            self.password_dialog_submitted(submission);
        }

        if let Some(progress) = self.ctrl.progress() {
            supazip_gui::show_progress_modal(ui.ctx(), progress);
        }

        // WS-G: render the settings window when the user opened it from
        // the File menu.
        let mut show_settings = self.ctrl.state().show_settings;
        if show_settings {
            supazip_gui::settings_window::show_settings_window(
                ui.ctx(),
                &mut self.ctrl.state_mut().settings,
                &mut show_settings,
            );
            self.ctrl.state_mut().show_settings = show_settings;
        }
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        visuals.panel_fill.to_normalized_gamma_f32()
    }
}

impl App {
    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("archives", &["zip", "7z"])
            .pick_file()
        {
            self.spawn_list(path);
        }
    }

    /// Dispatch a menu-bar action through the controller, then handle
    /// the GUI-side effects that the headless controller cannot perform
    /// (file pickers, viewport commands).
    fn dispatch_menu_action(&mut self, action: supazip_gui::MenuAction) {
        use supazip_gui::{MenuAction, MenuActionOutcome};
        match self.ctrl.dispatch_menu_action(action) {
            MenuActionOutcome::Done | MenuActionOutcome::Noop => {}
            MenuActionOutcome::Gui => match action {
                MenuAction::Open => self.open_dialog(),
                MenuAction::Extract => self.extract_clicked(),
                MenuAction::Create => self.create_clicked(),
                MenuAction::Test => self.test_clicked(),
                MenuAction::Quit => {
                    // TODO: eframe 0.34 quit — for now just request close.
                    // ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                _ => {}
            },
        }
    }

    fn extract_clicked(&mut self) {
        let Some(oa) = self.ctrl.state().open_archive.clone() else {
            return;
        };
        let out = match rfd::FileDialog::new().pick_folder() {
            Some(f) => f,
            None => return,
        };
        self.spawn_extract(oa.path, out);
    }

    fn create_clicked(&mut self) {
        let target = match rfd::FileDialog::new()
            .add_filter("archive", &["zip", "7z"])
            .set_file_name("archive.zip")
            .save_file()
        {
            Some(p) => p,
            None => return,
        };
        let inputs = match rfd::FileDialog::new().pick_files() {
            Some(f) if !f.is_empty() => f,
            _ => {
                self.ctrl.apply(EngineEvent::Error(
                    "create cancelled (no input files)".into(),
                ));
                return;
            }
        };
        self.spawn_create(target, inputs);
    }

    fn test_clicked(&mut self) {
        let Some(oa) = self.ctrl.state().open_archive.clone() else {
            return;
        };
        self.spawn_test(oa.path);
    }

    /// Empty the recent-files list and persist. Best-effort: a write
    /// failure shows up as a non-blocking `EngineEvent::Error` so the
    /// user knows the clear did not stick on disk.
    fn clear_recent_clicked(&mut self) {
        if let Err(e) = self.ctrl.clear_recent() {
            self.ctrl
                .apply(EngineEvent::Error(format!("clear recent: {e}")));
        }
    }

    fn spawn_list(&mut self, path: PathBuf) {
        self.spawn_list_with_password(path, None);
    }

    fn spawn_list_with_password(&mut self, path: PathBuf, password: Option<String>) {
        let Some(progress) = self
            .ctrl
            .begin_progress_operation(format!("opening {}…", path.display()))
        else {
            return;
        };
        self.spawn_list_worker(path, password, progress);
    }

    fn spawn_reserved_list_worker(&mut self, path: PathBuf, password: Option<String>) {
        let Some(progress) = self.ctrl.attach_progress_to_reserved_operation() else {
            self.ctrl.apply(EngineEvent::Error(
                "internal error: open operation lost its worker reservation".into(),
            ));
            return;
        };
        self.spawn_list_worker(path, password, progress);
    }

    fn spawn_list_worker(
        &mut self,
        path: PathBuf,
        password: Option<String>,
        progress: Arc<ProgressState>,
    ) {
        let tx = self.ctrl.engine_sender();
        std::thread::spawn(move || {
            let ev = run_list_blocking(&path, password.as_deref(), &progress);
            let _ = tx.send(ev);
        });
    }

    fn spawn_extract(&mut self, archive: PathBuf, out: PathBuf) {
        self.spawn_extract_with_password(archive, out, None);
    }

    fn spawn_extract_with_password(
        &mut self,
        archive: PathBuf,
        out: PathBuf,
        password: Option<String>,
    ) {
        let tx = self.ctrl.engine_sender();
        let Some(progress) = self.ctrl.begin_progress_operation(format!(
            "extracting {} to {}…",
            archive.display(),
            out.display()
        )) else {
            return;
        };
        let limits = *self.ctrl.limits();
        std::thread::spawn(move || {
            let ev = run_extract_blocking(&archive, &out, password.as_deref(), &progress, &limits);
            let _ = tx.send(ev);
        });
    }

    fn spawn_create(&mut self, target: PathBuf, inputs: Vec<PathBuf>) {
        self.spawn_create_with_password(target, inputs, None);
    }

    fn spawn_create_with_password(
        &mut self,
        target: PathBuf,
        inputs: Vec<PathBuf>,
        password: Option<String>,
    ) {
        let tx = self.ctrl.engine_sender();
        let Some(progress) = self.ctrl.begin_progress_operation(format!(
            "creating {} ({} entries)…",
            target.display(),
            inputs.len()
        )) else {
            return;
        };
        let limits = *self.ctrl.limits();
        std::thread::spawn(move || {
            let ev = run_create_blocking(&target, &inputs, password.as_deref(), &progress, &limits);
            let _ = tx.send(ev);
        });
    }

    fn spawn_test(&mut self, archive: PathBuf) {
        self.spawn_test_with_password(archive, None);
    }

    fn spawn_test_with_password(&mut self, archive: PathBuf, password: Option<String>) {
        let tx = self.ctrl.engine_sender();
        let Some(progress) = self
            .ctrl
            .begin_progress_operation(format!("testing {}…", archive.display()))
        else {
            return;
        };
        let limits = *self.ctrl.limits();
        std::thread::spawn(move || {
            let ev = run_test_blocking(&archive, password.as_deref(), &progress, &limits);
            let _ = tx.send(ev);
        });
    }

    /// Handle a context-menu click on an entry. The controller records
    /// the intent in state (busy / status) for every variant; the GUI
    /// additionally spawns a worker for the extract variants.
    fn dispatch_entry_action(&mut self, action: EntryContextAction) {
        if !self.ctrl.can_start_operation() {
            return;
        }
        match &action.kind {
            EntryAction::ExtractHere(dest) | EntryAction::ExtractTo(dest) => {
                let Some(archive) = self
                    .ctrl
                    .state()
                    .open_archive
                    .as_ref()
                    .map(|archive| archive.path.clone())
                else {
                    return;
                };
                let entries = vec![action.entry_name.clone()];
                let dest_for_spawn = dest.clone();

                // Let the controller decide whether to open the password
                // dialog (WS-E: encrypted archive + ExtractTo) or to
                // record the extract intent. If the dialog was opened, we
                // do NOT spawn a worker yet — the user has to submit a
                // password first, which `password_dialog_submitted` handles.
                self.ctrl.dispatch_entry_action(action);
                if self.ctrl.password_dialog_ref().visible {
                    return;
                }

                self.spawn_extract_entries(archive, dest_for_spawn, entries);
            }
            EntryAction::TestEntry | EntryAction::CopyPath => {
                // No worker thread needed: status update only. The
                // clipboard write happened inside `show_entry_context_menu`.
                self.ctrl.dispatch_entry_action(action);
            }
        }
    }

    fn spawn_extract_entries(&mut self, archive: PathBuf, out: PathBuf, entries: Vec<String>) {
        let Some(progress) = self.ctrl.attach_progress_to_reserved_operation() else {
            self.ctrl.apply(EngineEvent::Error(
                "internal error: extract operation lost its worker reservation".into(),
            ));
            return;
        };
        self.spawn_extract_entries_worker(archive, out, entries, None, progress);
    }

    fn spawn_extract_entries_with_password(
        &mut self,
        archive: PathBuf,
        out: PathBuf,
        entries: Vec<String>,
        password: Option<String>,
    ) {
        let Some(progress) = self.ctrl.begin_progress_operation(format!(
            "extracting {} entr{} from {} to {}…",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" },
            archive.display(),
            out.display()
        )) else {
            return;
        };
        self.spawn_extract_entries_worker(archive, out, entries, password, progress);
    }

    fn spawn_extract_entries_worker(
        &mut self,
        archive: PathBuf,
        out: PathBuf,
        entries: Vec<String>,
        password: Option<String>,
        progress: Arc<ProgressState>,
    ) {
        let tx = self.ctrl.engine_sender();
        let limits = *self.ctrl.limits();
        // The EngineEvent::Extract was applied by the caller; do not
        // double-apply it here.
        std::thread::spawn(move || {
            let ev = run_extract_entries_blocking(
                &archive,
                &out,
                &entries,
                password.as_deref(),
                &progress,
                &limits,
            );
            let _ = tx.send(ev);
        });
    }

    /// Re-dispatch the operation whose password prompt the user just
    /// answered. The controller stores the target (Open / Extract /
    /// Create / Test); the GUI is responsible for re-spawning the worker
    /// with the entered password. For Extract, the cached intent on the
    /// `Action` is honoured by spawning the entry-list worker again.
    fn password_dialog_submitted(&mut self, submission: PasswordSubmission) {
        match password_retry_command(submission) {
            PasswordRetryCommand::Open { path, password } => {
                self.spawn_list_with_password(path, Some(password));
            }
            PasswordRetryCommand::Extract { request, password } => {
                self.spawn_extract_entries_with_password(
                    request.archive,
                    request.destination,
                    request.entries,
                    Some(password),
                );
            }
            PasswordRetryCommand::CreateUnsupported { path } => {
                // Create-with-password needs the original input list. The
                // controller does not currently cache it; the caller is
                // expected to re-trigger the create flow. As a fallback we
                // surface a clear status message.
                self.ctrl.apply(EngineEvent::Error(format!(
                    "encrypted create: re-pick inputs for {}",
                    path.display()
                )));
            }
            PasswordRetryCommand::Test { path, password } => {
                self.spawn_test_with_password(path, Some(password));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Worker-thread bodies. Plain functions so the App impl above stays small.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum PasswordRetryCommand {
    Open {
        path: PathBuf,
        password: String,
    },
    Extract {
        request: ExtractRequest,
        password: String,
    },
    CreateUnsupported {
        path: PathBuf,
    },
    Test {
        path: PathBuf,
        password: String,
    },
}

fn password_retry_command(submission: PasswordSubmission) -> PasswordRetryCommand {
    let PasswordSubmission { target, password } = submission;
    match target {
        PasswordTarget::Open(path) => PasswordRetryCommand::Open { path, password },
        PasswordTarget::Extract(request) => PasswordRetryCommand::Extract { request, password },
        PasswordTarget::Create(path) => PasswordRetryCommand::CreateUnsupported { path },
        PasswordTarget::Test(path) => PasswordRetryCommand::Test { path, password },
    }
}

/// Translate an `ArchiverError` from the engine into a GUI event. Password
/// errors are mapped to `EngineEvent::PasswordRequired` so the controller
/// can open the modal dialog and the user can retry with a real key.
fn map_engine_error(
    target: PasswordTarget,
    prefix: &str,
    e: supazip_core::ArchiverError,
) -> EngineEvent {
    use supazip_core::ArchiverError as A;
    match e {
        A::PasswordRequired | A::WrongPassword => EngineEvent::PasswordRequired { target },
        other => EngineEvent::Error(format!("{prefix}: {other}")),
    }
}

fn map_extract_error(
    archive: &std::path::Path,
    out: &std::path::Path,
    entries: &[String],
    e: supazip_core::ArchiverError,
) -> EngineEvent {
    map_engine_error(
        PasswordTarget::Extract(ExtractRequest {
            archive: archive.to_path_buf(),
            destination: out.to_path_buf(),
            entries: entries.to_vec(),
        }),
        "extract",
        e,
    )
}

struct CancellableReader<R> {
    inner: R,
    progress: Arc<ProgressState>,
}

impl<R> CancellableReader<R> {
    fn new(inner: R, progress: Arc<ProgressState>) -> Self {
        Self { inner, progress }
    }
}

impl<R: Read> Read for CancellableReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.progress.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "archive listing cancelled",
            ));
        }
        self.inner.read(buf)
    }
}

fn run_list_blocking(
    path: &std::path::Path,
    password: Option<&str>,
    progress: &Arc<ProgressState>,
) -> EngineEvent {
    progress.set_message("Reading archive index");
    if progress.is_cancelled() {
        return EngineEvent::Error("list: cancelled".into());
    }
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("open: {e}")),
    };
    let result = backend.list(
        Box::new(CancellableReader::new(
            std::io::BufReader::new(file),
            progress.clone(),
        )),
        password,
        &Limits::default(),
    );
    if progress.is_cancelled() {
        return EngineEvent::Error("list: cancelled".into());
    }
    match result {
        Ok(list) => EngineEvent::Listed {
            path: path.to_path_buf(),
            backend_name: backend.name(),
            entries: list.into_iter().map(OpenEntry::from).collect(),
        },
        Err(e) => map_engine_error(PasswordTarget::Open(path.to_path_buf()), "list", e),
    }
}

fn run_extract_blocking(
    archive: &std::path::Path,
    out: &std::path::Path,
    password: Option<&str>,
    progress: &Arc<ProgressState>,
    limits: &Limits,
) -> EngineEvent {
    run_extract_entries_blocking(archive, out, &[], password, progress, limits)
}

/// Extract a specific list of entry names. An empty `entries` slice is
/// the "extract all" sentinel used by the toolbar; the context menu always
/// passes a non-empty list. Entry names that do not exist in the archive
/// are silently skipped by the core — the controller does not currently
/// differentiate that from a clean extract.
fn run_extract_entries_blocking(
    archive: &std::path::Path,
    out: &std::path::Path,
    entries: &[String],
    password: Option<&str>,
    progress: &Arc<ProgressState>,
    limits: &Limits,
) -> EngineEvent {
    let ext = archive.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::open(archive) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("open: {e}")),
    };
    let entry_refs: Vec<&str> = entries.iter().map(String::as_str).collect();
    match backend.extract(
        Box::new(std::io::BufReader::new(file)),
        out,
        &entry_refs,
        password,
        progress,
        limits,
    ) {
        Ok(()) => EngineEvent::Done(format!("extracted to {}", out.display())),
        Err(e) => map_extract_error(archive, out, entries, e),
    }
}

fn run_create_blocking(
    target: &std::path::Path,
    inputs: &[PathBuf],
    password: Option<&str>,
    progress: &Arc<ProgressState>,
    limits: &Limits,
) -> EngineEvent {
    let ext = target.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::create(target) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("create target: {e}")),
    };
    let writer: Box<dyn supazip_core::traits::WriteSeek> = Box::new(std::io::BufWriter::new(file));
    let opts = supazip_core::traits::CreateOptions {
        compression_method: "deflate".to_string(),
        compression_level: None,
        ..supazip_core::traits::CreateOptions::default()
    };
    let cb: &dyn supazip_core::traits::ProgressCallback = progress;
    match backend.create(writer, inputs, &opts, password, cb, limits) {
        Ok(()) => EngineEvent::Done(format!("created {}", target.display())),
        Err(e) => map_engine_error(PasswordTarget::Create(target.to_path_buf()), "create", e),
    }
}

fn run_test_blocking(
    archive: &std::path::Path,
    password: Option<&str>,
    progress: &Arc<ProgressState>,
    limits: &Limits,
) -> EngineEvent {
    let ext = archive.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::open(archive) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("open: {e}")),
    };
    let cb: &dyn supazip_core::traits::ProgressCallback = progress;
    match backend.test(
        Box::new(std::io::BufReader::new(file)),
        password,
        cb,
        limits,
    ) {
        Ok(true) => EngineEvent::Done(format!("OK: {}", archive.display())),
        Ok(false) => EngineEvent::Error("test: integrity check failed".into()),
        Err(e) => map_engine_error(PasswordTarget::Test(archive.to_path_buf()), "test", e),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    // The GUI does not need a multi-threaded tokio runtime today — the
    // engine calls are synchronous — but a runtime must exist for the
    // `tokio` features we pull in (e.g. signal). We build a minimal one.
    let _runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let native_options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "SupaZip",
        native_options,
        Box::new(|cc| {
            let app = App::default();
            theme::install_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx, app.ctrl.state().settings.theme);
            Ok(Box::new(app))
        }),
    ) {
        eprintln!("eframe failed: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn popup_frame(
        ctx: &egui::Context,
        input: egui::RawInput,
        open: &mut bool,
        recent: &[RecentEntry],
    ) -> RecentPopupOutput {
        let mut output = None;
        let _ = ctx.run_ui(input, |ui| {
            let anchor = ui.allocate_response(egui::vec2(80.0, 24.0), egui::Sense::hover());
            output = egui::Popup::from_response(&anchor)
                .open_bool(open)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| show_recent_popup_contents(ui, recent))
                .map(|response| response.inner);
        });
        let mut output = output.expect("popup should be open");
        for (rect, layer_id) in &mut output.open_rects {
            if let Some(transform) = ctx.layer_transform_to_global(*layer_id) {
                *rect = transform * *rect;
            }
        }
        if let Some((rect, layer_id)) = &mut output.clear_rect {
            if let Some(transform) = ctx.layer_transform_to_global(*layer_id) {
                *rect = transform * *rect;
            }
        }
        output
    }

    fn pointer_input(pos: egui::Pos2, pressed: bool) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events: vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        }
    }

    fn empty_input() -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        }
    }

    fn hover_input(pos: egui::Pos2) -> egui::RawInput {
        let mut input = empty_input();
        input.events.push(egui::Event::PointerMoved(pos));
        input
    }

    fn one_recent() -> Vec<RecentEntry> {
        vec![RecentEntry::now("/archives/one.zip".into())]
    }

    fn assert_extract_password_retry_round_trip(
        archive: &str,
        destination: &str,
        entries: Vec<String>,
    ) {
        let event = map_extract_error(
            std::path::Path::new(archive),
            std::path::Path::new(destination),
            &entries,
            supazip_core::ArchiverError::PasswordRequired,
        );
        let EngineEvent::PasswordRequired { target } = event else {
            panic!("password error must request a retry");
        };

        let command = password_retry_command(PasswordSubmission {
            target,
            password: "secret".into(),
        });
        assert_eq!(
            command,
            PasswordRetryCommand::Extract {
                request: ExtractRequest {
                    archive: archive.into(),
                    destination: destination.into(),
                    entries,
                },
                password: "secret".into(),
            }
        );
    }

    #[test]
    fn toolbar_extract_all_password_retry_keeps_destination() {
        assert_extract_password_retry_round_trip(
            "/archives/locked.7z",
            "/chosen/toolbar",
            Vec::new(),
        );
    }

    #[test]
    fn context_extract_here_password_retry_keeps_entry_and_destination() {
        assert_extract_password_retry_round_trip(
            "/archives/locked.7z",
            "/archives",
            vec!["folder/one.txt".into()],
        );
    }

    #[test]
    fn context_extract_to_password_retry_keeps_entry_and_destination() {
        assert_extract_password_retry_round_trip(
            "/archives/locked.7z",
            "/chosen/context",
            vec!["folder/one.txt".into()],
        );
    }

    #[test]
    fn encrypted_test_retries_test_instead_of_open() {
        let target = PasswordTarget::Test("/archives/locked.7z".into());
        let event = map_engine_error(
            target.clone(),
            "test",
            supazip_core::ArchiverError::WrongPassword,
        );
        let EngineEvent::PasswordRequired { target } = event else {
            panic!("password error must request a retry");
        };
        assert_eq!(
            password_retry_command(PasswordSubmission {
                target,
                password: "secret".into(),
            }),
            PasswordRetryCommand::Test {
                path: "/archives/locked.7z".into(),
                password: "secret".into(),
            }
        );
    }

    #[test]
    fn cancelled_list_reader_interrupts_backend_reads() {
        let progress = Arc::new(ProgressState::new());
        progress.cancel();
        let mut reader =
            CancellableReader::new(std::io::Cursor::new(b"archive bytes".to_vec()), progress);
        let mut byte = [0_u8; 1];

        let error = reader.read(&mut byte).expect_err("read must be cancelled");
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    }

    #[test]
    fn cancelled_list_operation_returns_terminal_error_instead_of_listed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let archive = temp.path().join("cancelled.zip");
        std::fs::write(&archive, b"not reached because cancellation is pre-set")
            .expect("write archive fixture");
        let progress = Arc::new(ProgressState::new());
        progress.cancel();

        let event = run_list_blocking(&archive, None, &progress);

        let EngineEvent::Error(message) = event else {
            panic!("cancelled listing must terminate with an error event");
        };
        assert_eq!(message, "list: cancelled");
    }

    #[test]
    fn selecting_recent_entry_closes_popup() {
        let ctx = egui::Context::default();
        let recent = one_recent();
        let mut open = true;
        let first = popup_frame(&ctx, empty_input(), &mut open, &recent);
        let click = first.open_rects[0].0.center();

        let _hovered = popup_frame(&ctx, hover_input(click), &mut open, &recent);
        let pressed = popup_frame(&ctx, pointer_input(click, true), &mut open, &recent);
        assert!(pressed.open_rects[0].0.contains(click));
        let second = popup_frame(&ctx, pointer_input(click, false), &mut open, &recent);
        assert!(second.open_rects[0].0.contains(click));

        assert_eq!(
            second.action,
            Some(RecentPopupAction::Open(recent[0].path.clone()))
        );
        assert!(!open, "selecting an entry should dismiss the popup");
    }

    #[test]
    fn clearing_recent_entries_closes_popup() {
        let ctx = egui::Context::default();
        let recent = one_recent();
        let mut open = true;
        let first = popup_frame(&ctx, empty_input(), &mut open, &recent);
        let click = first.clear_rect.expect("clear button").0.center();

        let _hovered = popup_frame(&ctx, hover_input(click), &mut open, &recent);
        let pressed = popup_frame(&ctx, pointer_input(click, true), &mut open, &recent);
        assert!(pressed.clear_rect.expect("clear button").0.contains(click));
        let second = popup_frame(&ctx, pointer_input(click, false), &mut open, &recent);
        assert!(second.clear_rect.expect("clear button").0.contains(click));

        assert_eq!(second.action, Some(RecentPopupAction::Clear));
        assert!(!open, "clearing entries should dismiss the popup");
    }
}
