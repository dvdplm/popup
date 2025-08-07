use egui;

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

impl TrrpyApp {
    pub fn update(&mut self, ctx: &egui::Context) {
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
