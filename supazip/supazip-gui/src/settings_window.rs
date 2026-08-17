//! Floating settings window (WS-G, milestone 0.5.0).
//!
//! Rendered as a centred, non-resizable `egui::Window`. The caller owns
//! the visibility flag and the `Settings` struct; this module only draws
//! the widgets and calls `save` when the user presses "Save" or closes
//! the window.

use eframe::egui;

use crate::settings::{Settings, ThemePreference};
use crate::theme;

/// Show the settings window.
///
/// * `open` — visibility flag. Set to `true` to show the window; egui
///   sets it to `false` when the user clicks the close button. The
///   function persists settings on close.
pub fn show_settings_window(ctx: &egui::Context, settings: &mut Settings, open: &mut bool) {
    let was_open = *open;

    egui::Window::new("Settings")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(theme::dialog_frame(ctx))
        .open(open)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Language:");
                let lang = settings.language.as_deref().unwrap_or("system");
                egui::ComboBox::from_id_salt("language")
                    .selected_text(lang)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.language, None, "system");
                        ui.selectable_value(&mut settings.language, Some("en".into()), "en");
                        ui.selectable_value(&mut settings.language, Some("ru".into()), "ru");
                        ui.selectable_value(&mut settings.language, Some("de".into()), "de");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Max archive size (MiB):");
                let mut mb = settings.max_archive_size / (1024 * 1024);
                ui.add(egui::DragValue::new(&mut mb).range(1..=4096));
                settings.max_archive_size = mb * 1024 * 1024;
            });
            ui.horizontal(|ui| {
                ui.label("Recent files limit:");
                ui.add(egui::DragValue::new(&mut settings.recent_files_limit).range(1..=50));
            });
            ui.horizontal(|ui| {
                ui.label("Theme:");
                let theme_label = match settings.theme {
                    ThemePreference::Dark => "Dark",
                    ThemePreference::Light => "Light",
                    ThemePreference::System => "System",
                };
                egui::ComboBox::from_id_salt("theme")
                    .selected_text(theme_label)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut settings.theme, ThemePreference::Dark, "Dark");
                        ui.selectable_value(&mut settings.theme, ThemePreference::Light, "Light");
                        ui.selectable_value(&mut settings.theme, ThemePreference::System, "System");
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Debug overlay:");
                ui.checkbox(&mut settings.show_debug_overlay, "");
            });
            if ui.button("Save").clicked() {
                if let Err(e) = settings.save() {
                    log::warn!("failed to save settings: {e}");
                }
            }
        });

    // Persist on close (the X button).
    if was_open && !*open {
        if let Err(e) = settings.save() {
            log::warn!("failed to save settings on close: {e}");
        }
    }
}
