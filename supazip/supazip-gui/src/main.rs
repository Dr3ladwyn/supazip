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

use supazip_gui::{AppController, EngineEvent, OpenEntry};

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

        egui::Panel::top("toolbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let busy = self.ctrl.state().busy;
                if ui.add_enabled(!busy, egui::Button::new("Open…")).clicked() {
                    self.open_dialog();
                }
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
                                ui.label(&e.name);
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
    let ext = archive.extension().and_then(|s| s.to_str()).unwrap_or("");
    let Some(backend) = formats::get_backend(ext) else {
        return EngineEvent::Error(format!("unsupported: {ext}"));
    };
    let file = match std::fs::File::open(archive) {
        Ok(f) => f,
        Err(e) => return EngineEvent::Error(format!("open: {e}")),
    };
    match backend.extract(
        Box::new(std::io::BufReader::new(file)),
        out,
        &[],
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
