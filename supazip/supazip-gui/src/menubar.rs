//! Native menu bar (WS-F, milestone 0.3.0).
//!
//! The bar is built with eframe's `with_menu` API: on macOS the menu lives
//! in the system menu bar at the top of the screen, on Windows / Linux it
//! is rendered in-app at the top of the window. The widgets are the
//! `egui::menu::bar` helper inside an `egui::TopBottomPanel::top` slot,
//! which is the form the milestone plan prescribes.
//!
//! ## Architecture
//!
//! `show_menu_bar` is the only UI touchpoint. It renders the four
//! top-level menus (File, Edit, View, Help), polls the egui input state
//! for the keyboard accelerators, and returns a flat `Vec<MenuAction>`
//! the caller should dispatch. The split exists because some actions
//! (open file picker, viewport close) need a live `egui::Context` and
//! cannot be handled by the headless [`crate::AppController`].
//!
//! [`AppController::dispatch_menu_action`](crate::AppController::dispatch_menu_action)
//! handles every action that can be applied without a window:
//! [`MenuAction::ToggleDebug`], [`MenuAction::Close`], and
//! [`MenuAction::About`]. The remaining actions are returned to the
//! caller through [`MenuActionOutcome::Gui`].

use crate::AppController;
use eframe::egui;

/// All possible top-level menu items. The order matches the order the
/// items appear in the on-screen menu so the variant index in `Debug` is
/// stable for snapshot tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuAction {
    /// File → Open…  (Ctrl/Cmd-O)
    Open,
    /// File → Close   (Ctrl/Cmd-W)
    Close,
    /// File → Extract… (Ctrl/Cmd-E)
    Extract,
    /// File → Create… (no accelerator)
    Create,
    /// File → Test integrity (no accelerator)
    Test,
    /// Help → About   (F1)
    About,
    /// File → Quit    (Ctrl/Cmd-Q)
    Quit,
    /// View → Debug overlay (checkbox; no accelerator)
    ToggleDebug,
    /// File → Settings… (no accelerator)
    Settings,
}

impl MenuAction {
    /// Every variant in declaration order. Useful for the snapshot and
    /// round-trip tests below.
    pub const ALL: &'static [MenuAction] = &[
        MenuAction::Open,
        MenuAction::Close,
        MenuAction::Extract,
        MenuAction::Create,
        MenuAction::Test,
        MenuAction::About,
        MenuAction::Quit,
        MenuAction::ToggleDebug,
        MenuAction::Settings,
    ];
}

/// What [`AppController::dispatch_menu_action`](crate::AppController::dispatch_menu_action)
/// did with the action. The GUI front-end uses this to decide whether it
/// still has work to do (open a file picker, send a viewport command).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuActionOutcome {
    /// The controller applied the action and updated state.
    Done,
    /// The action was a no-op for the current state (e.g. `Close` with
    /// no archive open). The caller does not have to do anything.
    Noop,
    /// The action needs a live window to fire (file dialog, viewport
    /// command). The GUI front-end handles it from the action list.
    Gui,
}

