//! WebSocket connection manager.
//!
//! Provides connection pooling, lifecycle management, and CDP event emission
//! for WebSocket connections.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{debug, trace};

use crate::websocket::connection::{ConnectionId, WebSocketConnection, WebSocketFrame};

/// Configuration for WebSocket connections.
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// Maximum concurrent connections per origin.
    pub max_per_origin: usize,
    /// Connection timeout in seconds.
    pub connect_timeout_secs: u64,
    /// Maximum message size in bytes (0 = unlimited).
    pub max_message_size: usize,
    /// Block private IP addresses.
    pub block_private_ips: bool,
    /// Block loopback addresses.
    pub block_loopback: bool,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            max_per_origin: 6,
            connect_timeout_secs: 30,
            max_message_size: 10 * 1024 * 1024, // 10MB
            block_private_ips: true,
            block_loopback: true,
        }
    }
}

/// CDP event for WebSocket lifecycle.
#[derive(Debug, Clone)]
pub struct WebSocketEvent {
    pub method: String,
    pub params: serde_json::Value,
}

/// Sender for CDP events.
pub type EventBusSender = mpsc::UnboundedSender<WebSocketEvent>;

/// Manager for WebSocket connections.
pub struct WebSocketManager {
    /// Configuration.
    config: WebSocketConfig,
    /// Active connections by ID.
    connections: HashMap<ConnectionId, WebSocketConnection>,
    /// CDP event bus sender (optional).
    event_bus: Option<EventBusSender>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager with the given configuration.
    pub fn new(config: WebSocketConfig) -> Self {
        Self {
            config,
            connections: HashMap::new(),
            event_bus: None,
        }
    }

    /// Set the CDP event bus sender.
    pub fn with_event_bus(mut self, sender: EventBusSender) -> Self {
        self.event_bus = Some(sender);
        self
    }

    /// Connect to a WebSocket endpoint.
    ///
    /// Returns the connection ID on success.
    pub async fn connect(&mut self, url: &str) -> Result<ConnectionId> {
        // Check connection limit per origin
        let origin = extract_origin(url);
        let origin_count = self
            .connections
            .values()
            .filter(|c| extract_origin(c.url()) == origin)
            .count();

        if origin_count >= self.config.max_per_origin {
            return Err(anyhow!(
                "Maximum WebSocket connections ({}) reached for origin: {}",
                self.config.max_per_origin,
                origin
            ));
        }

        // Create connection
        let conn = WebSocketConnection::connect(
            url,
            self.config.block_private_ips,
            self.config.block_loopback,
            self.config.connect_timeout_secs,
        )
        .await?;

        let id = conn.id().to_string();
        let url_clone = conn.url().to_string();

        // Emit webSocketCreated event
        self.emit_event("Network.webSocketCreated", json!({
            "requestId": &id,
            "url": &url_clone,
            "initiator": {
                "type": "other"
            }
        }));

        debug!(id = %id, url = %url_clone, "WebSocket connection created");

        self.connections.insert(id.clone(), conn);
        Ok(id)
    }

    /// Get a connection by ID.
    pub fn get(&self, id: &str) -> Option<&WebSocketConnection> {
        self.connections.get(id)
    }

