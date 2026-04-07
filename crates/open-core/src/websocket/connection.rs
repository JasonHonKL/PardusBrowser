//! Individual WebSocket connection handling.
//!
//! Wraps `tokio-tungstenite` to provide a clean async API for
//! WebSocket connections with frame tracking and event emission.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio_tungstenite::{connect_async, WebSocketStream};
use tungstenite::protocol::Message;
use url::Url;

/// Unique identifier for a WebSocket connection.
pub type ConnectionId = String;

/// Direction of a WebSocket frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    Sent,
    Received,
}

/// A WebSocket frame for logging/event emission.
#[derive(Debug, Clone)]
pub struct WebSocketFrame {
    pub opcode: String,
    pub payload_length: usize,
    pub is_masked: bool,
    pub timestamp_ms: u64,
    pub direction: FrameDirection,
}

/// Statistics for a WebSocket connection.
#[derive(Debug, Clone, Default)]
pub struct WebSocketStats {
    pub frames_sent: usize,
    pub frames_received: usize,
    pub bytes_sent: usize,
    pub bytes_received: usize,
    pub created_at: Option<Instant>,
}

/// An active WebSocket connection.
pub struct WebSocketConnection {
    /// Unique identifier for this connection.
    pub id: ConnectionId,
    /// The WebSocket URL (ws:// or wss://).
    pub url: String,
    /// Write half of the WebSocket stream.
    sink: SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>,
    /// Read half of the WebSocket stream.
    stream: SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    /// Connection statistics.
    pub stats: Arc<Mutex<WebSocketStats>>,
    /// Whether the connection is closed.
    is_closed: bool,
}

impl WebSocketConnection {
    /// Connect to a WebSocket endpoint.
    ///
    /// Validates the URL for SSRF protection.
    pub async fn connect(
        url: &str,
        block_private_ips: bool,
        block_loopback: bool,
        connect_timeout_secs: u64,
    ) -> Result<Self> {
        // Parse URL
        let parsed_url = Url::parse(url)
            .map_err(|e| anyhow!("Invalid URL '{}': {}", url, e))?;

        // Ensure ws:// or wss:// scheme
        let scheme = parsed_url.scheme().to_lowercase();
        if !matches!(scheme.as_str(), "ws" | "wss") {
            return Err(anyhow!(
                "Invalid WebSocket URL scheme '{}'. Expected 'ws' or 'wss'.",
                scheme
            ));
        }

        // Apply SSRF protection checks
        if let Some(host) = parsed_url.host_str() {
            // Try to parse as IP and validate
            let ip_host = if host.starts_with('[') && host.ends_with(']') {
                &host[1..host.len() - 1]
            } else {
                host
            };

            if let Ok(ip) = ip_host.parse::<IpAddr>() {
                if block_loopback && ip.is_loopback() {
                    return Err(anyhow!("Loopback address {} is blocked by security policy", ip));
                }
                if ip.is_multicast() {
                    return Err(anyhow!("Multicast address {} is blocked by security policy", ip));
                }
                if block_private_ips && is_private_ip(&ip) {
                    return Err(anyhow!("Private IP address {} is blocked by security policy", ip));
                }
                if is_link_local_ip(&ip) {
                    return Err(anyhow!("Link-local address {} is blocked by security policy", ip));
                }
            }

            // Block common metadata endpoints
            let blocked_hosts = [
                "localhost",
                "metadata.google.internal",
                "169.254.169.254",
                "100.100.100.200",
            ];
            for blocked in &blocked_hosts {
                if host.eq_ignore_ascii_case(blocked) {
                    return Err(anyhow!("Host '{}' is blocked by security policy", host));
                }
            }
        }

        // Generate unique connection ID using URL hash
        let id = generate_connection_id(url);

        // Connect with timeout
        let connect_future = connect_async(url);
        let (stream, _response) = tokio::time::timeout(
            std::time::Duration::from_secs(connect_timeout_secs),
            connect_future,
        )
        .await
        .map_err(|_| anyhow!("WebSocket connection timeout for {}", url))?
        .map_err(|e| anyhow!("WebSocket connection failed for {}: {}", url, e))?;

        // Split into sink and stream
        let (sink, stream) = stream.split();

        let stats = Arc::new(Mutex::new(WebSocketStats {
            created_at: Some(Instant::now()),
            ..Default::default()
        }));

        tracing::info!(id = %id, url = %url, "WebSocket connection established");

        Ok(Self {
            id,
            url: url.to_string(),
            sink,
            stream,
            stats,
            is_closed: false,
        })
    }

