//! SupaZip GUI — a PeaZip-style archive manager for 7z and ZIP, built on
//! eframe/egui.
//!
//! This binary is the desktop front-end for the same engine the CLI uses
//! (`supazip-core`). The goal of the GUI in this commit is a working
//! *skeleton* — a usable shell that drives the engine through every public
//! operation. Rich features (drag-and-drop, parallel jobs, thumbnails) are
//! tracked in the memory bank.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use eframe::egui;
use supazip_core::traits::ProgressState;
use supazip_core::{formats, Limits};

// ---------------------------------------------------------------------------
// GUI: an `App` that holds the state the user sees.
//
// The struct is split into three concerns:
//   1. The currently open archive, if any (`open_archive`).
//   2. The view model: the entry list, the selected entry, status text.
//   3. The async plumbing: a `mpsc` pair so the GUI thread can fire an
//      engine operation on a worker thread and receive progress events.
//
// The GUI thread owns `App`. Engine operations run on a `std::thread` that
// calls into `supazip-core` synchronously (the engine is not async-aware
// today) and pushes `EngineEvent` messages back through the channel.
// ---------------------------------------------------------------------------

pub struct App {
    open_archive: Option<OpenArchive>,

    /// Free-form status string shown in the status bar.
    status: String,

    /// True when an engine operation is running; the toolbar buttons disable
    /// themselves while this is set.
    busy: bool,

    /// Channel for engine completion events.
    engine_tx: Sender<EngineEvent>,
    engine_rx: Receiver<EngineEvent>,

    /// Shared cancellation flag; the GUI sets this on Cancel, the engine
    /// checks it between entries.
    cancel_flag: Arc<ProgressState>,
}

/// One opened archive. Holds the parsed entry list and the backend name
/// that produced it.
struct OpenArchive {
    path: PathBuf,
    backend_name: &'static str,
    entries: Vec<OpenEntry>,
}

/// One row in the entry grid.
#[derive(Clone)]
struct OpenEntry {
    name: String,
    size: u64,
    encrypted: bool,
}

/// What the worker thread can send back to the GUI.
enum EngineEvent {
    /// List succeeded: a fresh entry list is now active.
    Listed {
        path: PathBuf,
        backend_name: &'static str,
        entries: Vec<OpenEntry>,
    },
    /// An operation finished successfully; carries a free-form status line.
    Done(String),
    /// The engine produced an error; the GUI shows it in the status bar and
    /// re-enables the toolbar.
    Error(String),
}

