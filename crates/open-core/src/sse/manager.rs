//! Thread-safe SSE connection manager.
//!
//! Stores active SSE connections and provides methods to open/close,
//! poll events, and drain all pending events as JS dispatch code.

use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use parking_lot::Mutex;

use crate::sse::client::{spawn_sse_connection, SSE_CLOSED, SSE_OPEN};
use crate::sse::parser::SseEvent;
use crate::url_policy::UrlPolicy;

struct ConnectionEntry {
    url: String,
    event_rx: Mutex<std::sync::mpsc::Receiver<SseEvent>>,
    ready_state: std::sync::Arc<std::sync::atomic::AtomicU8>,
    closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Manages multiple SSE connections. Stored in deno_core `OpState`.
///
/// Thread-safe via `DashMap`. Events are drained during the JS event loop
/// to dispatch SSE events to JavaScript `EventSource` instances.
pub struct SseManager {
    connections: DashMap<u64, ConnectionEntry>,
    next_id: AtomicU64,
    url_policy: UrlPolicy,
}

impl SseManager {
    pub fn new(url_policy: UrlPolicy) -> Self {
        Self {
            connections: DashMap::new(),
            next_id: AtomicU64::new(1),
            url_policy,
        }
    }

    /// Open a new SSE connection. Returns the connection ID.
    pub fn open(&self, url: String) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let handle = spawn_sse_connection(id, url.clone(), self.url_policy.clone());

        self.connections.insert(
            id,
            ConnectionEntry {
                url,
                event_rx: Mutex::new(handle.event_rx),
                ready_state: handle.ready_state,
                closed: handle.closed,
            },
        );

        id
    }

    /// Close an SSE connection and remove it from the manager.
    pub fn close(&self, id: u64) {
        if let Some(entry) = self.connections.remove(&id) {
            entry.1.closed.store(true, Ordering::Relaxed);
            entry.1.ready_state.store(SSE_CLOSED, Ordering::Relaxed);
        }
    }

    /// Get the ready state of a connection.
    pub fn ready_state(&self, id: u64) -> u8 {
        self.connections
            .get(&id)
            .map(|e| e.ready_state.load(Ordering::Relaxed))
            .unwrap_or(SSE_CLOSED)
    }

    /// Get the URL of a connection.
    pub fn url(&self, id: u64) -> Option<String> {
        self.connections.get(&id).map(|e| e.url.clone())
    }

    /// Non-blocking poll of a single connection's events.
    pub fn poll_event(&self, id: u64) -> Option<SseEvent> {
        let entry = self.connections.get(&id)?;
        let rx = entry.event_rx.lock();
        rx.try_recv().ok()
    }

    /// Drain all pending events from all connections and generate JS dispatch code.
    ///
    /// The generated JS calls `__sse_dispatch(id, eventType, eventInit)` for each event,
    /// which is defined in `bootstrap.js` and dispatches to the correct `EventSource` instance.
    pub fn drain_events_js(&self) -> String {
        let mut js = String::new();

        for entry in self.connections.iter() {
            let id = *entry.key();
            let url = &entry.value().url;
            let origin = url::Url::parse(url)
                .map(|u| u.origin().ascii_serialization())
                .unwrap_or_else(|_| String::new());

            loop {
                let event = {
                    let rx = entry.value().event_rx.lock();
                    rx.try_recv().ok()
                };

                let event = match event {
                    Some(e) => e,
                    None => break,
                };

                let dispatch_type = match event.event_type.as_str() {
                    "__open" => "open",
                    "__error" => "error",
                    other => other,
                };

                let data_json =
                    serde_json::to_string(&event.data).unwrap_or_else(|_| "\"\"".to_string());
                let id_json = event
                    .id
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "\"\"".to_string()))
                    .unwrap_or_else(|| "null".to_string());
                let origin_json =
                    serde_json::to_string(&origin).unwrap_or_else(|_| "\"\"".to_string());

                let ready_state = match event.event_type.as_str() {
                    "__open" => SSE_OPEN,
                    "__error" => SSE_CLOSED,
                    _ => SSE_OPEN,
                };

                let type_json =
                    serde_json::to_string(&dispatch_type).unwrap_or_else(|_| "\"\"".to_string());

                use std::fmt::Write;
                write!(
                    &mut js,
                    "try{{__sse_dispatch({},{},{{{}}},{})}}catch(e){{}}\n",
                    id,
                    type_json,
                    format!(
                        "data:{},lastEventId:{},origin:{}",
                        data_json, id_json, origin_json
                    ),
                    ready_state,
                )
                .ok();
            }
        }

        js
    }

    /// Close all connections and clean up.
    pub fn close_all(&self) {
        for entry in self.connections.iter() {
            entry.value().closed.store(true, Ordering::Relaxed);
            entry
                .value()
                .ready_state
                .store(SSE_CLOSED, Ordering::Relaxed);
        }
        self.connections.clear();
    }
}

