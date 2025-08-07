use egui;

#[derive(Debug)]
pub struct TrrpyApp {
    name: String,
    counter: i32,
    text_input: String,
    mouse_pos: egui::Pos2,
    last_key: Option<String>,
}

impl Default for TrrpyApp {
    fn default() -> Self {
        Self {
            name: "Trrpy".to_owned(),
            counter: 0,
            text_input: "Type something here...".to_owned(),
            mouse_pos: egui::Pos2::ZERO,
            last_key: None,
        }
    }
}

impl TrrpyApp {
    pub fn update(&mut self, ctx: &egui::Context) {
        // Capture mouse position
        if let Some(pointer_pos) = ctx.input(|i| i.pointer.hover_pos()) {
            self.mouse_pos = pointer_pos;
        }

        // Capture last pressed key
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key, pressed: true, ..
                } = event
                {
                    self.last_key = Some(format!("{:?}", key));
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎯 Trrpy App - Event Testing");
            ui.separator();

            ui.label("🖱️ Mouse & Keyboard Event Test");

            // Mouse position display
            ui.horizontal(|ui| {
                ui.label("Mouse position:");
                ui.label(format!(
                    "({:.1}, {:.1})",
                    self.mouse_pos.x, self.mouse_pos.y
                ));
            });

            // Last key pressed display
            ui.horizontal(|ui| {
                ui.label("Last key pressed:");
                ui.label(self.last_key.as_ref().unwrap_or(&"None".to_string()));
            });

            ui.separator();

            // Counter test
            ui.horizontal(|ui| {
                ui.label("Click counter:");
                ui.label(format!("{}", self.counter));
                if ui.button("Increment").clicked() {
                    self.counter += 1;
                }
                if ui.button("Reset").clicked() {
                    self.counter = 0;
                }
            });

            ui.separator();

            // Text input test
            ui.horizontal(|ui| {
                ui.label("Text input test:");
                ui.text_edit_singleline(&mut self.text_input);
            });

            ui.separator();

            // Instructions
            ui.label("Test the following:");
            ui.label("• Move your mouse around");
            ui.label("• Click buttons");
            ui.label("• Type in the text field");
            ui.label("• Press various keys");

            ui.separator();
            if ui.button("Close Window").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}