    /// Send a text message over the WebSocket.
    pub async fn send_text(&mut self, msg: &str) -> Result<WebSocketFrame> {
        if self.is_closed {
            return Err(anyhow!("WebSocket connection is closed"));
        }

        let payload_length = msg.len();
        self.sink
            .send(Message::Text(msg.to_string().into()))
            .await
            .map_err(|e| anyhow!("WebSocket send failed: {}", e))?;

        let frame = WebSocketFrame {
            opcode: "text".to_string(),
            payload_length,
            is_masked: true,
            timestamp_ms: current_timestamp_ms(),
            direction: FrameDirection::Sent,
        };

        let mut stats = self.stats.lock();
        stats.frames_sent += 1;
        stats.bytes_sent += payload_length;
        drop(stats);

        tracing::trace!(id = %self.id, len = payload_length, "WebSocket text frame sent");
        Ok(frame)
    }

    /// Send a binary message over the WebSocket.
    pub async fn send_binary(&mut self, data: &[u8]) -> Result<WebSocketFrame> {
        if self.is_closed {
            return Err(anyhow!("WebSocket connection is closed"));
        }

        let payload_length = data.len();
        self.sink
            .send(Message::Binary(data.to_vec().into()))
            .await
            .map_err(|e| anyhow!("WebSocket send failed: {}", e))?;

        let frame = WebSocketFrame {
            opcode: "binary".to_string(),
            payload_length,
            is_masked: true,
            timestamp_ms: current_timestamp_ms(),
            direction: FrameDirection::Sent,
        };

        let mut stats = self.stats.lock();
        stats.frames_sent += 1;
        stats.bytes_sent += payload_length;
        drop(stats);

        tracing::trace!(id = %self.id, len = payload_length, "WebSocket binary frame sent");
        Ok(frame)
    }

    /// Receive the next message from the WebSocket.
    ///
    /// Returns `None` if the connection is closed.
    pub async fn recv(&mut self) -> Result<Option<(WebSocketFrame, Vec<u8>)>> {
        if self.is_closed {
            return Ok(None);
        }

        let msg = match self.stream.next().await {
            Some(Ok(msg)) => msg,
            Some(Err(e)) => {
                tracing::warn!(id = %self.id, error = %e, "WebSocket receive error");
                return Err(anyhow!("WebSocket receive failed: {}", e));
            }
            None => {
                // Stream closed
                self.is_closed = true;
                return Ok(None);
            }
        };

        match msg {
            Message::Text(text) => {
                let payload_length = text.len();
                let frame = WebSocketFrame {
                    opcode: "text".to_string(),
                    payload_length,
                    is_masked: false,
                    timestamp_ms: current_timestamp_ms(),
                    direction: FrameDirection::Received,
                };

                let mut stats = self.stats.lock();
                stats.frames_received += 1;
                stats.bytes_received += payload_length;
                drop(stats);

                tracing::trace!(id = %self.id, len = payload_length, "WebSocket text frame received");
                Ok(Some((frame, text.as_bytes().to_vec())))
            }
            Message::Binary(data) => {
                let payload_length = data.len();
                let frame = WebSocketFrame {
                    opcode: "binary".to_string(),
                    payload_length,
                    is_masked: false,
                    timestamp_ms: current_timestamp_ms(),
                    direction: FrameDirection::Received,
                };

                let mut stats = self.stats.lock();
                stats.frames_received += 1;
                stats.bytes_received += payload_length;
                drop(stats);

                tracing::trace!(id = %self.id, len = payload_length, "WebSocket binary frame received");
                Ok(Some((frame, data.to_vec())))
            }
            Message::Ping(data) => {
                // Respond with pong
                let _ = self.sink.send(Message::Pong(data)).await;
                Ok(None) // Don't surface ping/pong to caller
            }
            Message::Pong(_) => {
                // Ignore pong
                Ok(None)
            }
            Message::Close(frame) => {
                tracing::debug!(id = %self.id, ?frame, "WebSocket close frame received");
                self.is_closed = true;
                Ok(None)
            }
            Message::Frame(_) => {
                // Raw frame, not typically used
                Ok(None)
            }
        }
    }