impl Default for App {
    fn default() -> Self {
        let (engine_tx, engine_rx) = channel();
        Self {
            open_archive: None,
            status: "Open an archive to get started.".to_string(),
            busy: false,
            engine_tx,
            engine_rx,
            cancel_flag: Arc::new(ProgressState::new()),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain engine events first so the UI reflects the latest state.
        self.drain_engine_events();

        egui::Panel::top("toolbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Open…"))
                    .clicked()
                {
                    self.open_dialog();
                }
                let can_extract = self.open_archive.is_some() && !self.busy;
                if ui
                    .add_enabled(can_extract, egui::Button::new("Extract"))
                    .clicked()
                {
                    self.extract_clicked();
                }
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Create…"))
                    .clicked()
                {
                    self.create_clicked();
                }
                let can_test = self.open_archive.is_some() && !self.busy;
                if ui
                    .add_enabled(can_test, egui::Button::new("Test"))
                    .clicked()
                {
                    self.test_clicked();
                }
                if self.busy && ui.button("Cancel").clicked() {
                    self.cancel_flag.cancel();
                }
            });
        });

        egui::Panel::bottom("statusbar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                ui.label(&self.status);
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match &self.open_archive {
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
    fn drain_engine_events(&mut self) {
        while let Ok(ev) = self.engine_rx.try_recv() {
            match ev {
                EngineEvent::Listed {
                    path,
                    backend_name,
                    entries,
                } => {
                    self.open_archive = Some(OpenArchive {
                        path,
                        backend_name,
                        entries,
                    });
                    self.busy = false;
                    self.status = "loaded".to_string();
                    self.cancel_flag = Arc::new(ProgressState::new());
                }
                EngineEvent::Done(msg) => {
                    self.busy = false;
                    self.status = msg;
                    self.cancel_flag = Arc::new(ProgressState::new());
                }
                EngineEvent::Error(msg) => {
                    self.busy = false;
                    self.status = format!("error: {msg}");
                    self.cancel_flag = Arc::new(ProgressState::new());
                }
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("archives", &["zip", "7z"])
            .pick_file()
        {
            self.spawn_list(path);
        }
    }

    fn extract_clicked(&mut self) {
        let Some(oa) = &self.open_archive else { return };
        let archive = oa.path.clone();
        let out = match rfd::FileDialog::new().pick_folder() {
            Some(f) => f,
            None => return,
        };
        self.spawn_extract(archive, out);
    }

    fn create_clicked(&mut self) {
        // Pick a destination first; the user then types the archive name.
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
                self.status = "create cancelled (no input files)".into();
                return;
            }
        };
        self.spawn_create(target, inputs);
    }

    fn test_clicked(&mut self) {
        let Some(oa) = &self.open_archive else { return };
        let archive = oa.path.clone();
        self.spawn_test(archive);
    }

    fn spawn_list(&mut self, path: PathBuf) {
        let tx = self.engine_tx.clone();
        let cancel = self.cancel_flag.clone();
        self.busy = true;
        self.status = format!("opening {}…", path.display());
        std::thread::spawn(move || {
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let backend = match formats::get_backend(&ext) {
                Some(b) => b,
                None => {
                    let _ = tx.send(EngineEvent::Error(format!("unsupported: {ext}")));
                    return;
                }
            };
            let file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("open: {e}")));
                    return;
                }
            };
            let entries = match backend.list(
                Box::new(std::io::BufReader::new(file)),
                None,
                &Limits::default(),
            ) {
                Ok(list) => list
                    .into_iter()
                    .map(|e| OpenEntry {
                        name: e.name,
                        size: e.size,
                        encrypted: e.encrypted,
                    })
                    .collect(),
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("list: {e}")));
                    return;
                }
            };
            let _ = cancel; // exercised in extract / test paths
            let _ = tx.send(EngineEvent::Listed {
                path,
                backend_name: backend.name(),
                entries,
            });
        });
    }

    fn spawn_extract(&mut self, archive: PathBuf, out: PathBuf) {
        let tx = self.engine_tx.clone();
        let cancel = self.cancel_flag.clone();
        self.busy = true;
        self.status = format!("extracting {} to {}…", archive.display(), out.display());
        std::thread::spawn(move || {
            let ext = archive.extension().and_then(|s| s.to_str()).unwrap_or("");
            let backend = match formats::get_backend(ext) {
                Some(b) => b,
                None => {
                    let _ = tx.send(EngineEvent::Error(format!("unsupported: {ext}")));
                    return;
                }
            };
            let file = match std::fs::File::open(&archive) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("open: {e}")));
                    return;
                }
            };
            let cancel_cb: Arc<ProgressState> = cancel;
            let res = backend.extract(
                Box::new(std::io::BufReader::new(file)),
                &out,
                &[],
                None,
                &cancel_cb,
                &Limits::default(),
            );
            match res {
                Ok(()) => {
                    let _ = tx.send(EngineEvent::Done(format!("extracted to {}", out.display())));
                }
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("extract: {e}")));
                }
            }
        });
    }

    fn spawn_create(&mut self, target: PathBuf, inputs: Vec<PathBuf>) {
        let tx = self.engine_tx.clone();
        self.busy = true;
        self.status = format!("creating {} ({} entries)…", target.display(), inputs.len());
        std::thread::spawn(move || {
            let ext = target.extension().and_then(|s| s.to_str()).unwrap_or("");
            let backend = match formats::get_backend(ext) {
                Some(b) => b,
                None => {
                    let _ = tx.send(EngineEvent::Error(format!("unsupported: {ext}")));
                    return;
                }
            };
            let file = match std::fs::File::create(&target) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("create target: {e}")));
                    return;
                }
            };
            let writer: Box<dyn supazip_core::traits::WriteSeek> =
                Box::new(std::io::BufWriter::new(file));
            let opts = supazip_core::traits::CreateOptions {
                compression_method: "deflate".to_string(),
                compression_level: None,
            };
            let progress = supazip_core::traits::NoOpProgress;
            let res = backend.create(writer, &inputs, &opts, None, &progress, &Limits::default());
            match res {
                Ok(()) => {
                    let _ = tx.send(EngineEvent::Done(format!("created {}", target.display())));
                }
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("create: {e}")));
                }
            }
        });
    }

    fn spawn_test(&mut self, archive: PathBuf) {
        let tx = self.engine_tx.clone();
        let cancel = self.cancel_flag.clone();
        self.busy = true;
        self.status = format!("testing {}…", archive.display());
        std::thread::spawn(move || {
            let ext = archive.extension().and_then(|s| s.to_str()).unwrap_or("");
            let backend = match formats::get_backend(ext) {
                Some(b) => b,
                None => {
                    let _ = tx.send(EngineEvent::Error(format!("unsupported: {ext}")));
                    return;
                }
            };
            let file = match std::fs::File::open(&archive) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("open: {e}")));
                    return;
                }
            };
            let cancel_cb: Arc<ProgressState> = cancel;
            let res = backend.test(
                Box::new(std::io::BufReader::new(file)),
                None,
                &cancel_cb,
                &Limits::default(),
            );
            match res {
                Ok(true) => {
                    let _ = tx.send(EngineEvent::Done(format!("OK: {}", archive.display())));
                }
                Ok(false) => {
                    let _ = tx.send(EngineEvent::Error("test: integrity check failed".into()));
                }
                Err(e) => {
                    let _ = tx.send(EngineEvent::Error(format!("test: {e}")));
                }
            }
        });
    }
}

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