    /// Get a mutable connection by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut WebSocketConnection> {
        self.connections.get_mut(id)
    }

    /// Send a text message on a connection.
    pub async fn send_text(&mut self, id: &str, msg: &str) -> Result<WebSocketFrame> {
        let conn = self
            .connections
            .get_mut(id)
            .ok_or_else(|| anyhow!("WebSocket connection not found: {}", id))?;

        let frame = conn.send_text(msg).await?;

        // Emit webSocketFrameSent event
        self.emit_event("Network.webSocketFrameSent", json!({
            "requestId": id,
            "timestamp": frame.timestamp_ms,
            "response": {
                "opcode": if frame.opcode == "text" { 1 } else { 2 },
                "mask": frame.is_masked,
                "payloadData": msg
            }
        }));

        Ok(frame)
    }

    /// Send a binary message on a connection.
    pub async fn send_binary(&mut self, id: &str, data: &[u8]) -> Result<WebSocketFrame> {
        let conn = self
            .connections
            .get_mut(id)
            .ok_or_else(|| anyhow!("WebSocket connection not found: {}", id))?;

        let frame = conn.send_binary(data).await?;

        // Emit webSocketFrameSent event with base64-encoded payload
        let payload_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        self.emit_event("Network.webSocketFrameSent", json!({
            "requestId": id,
            "timestamp": frame.timestamp_ms,
            "response": {
                "opcode": 2, // binary
                "mask": frame.is_masked,
                "payloadData": payload_b64
            }
        }));

        Ok(frame)
    }

    /// Receive the next message on a connection.
    pub async fn recv(&mut self, id: &str) -> Result<Option<(WebSocketFrame, Vec<u8>)>> {
        let conn = self
            .connections
            .get_mut(id)
            .ok_or_else(|| anyhow!("WebSocket connection not found: {}", id))?;

        let result = conn.recv().await?;

        if let Some((ref frame, ref data)) = result {
            // Emit webSocketFrameReceived event
            let payload_data = if frame.opcode == "binary" {
                json!(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    data
                ))
            } else {
                json!(String::from_utf8_lossy(data).to_string())
            };

            self.emit_event("Network.webSocketFrameReceived", json!({
                "requestId": id,
                "timestamp": frame.timestamp_ms,
                "response": {
                    "opcode": if frame.opcode == "text" { 1 } else { 2 },
                    "mask": frame.is_masked,
                    "payloadData": payload_data
                }
            }));
        }

        Ok(result)
    }

    /// Close a connection by ID.
    pub async fn close(&mut self, id: &str) -> Result<()> {
        let conn = self
            .connections
            .get_mut(id)
            .ok_or_else(|| anyhow!("WebSocket connection not found: {}", id))?;

        let url = conn.url().to_string();
        conn.close().await?;

        // Emit webSocketClosed event
        self.emit_event("Network.webSocketClosed", json!({
            "requestId": id,
            "url": url,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64
        }));

        debug!(id = %id, "WebSocket connection closed");
        Ok(())
    }

    /// Close and remove a connection.
    pub async fn close_and_remove(&mut self, id: &str) -> Result<()> {
        self.close(id).await?;
        self.connections.remove(id);
        Ok(())
    }

    /// Close all connections.
    pub async fn close_all(&mut self) {
        let ids: Vec<String> = self.connections.keys().cloned().collect();
        for id in ids {
            let _ = self.close(&id).await;
        }
        self.connections.clear();
    }

    /// List all active connection IDs.
    pub fn connection_ids(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Get the number of active connections.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Get the configuration.
    pub fn config(&self) -> &WebSocketConfig {
        &self.config
    }

    /// Emit a CDP event if event bus is configured.
    fn emit_event(&self, method: &str, params: serde_json::Value) {
        if let Some(ref sender) = self.event_bus {
            let event = WebSocketEvent {
                method: method.to_string(),
                params,
            };
            let _ = sender.send(event);
            trace!(method = %method, "WebSocket CDP event emitted");
        }
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new(WebSocketConfig::default())
    }
}

/// Extract the origin (scheme + host + port) from a URL.
fn extract_origin(url: &str) -> String {
    if let Ok(parsed) = url::Url::parse(url) {
        format!(
            "{}://{}{}",
            parsed.scheme(),
            parsed.host_str().unwrap_or(""),
            parsed.port().map_or(String::new(), |p| format!(":{}", p))
        )
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WebSocketConfig::default();
        assert_eq!(config.max_per_origin, 6);
        assert_eq!(config.connect_timeout_secs, 30);
        assert_eq!(config.max_message_size, 10 * 1024 * 1024);
        assert!(config.block_private_ips);
        assert!(config.block_loopback);
    }

    #[test]
    fn test_manager_creation() {
        let manager = WebSocketManager::new(WebSocketConfig::default());
        assert_eq!(manager.connection_count(), 0);
    }

    #[test]
    fn test_extract_origin() {
        assert_eq!(
            extract_origin("wss://example.com/path"),
            "wss://example.com"
        );
        assert_eq!(
            extract_origin("ws://localhost:8080/ws"),
            "ws://localhost:8080"
        );
        // Default port (443) is normalized away by url::Url::parse
        assert_eq!(
            extract_origin("wss://api.example.com:443/v1/ws"),
            "wss://api.example.com"
        );
    }
}