    /// Receive the next text message (convenience method).
    pub async fn recv_text(&mut self) -> Result<Option<String>> {
        loop {
            let (frame, data) = match self.recv().await? {
                Some(result) => result,
                None => return Ok(None),
            };

            if frame.opcode == "text" {
                return Ok(Some(String::from_utf8_lossy(&data).to_string()));
            }
            // Skip non-text frames
        }
    }

    /// Close the WebSocket connection gracefully.
    pub async fn close(&mut self) -> Result<()> {
        if self.is_closed {
            return Ok(());
        }

        tracing::debug!(id = %self.id, url = %self.url, "Closing WebSocket connection");

        self.sink
            .close()
            .await
            .map_err(|e| anyhow!("WebSocket close failed: {}", e))?;

        self.is_closed = true;
        Ok(())
    }

    /// Check if the connection is closed.
    pub fn is_closed(&self) -> bool {
        self.is_closed
    }

    /// Get the connection URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the connection ID.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for WebSocketConnection {
    fn drop(&mut self) {
        if !self.is_closed {
            tracing::debug!(id = %self.id, "WebSocket connection dropped without explicit close");
        }
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Generate a unique connection ID from a URL.
fn generate_connection_id(url: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("ws-{:016x}", hasher.finish())
}

/// Get current timestamp in milliseconds.
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis() as u64
}

/// Check if an IP address is private (RFC 1918 for IPv4, unique local for IPv6).
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => is_private_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_private_ipv6(ipv6),
    }
}

/// Check if an IPv4 address is private (RFC 1918).
fn is_private_ipv4(ip: &std::net::Ipv4Addr) -> bool {
    let octets = ip.octets();

    // 10.0.0.0/8
    if octets[0] == 10 {
        return true;
    }

    // 172.16.0.0/12 (172.16.0.0 - 172.31.255.255)
    if octets[0] == 172 && (16..=31).contains(&octets[1]) {
        return true;
    }

    // 192.168.0.0/16
    if octets[0] == 192 && octets[1] == 168 {
        return true;
    }

    false
}

/// Check if an IPv6 address is private (unique local fc00::/7).
fn is_private_ipv6(ip: &std::net::Ipv6Addr) -> bool {
    let segments = ip.segments();
    // Unique local addresses: fc00::/7 (fc00:: - fdff::)
    (segments[0] & 0xfe00) == 0xfc00
}

/// Check if an IP address is link-local.
fn is_link_local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // 169.254.0.0/16
            octets[0] == 169 && octets[1] == 254
        }
        IpAddr::V6(ipv6) => {
            // fe80::/10
            let segments = ipv6.segments();
            segments[0] & 0xffc0 == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_direction() {
        assert_eq!(FrameDirection::Sent, FrameDirection::Sent);
        assert_ne!(FrameDirection::Sent, FrameDirection::Received);
    }

    #[test]
    fn test_websocket_stats_default() {
        let stats = WebSocketStats::default();
        assert_eq!(stats.frames_sent, 0);
        assert_eq!(stats.frames_received, 0);
        assert!(stats.created_at.is_none());
    }

    #[test]
    fn test_generate_connection_id() {
        let id1 = generate_connection_id("wss://example.com/ws");
        let id2 = generate_connection_id("wss://example.com/ws");
        let id3 = generate_connection_id("wss://other.com/ws");

        assert_eq!(id1, id2); // Same URL = same ID
        assert_ne!(id1, id3); // Different URL = different ID
        assert!(id1.starts_with("ws-"));
    }

    #[test]
    fn test_is_private_ip() {
        // Private IPv4
        assert!(is_private_ip(&"10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"192.168.1.1".parse().unwrap()));

        // Public IPv4
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"1.1.1.1".parse().unwrap()));

        // Private IPv6 (unique local)
        assert!(is_private_ip(&"fc00::1".parse().unwrap()));
        assert!(is_private_ip(&"fd00::1".parse().unwrap()));

        // Public IPv6
        assert!(!is_private_ip(&"2001:4860:4860::8888".parse().unwrap()));
    }

    #[test]
    fn test_is_link_local_ip() {
        // Link-local IPv4
        assert!(is_link_local_ip(&"169.254.1.1".parse().unwrap()));
        assert!(!is_link_local_ip(&"192.168.1.1".parse().unwrap()));

        // Link-local IPv6
        assert!(is_link_local_ip(&"fe80::1".parse().unwrap()));
        assert!(!is_link_local_ip(&"2001::1".parse().unwrap()));
    }
}
