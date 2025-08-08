use crate::utils::ll;
use crate::websocket::{WebSocketManager, WebSocketMessage};
use egui;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Debug)]
pub struct TrrpyApp {
    name: String,
    counter: i32,
    text_input: String,
    mouse_pos: egui::Pos2,
    last_key: Option<String>,
    pub esc_pressed: bool,
    pub prev_pid: Option<u32>,
    websocket_manager: Option<Arc<Mutex<WebSocketManager>>>,
    websocket_url: String,
    connection_status: String,
    received_messages: Vec<WebSocketMessage>,
    max_messages: usize,
}

impl Default for TrrpyApp {
    fn default() -> Self {
        Self {
            name: "Trrpy".to_owned(),
            counter: 0,
            text_input: "Type something here...".to_owned(),
            mouse_pos: egui::Pos2::ZERO,
            last_key: None,
            esc_pressed: false,
            prev_pid: None,
            websocket_manager: None,
            websocket_url: "wss://echo.websocket.org".to_owned(),
            connection_status: "Disconnected".to_owned(),
            received_messages: Vec::new(),
            max_messages: 50, // Keep only the last 50 messages
        }
    }
}

impl TrrpyApp {
    pub fn update(&mut self, ctx: &egui::Context) {
        self.esc_pressed = false;

        // Handle incoming WebSocket messages
        self.handle_websocket_messages();

        // Capture mouse position
        if let Some(pointer_pos) = ctx.input(|i| i.pointer.hover_pos()) {
            self.mouse_pos = pointer_pos;
        }

        // Capture last pressed key for display and detect ESC
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key: egui::Key::Escape,
                    pressed: true,
                    ..
                } = event
                {
                    self.esc_pressed = true;
                }
                if let egui::Event::Key {
                    key, pressed: true, ..
                } = event
                {
                    self.last_key = Some(format!("{:?}", key));
                }
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // ui.heading("🎯 Trrpy App - WebSocket & Event Testing");
            // ui.separator();

            // // WebSocket Controls
            // ui.label("🌐 WebSocket Connection");
            // ui.horizontal(|ui| {
            //     ui.label("URL:");
            //     ui.text_edit_singleline(&mut self.websocket_url);
            // });

            // ui.horizontal(|ui| {
            //     if ui.button("Connect").clicked() {
            //         self.connect_websocket();
            //     }
            //     if ui.button("Disconnect").clicked() {
            //         self.disconnect_websocket();
            //     }
            //     ui.label(format!("Status: {}", self.connection_status));
            // });

            // ui.horizontal(|ui| {
            //     if ui.button("Send Test Message").clicked() {
            //         self.send_test_message();
            //     }
            // });

            // ui.separator();

            // // WebSocket Messages Display
            // ui.label("📥 Recent Messages");
            // egui::ScrollArea::vertical()
            //     .max_height(200.0)
            //     .show(ui, |ui| {
            //         for msg in self.received_messages.iter().rev().take(10) {
            //             ui.horizontal(|ui| {
            //                 ui.label(format!("{}:", msg.message_type));
            //                 ui.label(msg.data.to_string());
            //             });
            //         }
            //         if self.received_messages.is_empty() {
            //             ui.label("No messages yet...");
            //         }
            //     });

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
            ui.colored_label(
                egui::Color32::from_rgb(100, 149, 237),
                "💡 Press ESC or hotkey (Cmd+Shift+K) to hide",
            );
            ui.label("🔑 ESC key is fully functional for quick dismissal");
        });
    }

    fn connect_websocket(&mut self) {
        if self.websocket_manager.is_none() {
            let manager = Arc::new(Mutex::new(WebSocketManager::new()));
            self.websocket_manager = Some(manager.clone());
        }

        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                mgr.connect(self.websocket_url.clone());
                self.connection_status = "Connecting...".to_owned();
            }
        }
    }

    fn disconnect_websocket(&mut self) {
        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                mgr.disconnect();
                self.connection_status = "Disconnecting...".to_owned();
            }
        }
    }

    // fn send_test_message(&self) {
    //     if let Some(ref manager) = self.websocket_manager {
    //         if let Ok(mgr) = manager.lock() {
    //             let test_message = WebSocketMessage {
    //                 message_type: "test".to_string(),
    //                 data: serde_json::json!({
    //                     "text": "Hello from Trrpy!",
    //                     "counter": self.counter,
    //                     "timestamp": std::time::SystemTime::now()
    //                         .duration_since(std::time::UNIX_EPOCH)
    //                         .unwrap()
    //                         .as_secs()
    //                 }),
    //                 timestamp: std::time::SystemTime::now()
    //                     .duration_since(std::time::UNIX_EPOCH)
    //                     .unwrap()
    //                     .as_secs(),
    //             };
    //             mgr.send_message(test_message);
    //         }
    //     }
    // }

    fn handle_websocket_messages(&mut self) {
        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                while let Some(message) = mgr.try_recv_message() {
                    // Update connection status based on message type
                    if message.mtype == "connection_status" {
                        let payload = String::from_utf8(message.payload.clone());
                        if let Ok(payload) = payload {
                            self.connection_status = payload
                        } else {
                            ll(&format!("Unexpected connection_status messag: {payload:?}"));
                        }
                    }

                    // Add to message history
                    self.received_messages.push(message);

                    // Keep only the most recent messages
                    if self.received_messages.len() > self.max_messages {
                        self.received_messages.remove(0);
                    }
                }
            }
        }
    }
}