impl Drop for SseManager {
    fn drop(&mut self) {
        self.close_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sse::{SSE_CLOSED, SSE_CONNECTING, SSE_OPEN};

    #[test]
    fn test_open_close_connection() {
        let manager = SseManager::new(UrlPolicy::default());
        let id = manager.open("http://localhost:1/events".to_string());
        assert!(id > 0);
        assert_eq!(manager.ready_state(id), SSE_CONNECTING);
        manager.close(id);
        assert_eq!(manager.ready_state(id), SSE_CLOSED);
    }

    #[test]
    fn test_close_nonexistent_id() {
        let manager = SseManager::new(UrlPolicy::default());
        manager.close(999);
        assert_eq!(manager.ready_state(999), SSE_CLOSED);
    }

    #[test]
    fn test_url_retrieved() {
        let manager = SseManager::new(UrlPolicy::default());
        let id = manager.open("http://example.com/stream".to_string());
        assert_eq!(
            manager.url(id).as_deref(),
            Some("http://example.com/stream")
        );
        manager.close(id);
    }

    #[test]
    fn test_drain_events_empty() {
        let manager = SseManager::new(UrlPolicy::default());
        let js = manager.drain_events_js();
        assert!(js.is_empty());
    }

    #[test]
    fn test_multiple_connections_sequential_ids() {
        let manager = SseManager::new(UrlPolicy::default());
        let id1 = manager.open("http://example.com/1".to_string());
        let id2 = manager.open("http://example.com/2".to_string());
        let id3 = manager.open("http://example.com/3".to_string());
        assert_eq!(id2, id1 + 1);
        assert_eq!(id3, id2 + 1);
        manager.close_all();
    }

    #[test]
    fn test_url_nonexistent_id() {
        let manager = SseManager::new(UrlPolicy::default());
        assert!(manager.url(999).is_none());
    }

    #[test]
    fn test_poll_event_nonexistent() {
        let manager = SseManager::new(UrlPolicy::default());
        assert!(manager.poll_event(999).is_none());
    }

    #[test]
    fn test_poll_event_empty() {
        let manager = SseManager::new(UrlPolicy::default());
        let id = manager.open("http://localhost:1/ev".to_string());
        let result = manager.poll_event(id);
        assert!(result.is_none());
        manager.close(id);
    }

    #[test]
    fn test_close_all_closes_everything() {
        let manager = SseManager::new(UrlPolicy::default());
        let id1 = manager.open("http://example.com/1".to_string());
        let id2 = manager.open("http://example.com/2".to_string());
        manager.close_all();
        assert_eq!(manager.ready_state(id1), SSE_CLOSED);
        assert_eq!(manager.ready_state(id2), SSE_CLOSED);
    }

    #[test]
    fn test_close_idempotent() {
        let manager = SseManager::new(UrlPolicy::default());
        let id = manager.open("http://localhost:1/ev".to_string());
        manager.close(id);
        manager.close(id);
        manager.close(id);
        assert_eq!(manager.ready_state(id), SSE_CLOSED);
    }

    #[test]
    fn test_drain_events_no_pending_events() {
        let manager = SseManager::new(UrlPolicy::default());
        let id = manager.open("http://localhost:1/ev".to_string());
        let js = manager.drain_events_js();
        assert!(js.is_empty());
        manager.close(id);
    }

    #[test]
    fn test_drain_events_js_format_message() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        let url = "http://example.com/stream".to_string();
        let id = 42u64;

        manager.connections.insert(
            id,
            ConnectionEntry {
                url: url.clone(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "hello world".to_string(),
            id: Some("abc".to_string()),
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("__sse_dispatch(42,\"message\""));
        assert!(js.contains("data:\"hello world\""));
        assert!(js.contains("lastEventId:\"abc\""));
        assert!(js.contains("origin:\"http://example.com\""));
        assert!(js.contains(",1)}"));

        manager.close(id);
    }

    #[test]
    fn test_drain_events_js_format_open_event() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "__open".to_string(),
            data: String::new(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("__sse_dispatch(1,\"open\""));
        assert!(js.contains("data:\"\""));
        assert!(js.contains(",1)}"));

        manager.close(1);
    }

    #[test]
    fn test_drain_events_js_format_error_event() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            5,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_CLOSED)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            },
        );

        tx.send(SseEvent {
            event_type: "__error".to_string(),
            data: "Connection refused".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("__sse_dispatch(5,\"error\""));
        assert!(js.contains("data:\"Connection refused\""));
        assert!(js.contains(",2)}"));

        manager.close(5);
    }

    #[test]
    fn test_drain_events_js_custom_event_type() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            10,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "notification".to_string(),
            data: "new message".to_string(),
            id: Some("n1".to_string()),
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("__sse_dispatch(10,\"notification\""));
        assert!(js.contains("data:\"new message\""));
        assert!(js.contains("lastEventId:\"n1\""));

        manager.close(10);
    }

    #[test]
    fn test_drain_events_js_multiple_events_same_connection() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "first".to_string(),
            id: None,
            retry: None,
        }).ok();
        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "second".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        let dispatch_count = js.matches("__sse_dispatch").count();
        assert_eq!(dispatch_count, 2);

        manager.close(1);
    }

    #[test]
    fn test_drain_events_js_escapes_special_chars() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "line1\nline2".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("data:\"line1\\nline2\""));

        manager.close(1);
    }

    #[test]
    fn test_drain_events_js_multiple_connections() {
        use std::sync::mpsc;

        let (tx1, rx1) = mpsc::channel();
        let (tx2, rx2) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://a.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx1),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );
        manager.connections.insert(
            2,
            ConnectionEntry {
                url: "http://b.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx2),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx1.send(SseEvent {
            event_type: "message".to_string(),
            data: "from_a".to_string(),
            id: None,
            retry: None,
        }).ok();
        tx2.send(SseEvent {
            event_type: "message".to_string(),
            data: "from_b".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("__sse_dispatch(1"));
        assert!(js.contains("from_a"));
        assert!(js.contains("origin:\"http://a.com\""));
        assert!(js.contains("__sse_dispatch(2"));
        assert!(js.contains("from_b"));
        assert!(js.contains("origin:\"http://b.com\""));

        manager.close_all();
    }

    #[tokio::test]
    async fn test_drain_events_js_from_real_server() {
        use std::time::Duration;
        use tokio::io::AsyncWriteExt;
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            if let Ok((mut stream, _addr)) = listener.accept().await {
                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n";
                stream.write_all(response.as_bytes()).await.ok();
                stream.write_all(b"data: hello\n\n").await.ok();
                stream.write_all(b"data: world\n\n").await.ok();
                stream.flush().await.ok();
            }
        });

        let manager = SseManager::new(UrlPolicy::permissive());
        let id = manager.open(format!("http://127.0.0.1:{}/events", port));

        let js = wait_for_manager_events(&manager, Duration::from_secs(5)).await;
        assert!(!js.is_empty(), "should have events from the server");
        assert!(js.contains("\"open\""));
        assert!(js.contains("hello"));
        assert!(js.contains("world"));

        manager.close(id);
    }

    async fn wait_for_manager_events(manager: &SseManager, timeout: std::time::Duration) -> String {
        let start = std::time::Instant::now();
        let mut all_js = String::new();
        while start.elapsed() < timeout {
            let js = manager.drain_events_js();
            if !js.is_empty() {
                all_js.push_str(&js);
                all_js.push(';');
            }
            if all_js.contains("hello") && all_js.contains("world") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        all_js
    }

    #[test]
    fn test_new_manager_empty_connections() {
        let manager = SseManager::new(UrlPolicy::default());
        assert!(manager.connections.is_empty());
        assert_eq!(manager.next_id.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_id_increments_monotonically() {
        let manager = SseManager::new(UrlPolicy::default());
        let ids: Vec<u64> = (0..10).map(|_| {
            let id = manager.open("http://localhost:1/ev".to_string());
            manager.close(id);
            id
        }).collect();
        for window in ids.windows(2) {
            assert!(window[1] > window[0], "IDs should be monotonically increasing");
        }
    }

    #[test]
    fn test_drain_events_js_no_id_field() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_OPEN)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            },
        );

        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "no-id".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(js.contains("lastEventId:null"));
        manager.close(1);
    }

    #[test]
    fn test_drain_events_after_close() {
        use std::sync::mpsc;

        let (tx, rx) = mpsc::channel();
        let manager = SseManager::new(UrlPolicy::default());

        manager.connections.insert(
            1,
            ConnectionEntry {
                url: "http://example.com/sse".to_string(),
                event_rx: parking_lot::Mutex::new(rx),
                ready_state: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(SSE_CLOSED)),
                closed: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
            },
        );

        tx.send(SseEvent {
            event_type: "message".to_string(),
            data: "queued".to_string(),
            id: None,
            retry: None,
        }).ok();

        let js = manager.drain_events_js();
        assert!(!js.is_empty(), "should drain events even after close");
        assert!(js.contains("queued"));
    }
}
