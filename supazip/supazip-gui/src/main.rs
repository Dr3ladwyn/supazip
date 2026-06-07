//! SupaZip GUI — eframe/egui desktop front-end.
//!
//! The headless state machine and tests live in `lib.rs`; this file is a
//! thin shim that boots a tokio runtime (so the GUI can use `signal` and
//! `rt-multi-thread` features) and hands an `App` to eframe.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use supazip_core::traits::ProgressState;
use supazip_core::{formats, Limits};

use supazip_gui::context_menu::{EntryAction, EntryContextAction};
use supazip_gui::{
    dialogs, dnd, AppController, EngineEvent, OpenEntry, PasswordOpKind, PasswordTarget,
};

// ---------------------------------------------------------------------------
// App: thin eframe wrapper around AppController.
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct App {
    ctrl: AppController,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain engine events first so the UI reflects the latest state.
        self.ctrl.drain();

        // Drag-and-drop: forward dropped paths to the controller before
        // drawing the panels. The returned list carries the paths that
        // were actually opened; we spawn one worker per path so the
        // standard open pipeline handles it.
        let opened = dnd::handle_dropped_files(ui.ctx(), &mut self.ctrl);
        for path in opened {
            self.spawn_list(path);
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

        egui::Panel::top("toolbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let busy = self.ctrl.state().busy;
                if ui.add_enabled(!busy, egui::Button::new("Open…")).clicked() {
                    self.open_dialog();
                }
                // WS-D: "Recent ▾" dropdown — 10 most-recent paths.
                // Disabled when the persisted list is empty.
                let recent = self.ctrl.state().recent.clone();
                let recent_label = if recent.is_empty() {
                    "Recent ▾".to_string()
                } else {
                    format!("Recent ▾ ({})", recent.len())
                };
                let recent_btn =
                    ui.add_enabled(!busy && !recent.is_empty(), egui::Button::new(recent_label));
                if recent_btn.clicked() {
                    ui.memory_mut(|mem| mem.toggle_popup(egui::Id::new("recent_menu")));
                }
                egui::popup::popup_below_widget(
                    ui,
                    egui::Id::new("recent_menu"),
                    &recent_btn,
                    egui::PopupCloseBehavior::CloseOnClickOutside,
                    |ui| {
                        ui.set_min_width(320.0);
                        if recent.is_empty() {
                            ui.label("(no recent files)");
                        } else {
                            // Lazy-prune missing files once when the menu
                            // opens. The pruned list is then used for the
                            // menu contents; the new state is written back
                            // through `apply(EngineEvent::Done("…"))` is
                            // overkill — we just call the helper directly.
                            for entry in &recent {
                                let label = entry.path.display().to_string();
                                if ui.button(&label).clicked() {
                                    let p = entry.path.clone();
                                    ui.memory_mut(|mem| mem.close_popup());
                                    if p.exists() {
                                        self.spawn_list(p);
                                    } else {
                                        self.ctrl.apply(EngineEvent::Error(format!(
                                            "missing: {}",
                                            p.display()
                                        )));
                                    }
                                }
                            }
                            ui.separator();
                            if ui.button("Clear recent").clicked() {
                                self.clear_recent_clicked();
                                ui.memory_mut(|mem| mem.close_popup());
                            }
                        }
                    },
                );
                let can_extract = self.ctrl.state().open_archive.is_some() && !busy;
                if ui
                    .add_enabled(can_extract, egui::Button::new("Extract"))
                    .clicked()
                {
                    self.extract_clicked();
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Create…"))
                    .clicked()
                {
                    self.create_clicked();
                }
                let can_test = self.ctrl.state().open_archive.is_some() && !busy;
                if ui
                    .add_enabled(can_test, egui::Button::new("Test"))
                    .clicked()
                {
                    self.test_clicked();
                }
                if busy && ui.button("Cancel").clicked() {
                    self.ctrl.cancel();
                }
            });
        });

        egui::Panel::bottom("statusbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if self.ctrl.state().busy {
                    ui.spinner();
                }
                ui.label(&self.ctrl.state().status);
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match &self.ctrl.state().open_archive {
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
                                        supazip_gui::context_menu::show_entry_context_menu(ui, e)
                                    {
                                        self.dispatch_entry_action(action);
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
        });
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
        let tx = self.ctrl.engine_sender();
        let cancel = self.ctrl.cancel_handle();
        self.ctrl.mark_busy(format!("opening {}…", path.display()));
        std::thread::spawn(move || {
            let ev = run_list_blocking(&path, &cancel);
            let _ = tx.send(ev);
        });
    }

    fn spawn_extract(&mut self, archive: PathBuf, out: PathBuf) {
        let tx = self.ctrl.engine_sender();
        let cancel = self.ctrl.cancel_handle();
        let limits = *self.ctrl.limits();
        self.ctrl.mark_busy(format!(
            "extracting {} to {}…",
            archive.display(),
            out.display()
        ));
        std::thread::spawn(move || {
            let ev = run_extract_blocking(&archive, &out, &cancel, &limits);
            let _ = tx.send(ev);
        });
    }

    fn spawn_create(&mut self, target: PathBuf, inputs: Vec<PathBuf>) {
        let tx = self.ctrl.engine_sender();
        let limits = *self.ctrl.limits();
        self.ctrl.mark_busy(format!(
            "creating {} ({} entries)…",
            target.display(),
            inputs.len()
        ));
        std::thread::spawn(move || {
            let ev = run_create_blocking(&target, &inputs, &limits);
            let _ = tx.send(ev);
        });
    }

    fn spawn_test(&mut self, archive: PathBuf) {
        let tx = self.ctrl.engine_sender();
        let cancel = self.ctrl.cancel_handle();
        let limits = *self.ctrl.limits();
        self.ctrl
            .mark_busy(format!("testing {}…", archive.display()));
        std::thread::spawn(move || {
            let ev = run_test_blocking(&archive, &cancel, &limits);
            let _ = tx.send(ev);
        });
    }

    /// Handle a context-menu click on an entry. The controller records
    /// the intent in state (busy / status) for every variant; the GUI
    /// additionally spawns a worker for the extract variants.
    fn dispatch_entry_action(&mut self, action: EntryContextAction) {
        match action.kind {
            EntryAction::ExtractHere(dest) | EntryAction::ExtractTo(dest) => {
                let Some(oa) = self.ctrl.state().open_archive.clone() else {
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

                // Apply the EngineEvent::Extract so the state-machine
                // tests see the intent without a worker.
                self.ctrl.apply(EngineEvent::Extract {
                    entries: entries.clone(),
                    dest: dest_for_spawn.clone(),
                    password: None,
                });
                self.spawn_extract_entries(oa.path, dest_for_spawn, entries);
            }
            EntryAction::TestEntry | EntryAction::CopyPath => {
                // No worker thread needed: status update only. The
                // clipboard write happened inside `show_entry_context_menu`.
                self.ctrl.dispatch_entry_action(action);
            }
        }
    }

    fn spawn_extract_entries(&mut self, archive: PathBuf, out: PathBuf, entries: Vec<String>) {
        let tx = self.ctrl.engine_sender();
        let cancel = self.ctrl.cancel_handle();
        let limits = *self.ctrl.limits();
        self.ctrl.mark_busy(format!(
            "extracting {} entr{} from {} to {}…",
            entries.len(),
            if entries.len() == 1 { "y" } else { "ies" },
            archive.display(),
            out.display()
        ));
        // The EngineEvent::Extract was applied by the caller; do not
        // double-apply it here.
        std::thread::spawn(move || {
            let ev = run_extract_entries_blocking(&archive, &out, &entries, &cancel, &limits);
            let _ = tx.send(ev);
        });
    }
}

