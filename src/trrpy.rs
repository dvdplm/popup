use crate::blitzortung::{BLITZ_HANDSHAKE, BLITZSERVERS, LightningStrike};
use crate::utils::ll;
use crate::websocket::WebSocketManager;
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
    connection_status: ConnectionStatus,
    lightning_strikes: Vec<String>,
    max_strikes: usize,
    is_popup_visible: bool,
}

#[derive(Debug, Clone)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
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
            connection_status: ConnectionStatus::Disconnected,
            lightning_strikes: Vec::new(),
            max_strikes: 100, // Keep only the last 100 strikes
            is_popup_visible: false,
        }
    }
}

impl TrrpyApp {
    pub fn set_popup_visible(&mut self, visible: bool) {
        if self.is_popup_visible != visible {
            self.is_popup_visible = visible;

            if visible {
                self.connect_blitzortung();
            } else {
                self.disconnect_blitzortung();
            }
        }
    }

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
            ui.heading("⚡ Lightning Strike Monitor");
            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Connect Blitzortung").clicked() {
                    self.connect_blitzortung();
                }
                if ui.button("Disconnect").clicked() {
                    self.disconnect_blitzortung();
                }
            });
            ui.separator();

            // Connection status indicator
            ui.horizontal(|ui| {
                let (color, text) = match &self.connection_status {
                    ConnectionStatus::Disconnected => (egui::Color32::GRAY, "Disconnected"),
                    ConnectionStatus::Connecting => (egui::Color32::YELLOW, "Connecting..."),
                    ConnectionStatus::Connected => (egui::Color32::GREEN, "Connected"),
                    ConnectionStatus::Error(err) => (egui::Color32::RED, err.as_str()),
                };

                // Draw status dot
                let (response, painter) =
                    ui.allocate_painter(egui::Vec2::splat(16.0), egui::Sense::hover());
                let center = response.rect.center();
                painter.circle_filled(center, 6.0, color);

                ui.label(text);
            });

            ui.separator();

            // Lightning strikes display
            ui.label(format!(
                "⚡ Lightning Strikes ({} total)",
                self.lightning_strikes.len()
            ));

            egui::ScrollArea::vertical()
                .max_height(250.0)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if self.lightning_strikes.is_empty() {
                        ui.label("Waiting for lightning strikes...");
                    } else {
                        // Show most recent strikes first
                        for strike in self.lightning_strikes.iter().rev() {
                            ui.label(strike);
                        }
                    }
                });

            ui.separator();

            // Debug info
            ui.collapsing("Debug Info", |ui| {
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

                // Text input test
                ui.horizontal(|ui| {
                    ui.label("Text input test:");
                    ui.text_edit_singleline(&mut self.text_input);
                });
            });

            ui.separator();
            ui.colored_label(
                egui::Color32::from_rgb(100, 149, 237),
                "💡 Press ESC or hotkey (Cmd+Shift+K) to hide",
            );
        });
    }

    fn connect_blitzortung(&mut self) {
        ll("⚡ Connecting to Blitzortung...");

        if self.websocket_manager.is_none() {
            let manager = Arc::new(Mutex::new(WebSocketManager::new()));
            self.websocket_manager = Some(manager.clone());
        }

        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                // Try the first server
                mgr.connect(BLITZSERVERS[0].to_string());
                self.connection_status = ConnectionStatus::Connecting;
            }
        }
    }

    fn disconnect_blitzortung(&mut self) {
        ll("⚡ Disconnecting from Blitzortung...");

        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                mgr.disconnect();
                self.connection_status = ConnectionStatus::Disconnected;
            }
        }
    }

    fn send_blitz_handshake(&self) {
        if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                ll("⚡ Sending Blitzortung handshake...");
                mgr.send_raw(BLITZ_HANDSHAKE.to_vec());
            }
        }
    }

    fn handle_websocket_messages(&mut self) {
        let messages: Vec<_> = if let Some(ref manager) = self.websocket_manager {
            if let Ok(mgr) = manager.lock() {
                let mut msgs = Vec::new();
                while let Some(message) = mgr.try_recv_message() {
                    msgs.push(message);
                }
                msgs
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        for message in messages {
            match message.mtype.as_str() {
                "connection_status" => {
                    let payload_str = String::from_utf8_lossy(&message.payload);
                    ll(&format!("⚡ Connection status: {}", payload_str));

                    if payload_str.contains("connected") {
                        self.connection_status = ConnectionStatus::Connected;
                        // Send handshake once connected
                        self.send_blitz_handshake();
                    } else if payload_str.contains("failed") || payload_str.contains("error") {
                        self.connection_status = ConnectionStatus::Error(payload_str.to_string());
                    } else if payload_str.contains("disconnected") {
                        self.connection_status = ConnectionStatus::Disconnected;
                    }
                }
                "raw_text" => {
                    let payload_str = String::from_utf8_lossy(&message.payload);
                    self.handle_lightning_message(&payload_str);
                }
                _ => {
                    ll(&format!("⚡ Unknown message type: {}", message.mtype));
                }
            }
        }
    }

    fn handle_lightning_message(&mut self, message: &str) {
        // Try to decode the message
        let decoded_message = crate::blitzortung::decode(message);

        // Try to parse as JSON lightning strike
        if let Ok(strike) = serde_json::from_str::<LightningStrike>(&decoded_message) {
            let datetime = std::time::UNIX_EPOCH + std::time::Duration::from_micros(strike.time);

            let strike_info =
                if let Ok(system_time) = datetime.duration_since(std::time::UNIX_EPOCH) {
                    let secs = system_time.as_secs();
                    let hours = (secs / 3600) % 24;
                    let minutes = (secs / 60) % 60;
                    let seconds = secs % 60;

                    format!(
                        "{:02}:{:02}:{:02} - Lat: {:.4}°, Lon: {:.4}°, Alt: {:.0}m",
                        hours, minutes, seconds, strike.lat, strike.lon, strike.alt
                    )
                } else {
                    format!(
                        "Time: {} - Lat: {:.4}°, Lon: {:.4}°, Alt: {:.0}m",
                        strike.time, strike.lat, strike.lon, strike.alt
                    )
                };

            self.lightning_strikes.push(strike_info);

            // Keep only the most recent strikes
            if self.lightning_strikes.len() > self.max_strikes {
                self.lightning_strikes.remove(0);
            }
        } else {
            // Log raw message for debugging
            ll(&format!("⚡ Raw message: {}", decoded_message));
        }
    }
}