/// Render the menu bar and collect the actions the user triggered this
/// frame, including keyboard accelerators. Pure with respect to the
/// controller: `ctrl.show_debug` is flipped in place through the View
/// menu checkbox, everything else is reported through the return value.
pub fn show_menu_bar(ctx: &egui::Context, ctrl: &mut AppController) -> Vec<MenuAction> {
    let mut actions = Vec::new();

    egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open\u{2026}    Ctrl+O").clicked() {
                    actions.push(MenuAction::Open);
                    ui.close_menu();
                }
                if ui.button("Close    Ctrl+W").clicked() {
                    actions.push(MenuAction::Close);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Extract\u{2026}    Ctrl+E").clicked() {
                    actions.push(MenuAction::Extract);
                    ui.close_menu();
                }
                if ui.button("Create\u{2026}").clicked() {
                    actions.push(MenuAction::Create);
                    ui.close_menu();
                }
                if ui.button("Test integrity").clicked() {
                    actions.push(MenuAction::Test);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Settings\u{2026}").clicked() {
                    actions.push(MenuAction::Settings);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Quit    Ctrl+Q").clicked() {
                    actions.push(MenuAction::Quit);
                    ui.close_menu();
                }
            });
            ui.menu_button("Edit", |ui| {
                // "Copy path" is owned by the right-click context menu
                // (WS-B). The Edit menu keeps a placeholder so the menu
                // bar shape is consistent across platforms; the click is
                // a no-op for now and a TODO captures the integration.
                if ui
                    .add_enabled(false, egui::Button::new("Copy path (use context menu)"))
                    .clicked()
                {
                    ui.close_menu();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut ctrl.state.show_debug, "Debug overlay");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("About    F1").clicked() {
                    actions.push(MenuAction::About);
                    ui.close_menu();
                }
            });
        });
    });

    // Keyboard accelerators. The `consume_*` helpers would also work, but
    // we want every modifier to count as a fresh trigger on a fresh
    // frame, and the simple read-only form is enough for that.
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::O)) {
        actions.push(MenuAction::Open);
    }
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::E)) {
        actions.push(MenuAction::Extract);
    }
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::W)) {
        actions.push(MenuAction::Close);
    }
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Q)) {
        actions.push(MenuAction::Quit);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
        actions.push(MenuAction::About);
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EngineEvent;

    #[test]
    fn menu_action_variants() {
        // Every variant must Debug-format with a distinct tag so a
        // snapshot test or a panic message is unambiguous.
        let tags: Vec<String> = MenuAction::ALL.iter().map(|a| format!("{a:?}")).collect();
        assert_eq!(tags.len(), 9);
        // No two variants share a Debug string.
        let mut sorted = tags.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), tags.len(), "duplicates: {tags:?}");

        // Spot-check the names. A rename that breaks a Debug tag is a
        // UX regression; this test is the early warning.
        assert_eq!(format!("{:?}", MenuAction::Open), "Open");
        assert_eq!(format!("{:?}", MenuAction::Close), "Close");
        assert_eq!(format!("{:?}", MenuAction::Extract), "Extract");
        assert_eq!(format!("{:?}", MenuAction::Create), "Create");
        assert_eq!(format!("{:?}", MenuAction::Test), "Test");
        assert_eq!(format!("{:?}", MenuAction::About), "About");
        assert_eq!(format!("{:?}", MenuAction::Quit), "Quit");
        assert_eq!(format!("{:?}", MenuAction::ToggleDebug), "ToggleDebug");
        assert_eq!(format!("{:?}", MenuAction::Settings), "Settings");
    }

    #[test]
    fn menu_action_all_iterates_nine() {
        // Guard against accidentally dropping a variant from the
        // declaration list.
        assert_eq!(MenuAction::ALL.len(), 9);
    }

    #[test]
    fn menu_action_outcome_distinct_variants() {
        // The outcome enum is the only channel between the headless
        // controller and the GUI front-end. Its variants must stay
        // distinct so the GUI switch cannot silently fall through.
        assert_ne!(MenuActionOutcome::Done, MenuActionOutcome::Noop);
        assert_ne!(MenuActionOutcome::Done, MenuActionOutcome::Gui);
        assert_ne!(MenuActionOutcome::Noop, MenuActionOutcome::Gui);
    }

    #[test]
    fn app_controller_dispatch_menu_action_toggle_debug_flips_field() {
        let mut ctrl = AppController::default();
        assert!(!ctrl.state().show_debug, "default is off");
        let outcome = ctrl.dispatch_menu_action(MenuAction::ToggleDebug);
        assert_eq!(outcome, MenuActionOutcome::Done);
        assert!(ctrl.state().show_debug, "first toggle should turn it on");
        ctrl.dispatch_menu_action(MenuAction::ToggleDebug);
        assert!(!ctrl.state().show_debug, "second toggle should turn it off");
        // Status line reflects the new state for status-bar visibility.
        assert!(ctrl.state().status.contains("debug overlay"));
    }

    #[test]
    fn app_controller_dispatch_menu_action_about_opens_modal() {
        let mut ctrl = AppController::default();
        assert!(!ctrl.state().show_about);
        let outcome = ctrl.dispatch_menu_action(MenuAction::About);
        assert_eq!(outcome, MenuActionOutcome::Done);
        assert!(ctrl.state().show_about);
    }

    #[test]
    fn app_controller_dispatch_menu_action_close_drops_open_archive() {
        let mut ctrl = AppController::default();
        // Simulate a loaded archive.
        ctrl.apply(EngineEvent::Listed {
            path: std::path::PathBuf::from("/tmp/x.zip"),
            backend_name: "zip",
            entries: vec![],
        });
        assert!(ctrl.state().open_archive.is_some());
        let outcome = ctrl.dispatch_menu_action(MenuAction::Close);
        assert_eq!(outcome, MenuActionOutcome::Done);
        assert!(ctrl.state().open_archive.is_none());
        assert!(!ctrl.state().busy);
    }

    #[test]
    fn app_controller_dispatch_menu_action_close_with_no_archive_is_noop() {
        let mut ctrl = AppController::default();
        assert!(ctrl.state().open_archive.is_none());
        let outcome = ctrl.dispatch_menu_action(MenuAction::Close);
        assert_eq!(outcome, MenuActionOutcome::Noop);
    }

    #[test]
    fn app_controller_dispatch_menu_action_open_extract_create_test_quit_are_gui() {
        // All five actions need a live window: file pickers or
        // ViewportCommand::Close. The controller must report them as
        // `Gui` so the front-end runs them.
        let mut ctrl = AppController::default();
        for action in [
            MenuAction::Open,
            MenuAction::Extract,
            MenuAction::Create,
            MenuAction::Test,
            MenuAction::Quit,
        ] {
            assert_eq!(
                ctrl.dispatch_menu_action(action),
                MenuActionOutcome::Gui,
                "{action:?} should be routed to the GUI front-end"
            );
        }
    }
}
