use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

use crate::utils::ll;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub mtype: String,
    pub payload: Vec<u8>,
    pub ts: u64,
}

#[derive(Debug)]
pub struct WebSocketManager {
    tx: mpsc::Sender<WebSocketCommand>,
    rx: mpsc::Receiver<WebSocketMessage>,
}

#[derive(Debug)]
pub enum WebSocketCommand {
    Connect(String),
    Disconnect,
    Send(WebSocketMessage),
    SendRaw(Vec<u8>),
}

impl WebSocketManager {
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<WebSocketCommand>();
        let (msg_tx, msg_rx) = mpsc::channel::<WebSocketMessage>();

        // Spawn the async worker thread
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");

            rt.block_on(async {
                WebSocketWorker::new(cmd_rx, msg_tx).run().await;
            });
        });

        Self {
            tx: cmd_tx,
            rx: msg_rx,
        }
    }

    pub fn connect(&self, url: String) {
        if let Err(e) = self.tx.send(WebSocketCommand::Connect(url)) {
            ll(&format!("❌ Failed to send connect command: {}", e));
        }
    }

    pub fn disconnect(&self) {
        if let Err(e) = self.tx.send(WebSocketCommand::Disconnect) {
            ll(&format!("❌ Failed to send disconnect command: {}", e));
        }
    }

    pub fn send_message(&self, message: WebSocketMessage) {
        if let Err(e) = self.tx.send(WebSocketCommand::Send(message)) {
            ll(&format!("❌ Failed to send message command: {}", e));
        }
    }

    pub fn send_raw(&self, payload: Vec<u8>) {
        if let Err(e) = self.tx.send(WebSocketCommand::SendRaw(payload)) {
            ll(&format!("❌ Failed to send raw message command: {}", e));
        }
    }

    pub fn try_recv_message(&self) -> Option<WebSocketMessage> {
        self.rx.try_recv().ok()
    }

    pub fn recv_message_blocking(&self) -> Result<WebSocketMessage, mpsc::RecvError> {
        self.rx.recv()
    }
}