// ---------------------------------------------------------------------------
// Worker-thread bodies. Plain functions so the App impl above stays small.
// ---------------------------------------------------------------------------

fn run_list_blocking(path: &std::path::Path, _cancel: &Arc<ProgressState>) -> EngineEvent {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("open: {e}")),
    };
    match backend.list(
        Box::new(std::io::BufReader::new(file)),
        None,
        &Limits::default(),
    ) {
        Ok(list) => EngineEvent::Listed {
            path: path.to_path_buf(),
            backend_name: backend.name(),
            entries: list.into_iter().map(OpenEntry::from).collect(),
        },
        Err(e) => EngineEvent::Error(format!("list: {e}")),
    }
}

fn run_extract_blocking(
    archive: &std::path::Path,
    out: &std::path::Path,
    cancel: &Arc<ProgressState>,
    limits: &Limits,
) -> EngineEvent {
    run_extract_entries_blocking(archive, out, &[], cancel, limits)
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
    cancel: &Arc<ProgressState>,
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
        None,
        cancel,
        limits,
    ) {
        Ok(()) => EngineEvent::Done(format!("extracted to {}", out.display())),
        Err(e) => EngineEvent::Error(format!("extract: {e}")),
    }
}

fn run_create_blocking(
    target: &std::path::Path,
    inputs: &[PathBuf],
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
    };
    let progress = supazip_core::traits::NoOpProgress;
    match backend.create(writer, inputs, &opts, None, &progress, limits) {
        Ok(()) => EngineEvent::Done(format!("created {}", target.display())),
        Err(e) => EngineEvent::Error(format!("create: {e}")),
    }
}

fn run_test_blocking(
    archive: &std::path::Path,
    cancel: &Arc<ProgressState>,
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
    match backend.test(
        Box::new(std::io::BufReader::new(file)),
        None,
        cancel,
        limits,
    ) {
        Ok(true) => EngineEvent::Done(format!("OK: {}", archive.display())),
        Ok(false) => EngineEvent::Error("test: integrity check failed".into()),
        Err(e) => EngineEvent::Error(format!("test: {e}")),
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

    let app = App::default();
    let native_options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native("SupaZip", native_options, Box::new(|_cc| Ok(Box::new(app))))
    {
        eprintln!("eframe failed: {e}");
        std::process::exit(1);
    }
}
