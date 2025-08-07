use crate::utils::ll;
use eframe::egui;

#[derive(Debug)]
pub struct TrrpyApp {
    name: String,
    counter: i32,
}

impl Default for TrrpyApp {
    fn default() -> Self {
        Self {
            name: "Trrpy".to_owned(),
            counter: 0,
        }
    }
}

impl eframe::App for TrrpyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎯 Trrpy App");
            ui.separator();

            ui.label("Hello from the hotkey-triggered app!");
            ui.label(format!("App name: {}", self.name));

            ui.horizontal(|ui| {
                ui.label("Counter:");
                ui.label(format!("{}", self.counter));
                if ui.button("Increment").clicked() {
                    self.counter += 1;
                }
            });

            ui.separator();
            if ui.button("Close").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

pub fn create_and_show_trrpy_app() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_title("Trrpy"),
        ..Default::default()
    };

    ll("About to start egui");
    let out = eframe::run_native(
        "Trrpy",
        options,
        Box::new(|_cc| Ok(Box::new(TrrpyApp::default()))),
    );
    ll(&format!("out={:?}", out));
    out
}
