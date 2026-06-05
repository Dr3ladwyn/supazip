fn main() {
    eframe::run_native(
        "SupaZip",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(supazip_gui::App::default()))),
    );
}

mod supazip_gui {
    use eframe::egui;

    pub struct App;

    impl Default for App {
        fn default() -> Self {
            Self
        }
    }

    impl eframe::App for App {
        fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("SupaZip");
                ui.label("Coming soon...");
            });
        }
    }
}