struct WebSocketWorker {
    cmd_rx: mpsc::Receiver<WebSocketCommand>,
    msg_tx: mpsc::Sender<WebSocketMessage>,
    connection: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl WebSocketWorker {
    fn new(
        cmd_rx: mpsc::Receiver<WebSocketCommand>,
        msg_tx: mpsc::Sender<WebSocketMessage>,
    ) -> Self {
        Self {
            cmd_rx,
            msg_tx,
            connection: None,
        }
    }

    async fn run(&mut self) {
        ll("🌐 WebSocket worker thread started");

        loop {
            // Handle commands from the main thread
            while let Ok(cmd) = self.cmd_rx.try_recv() {
                match cmd {
                    WebSocketCommand::Connect(url) => {
                        self.connect(&url).await;
                    }
                    WebSocketCommand::Disconnect => {
                        self.disconnect().await;
                    }
                    WebSocketCommand::Send(message) => {
                        self.send_message(message).await;
                    }
                    WebSocketCommand::SendRaw(payload) => {
                        self.send_raw(payload).await;
                    }
                }
            }

            // Handle incoming messages if connected
            if let Some(ref mut ws_stream) = self.connection {
                // Use a timeout to avoid blocking indefinitely
                match tokio::time::timeout(
                    tokio::time::Duration::from_millis(100),
                    ws_stream.next(),
                )
                .await
                {
                    Ok(Some(msg_result)) => match msg_result {
                        Ok(msg) => {
                            self.handle_incoming_message(msg).await;
                        }
                        Err(e) => {
                            ll(&format!("❌ WebSocket error: {}", e));
                            self.disconnect().await;
                        }
                    },
                    Ok(None) => {
                        ll("🔌 WebSocket connection closed");
                        self.disconnect().await;
                    }
                    Err(_) => {
                        // Timeout - continue the loop to check for commands
                    }
                }
            } else {
                // Not connected, sleep a bit to avoid busy waiting
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }

    async fn connect(&mut self, url: &str) {
        ll(&format!("🌐 Connecting to WebSocket: {}", url));

        match connect_async(url).await {
            Ok((ws_stream, response)) => {
                ll(&format!(
                    "✅ Connected to WebSocket. Response: {:?}",
                    response.status()
                ));
                self.connection = Some(ws_stream);

                // Send a connection success message
                let msg = WebSocketMessage {
                    mtype: "connection_status".to_string(),
                    payload: serde_json::to_vec(&serde_json::json!({
                        "status": "connected",
                        "url": url
                    }))
                    .unwrap(),
                    ts: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                if let Err(e) = self.msg_tx.send(msg) {
                    ll(&format!("❌ Failed to send connection status: {}", e));
                }
            }
            Err(e) => {
                ll(&format!("❌ Failed to connect to WebSocket: {}", e));

                // Send a connection failure message
                let msg = WebSocketMessage {
                    mtype: "connection_status".to_string(),
                    payload: serde_json::to_vec(&serde_json::json!({
                        "status": "failed",
                        "error": e.to_string(),
                        "url": url
                    }))
                    .unwrap(),
                    ts: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                if let Err(send_err) = self.msg_tx.send(msg) {
                    ll(&format!(
                        "❌ Failed to send connection failure status: {}",
                        send_err
                    ));
                }
            }
        }
    }

    async fn disconnect(&mut self) {
        if let Some(mut ws_stream) = self.connection.take() {
            ll("🔌 Disconnecting from WebSocket");

            if let Err(e) = ws_stream.close(None).await {
                ll(&format!("⚠️ Error closing WebSocket: {}", e));
            }

            // Send a disconnection message
            let msg = WebSocketMessage {
                mtype: "connection_status".to_string(),
                payload: serde_json::to_vec(&serde_json::json!({
                    "status": "disconnected"
                }))
                .unwrap(),
                ts: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };

            if let Err(e) = self.msg_tx.send(msg) {
                ll(&format!("❌ Failed to send disconnection status: {}", e));
            }
        }
    }

    async fn send_message(&mut self, message: WebSocketMessage) {
        if let Some(ref mut ws_stream) = self.connection {
            ll(&format!("Sending {:?}", serde_json::to_string(&message)));
            match serde_json::to_string(&message) {
                Ok(json_str) => {
                    if let Err(e) = ws_stream.send(Message::Text(json_str)).await {
                        ll(&format!("❌ Failed to send WebSocket message: {}", e));
                    } else {
                        ll(&format!("📤 Sent WebSocket message"));
                    }
                }
                Err(e) => {
                    ll(&format!("❌ Failed to serialize message: {}", e));
                }
            }
        } else {
            ll("⚠️ Cannot send message: WebSocket not connected");
        }
    }

    async fn send_raw(&mut self, payload: Vec<u8>) {
        if let Some(ref mut ws_stream) = self.connection {
            match ws_stream.send(Message::Binary(payload)).await {
                Ok(r) => ll(&format!("raw send ok: {r:?}")),
                Err(e) => ll(&format!("raw send fail: {e:?}")),
            };
        }
        ()
    }

    async fn handle_incoming_message(&mut self, msg: Message) {
        match msg {
            Message::Text(text) => {
                // ll(&format!("📥 Received text message: {}", text));

                match serde_json::from_str::<WebSocketMessage>(&text) {
                    Ok(ws_message) => {
                        if let Err(e) = self.msg_tx.send(ws_message) {
                            ll(&format!(
                                "❌ Failed to forward message to main thread: {}",
                                e
                            ));
                        }
                    }
                    Err(_e) => {
                        // Send a raw message for unstructured data
                        let raw_message = WebSocketMessage {
                            mtype: "raw_text".to_string(),
                            payload: text.into_bytes(),
                            ts: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        };

                        if let Err(send_err) = self.msg_tx.send(raw_message) {
                            ll(&format!("❌ Failed to send raw message: {}", send_err));
                        }
                    }
                }
            }
            Message::Binary(data) => {
                ll(&format!("📥 Received binary message: {} bytes", data.len()));

                let binary_message = WebSocketMessage {
                    mtype: "binary".to_string(),
                    payload: serde_json::to_vec(&serde_json::json!({
                        "size": data.len(),
                        "data": base64::engine::general_purpose::STANDARD.encode(&data)
                    }))
                    .unwrap(),
                    ts: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                };

                if let Err(e) = self.msg_tx.send(binary_message) {
                    ll(&format!("❌ Failed to send binary message: {}", e));
                }
            }
            Message::Ping(data) => {
                ll("🏓 Received ping");
                if let Some(ws_stream) = &mut self.connection {
                    if let Err(e) = ws_stream.send(Message::Pong(data)).await {
                        ll(&format!("❌ Failed to send pong: {}", e));
                    }
                }
            }
            Message::Pong(_) => {
                ll("🏓 Received pong");
            }
            Message::Close(_) => {
                ll("🔌 Received close message");
            }
            Message::Frame(_) => {
                ll("📦 Received frame message (should not happen in this context)");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blitzortung::{BLITZ_HANDSHAKE, BLITZSERVERS, LightningStrike, decode};
    #[test]
    fn test_websocket_message() {
        let message = WebSocketMessage {
            mtype: "text".to_string(),
            payload: serde_json::to_vec(&serde_json::json!({
                "content": "Hello, world!"
            }))
            .unwrap(),
            ts: 1678901234,
        };

        assert_eq!(message.mtype, "text");
        // assert_eq!(message.payload["content"], "Hello, world!");
        assert_eq!(message.ts, 1678901234);
    }

    #[tokio::test]
    async fn ws_connection() {
        let ws_manager = WebSocketManager::new();
        ws_manager.connect("wss://echo.websocket.org/".to_string());

        // Wait for connection to establish
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check for connection status message
        if let Some(incoming) = ws_manager.try_recv_message() {
            println!("Debug: Full message received: {:?}", incoming);
            // assert_eq!(status_msg.message_type, "connection_status");
            // assert_eq!(status_msg.data["status"], "connected");
        }
    }

    #[tokio::test]
    async fn blitzortung_connect() {
        const MAXMSGS: usize = 2;

        let ws_manager = WebSocketManager::new();
        ws_manager.connect(BLITZSERVERS[0].into());

        // TODO: Use async magic to block here until connection is established
        // Wait for connection to establish
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Check for connection status message
        if let Some(incoming) = ws_manager.try_recv_message() {
            println!("Debug: Full message received: {:?}", incoming);
        }

        // Send magic json bytes
        ws_manager.send_raw(BLITZ_HANDSHAKE.to_vec());
        // Read MAXMSGS from the socket, then hang up. Print each message.
        let mut msg_count = 0;
        while msg_count < MAXMSGS {
            if let Some(incoming) = ws_manager.try_recv_message() {
                // It's a `raw_text` message. Inside the `payload` there's a bag of bytes representing a
                // println!("Message {}: {:?}", msg_count + 1, incoming);
                let payload_str = std::str::from_utf8(&incoming.payload).unwrap_or("");
                let decoded: String = decode(payload_str); // takes &str
                // println!("decoded message: {decoded:?}");
                let lightning = serde_json::from_str::<LightningStrike>(&decoded);
                // println!("Deserialized LightningStrike: {lightning:?}");
                assert!(lightning.is_ok());

                msg_count += 1;
            } else {
                // Wait a bit before checking again
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }

        ws_manager.disconnect();
    }
}
