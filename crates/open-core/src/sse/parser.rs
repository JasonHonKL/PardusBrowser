//! SSE (Server-Sent Events) protocol parser.
//!
//! Implements the text/event-stream format per the HTML Living Standard.
//! Handles partial input (streaming), BOM, multi-line data, comments,
//! and all field types (event, data, id, retry).

use serde::{Deserialize, Serialize};

/// A parsed SSE event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SseEvent {
    /// Event type (defaults to "message" if not specified).
    pub event_type: String,
    /// Event data (multiline data fields joined with \n).
    pub data: String,
    /// Event ID (from the `id:` field).
    pub id: Option<String>,
    /// Reconnection interval in milliseconds (from the `retry:` field).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<u64>,
}

/// Streaming SSE parser that handles partial input across chunks.
///
/// ```rust
/// # use open_core::sse::parser::{SseParser, SseEvent};
/// let mut p = SseParser::new();
/// let events = p.feed("data: hello\n\n");
/// assert_eq!(events[0].data, "hello");
/// ```
pub struct SseParser {
    buffer: String,
    event_type: Option<String>,
    data_lines: Vec<String>,
    current_id: Option<String>,
    current_retry: Option<u64>,
    last_event_id: Option<String>,
    default_reconnect_ms: u64,
    started: bool,
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            event_type: None,
            data_lines: Vec::new(),
            current_id: None,
            current_retry: None,
            last_event_id: None,
            default_reconnect_ms: 3000,
            started: false,
        }
    }

    pub fn last_event_id(&self) -> Option<&str> {
        self.last_event_id.as_deref()
    }

    pub fn reconnect_ms(&self) -> u64 {
        self.default_reconnect_ms
    }

    /// Feed a chunk of data to the parser. Returns any complete events.
    pub fn feed(&mut self, chunk: &str) -> Vec<SseEvent> {
        let chunk = if !self.started {
            self.started = true;
            chunk.strip_prefix('\u{FEFF}').unwrap_or(chunk)
        } else {
            chunk
        };

        self.buffer.push_str(chunk);
        let mut events = Vec::new();

        loop {
            let (newline_pos, skip) = match find_newline(&self.buffer) {
                Some(pair) => pair,
                None => break,
            };

            let line = self.buffer[..newline_pos].to_string();
            self.buffer = self.buffer[newline_pos + skip..].to_string();

            if let Some(event) = self.process_line(&line) {
                events.push(event);
            }
        }

        events
    }

    /// Flush any remaining buffered data as a final event.
    pub fn finish(&mut self) -> Option<SseEvent> {
        if !self.buffer.is_empty() {
            let line = self.buffer.clone();
            if let Some(event) = self.process_line(&line) {
                self.buffer.clear();
                return Some(event);
            }
            self.buffer.clear();
        }
        if !self.data_lines.is_empty() {
            let event = SseEvent {
                event_type: self
                    .event_type
                    .take()
                    .unwrap_or_else(|| "message".to_string()),
                data: self.data_lines.join("\n"),
                id: self.current_id.take(),
                retry: self.current_retry.take(),
            };
            self.data_lines.clear();
            return Some(event);
        }
        None
    }

    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch_event();
        }

        if line.starts_with(':') {
            return None;
        }

        let (field, value) = if let Some(colon_pos) = line.find(':') {
            let field = &line[..colon_pos];
            let rest = &line[colon_pos + 1..];
            let value = rest.strip_prefix(' ').unwrap_or(rest);
            (field, value)
        } else {
            (line, "")
        };

        match field {
            "event" => {
                self.event_type = Some(value.to_string());
            }
            "data" => {
                self.data_lines.push(value.to_string());
            }
            "id" => {
                if !value.contains('\0') {
                    self.current_id = Some(value.to_string());
                    self.last_event_id = Some(value.to_string());
                }
            }
            "retry" => {
                if let Ok(ms) = value.parse::<u64>() {
                    self.current_retry = Some(ms);
                    self.default_reconnect_ms = ms;
                }
            }
            _ => {}
        }

        None
    }

    fn dispatch_event(&mut self) -> Option<SseEvent> {
        if self.data_lines.is_empty() {
            self.event_type = None;
            self.current_id = None;
            self.current_retry = None;
            return None;
        }

        let event = SseEvent {
            event_type: self
                .event_type
                .take()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "message".to_string()),
            data: self.data_lines.join("\n"),
            id: self.current_id.take(),
            retry: self.current_retry.take(),
        };
        self.data_lines.clear();
        Some(event)
    }
}

fn find_newline(s: &str) -> Option<(usize, usize)> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'\r' => {
                let skip = if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                    2
                } else {
                    1
                };
                return Some((i, skip));
            }
            b'\n' => return Some((i, 1)),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: hello\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "message");
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_event_with_type() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: custom\ndata: test\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "custom");
        assert_eq!(events[0].data, "test");
    }

    #[test]
    fn test_multiline_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: line1\ndata: line2\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "line1\nline2");
    }

    #[test]
    fn test_event_with_id() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: 42\ndata: msg\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id.as_deref(), Some("42"));
        assert_eq!(parser.last_event_id(), Some("42"));
    }

    #[test]
    fn test_last_event_id_persists_across_events() {
        let mut parser = SseParser::new();
        parser.feed("id: 1\ndata: first\n\n");
        parser.feed("data: second\n\n");
        assert_eq!(parser.last_event_id(), Some("1"));
    }

    #[test]
    fn test_retry_field() {
        let mut parser = SseParser::new();
        let events = parser.feed("retry: 5000\ndata: test\n\n");
        assert_eq!(parser.reconnect_ms(), 5000);
        assert_eq!(events[0].retry, Some(5000));
    }

    #[test]
    fn test_comment_ignored() {
        let mut parser = SseParser::new();
        let events = parser.feed(": this is a comment\ndata: real\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "real");
    }

    #[test]
    fn test_cr_lf_newlines() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: hello\r\n\r\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_cr_only_newlines() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: hello\r\r");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello");
    }

    #[test]
    fn test_bom_stripped() {
        let mut parser = SseParser::new();
        let events = parser.feed("\u{FEFF}data: bom test\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "bom test");
    }

    #[test]
    fn test_no_data_no_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: type\nid: 123\n\n");
        assert!(events.is_empty());
    }

    #[test]
    fn test_empty_data_field() {
        let mut parser = SseParser::new();
        let events = parser.feed("data\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "");
    }

    #[test]
    fn test_id_with_null_byte_rejected() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: bad\0id\ndata: test\n\n");
        assert_eq!(events.len(), 1);
        assert!(events[0].id.is_none());
    }

    #[test]
    fn test_data_field_leading_space_stripped() {
        let mut parser = SseParser::new();
        let events = parser.feed("data:  spaced\n\n");
        assert_eq!(events[0].data, " spaced");
    }

    #[test]
    fn test_data_field_no_space_preserved() {
        let mut parser = SseParser::new();
        let events = parser.feed("data:spaced\n\n");
        assert_eq!(events[0].data, "spaced");
    }

    #[test]
    fn test_chunked_input() {
        let mut parser = SseParser::new();
        assert!(parser.feed("data: hel").is_empty());
        assert!(parser.feed("lo wo").is_empty());
        let events = parser.feed("rld\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "hello world");
    }

    #[test]
    fn test_multiple_events() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: first\n\ndata: second\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].data, "first");
        assert_eq!(events[1].data, "second");
    }

    #[test]
    fn test_unknown_field_ignored() {
        let mut parser = SseParser::new();
        let events = parser.feed("unknown: field\ndata: ok\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "ok");
    }

    #[test]
    fn test_finish_flushes_partial_event() {
        let mut parser = SseParser::new();
        parser.feed("data: partial");
        let event = parser.finish();
        assert!(event.is_some());
        assert_eq!(event.unwrap().data, "partial");
    }

    #[test]
    fn test_finish_returns_none_when_empty() {
        let mut parser = SseParser::new();
        assert!(parser.finish().is_none());
    }

    #[test]
    fn test_field_without_colon() {
        let mut parser = SseParser::new();
        let events = parser.feed("data\nevent\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "");
        assert_eq!(events[0].event_type, "message");
    }

    #[test]
    fn test_json_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: {\"key\": \"value\"}\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data, "{\"key\": \"value\"}");
    }

    #[test]
    fn test_mixed_events_and_comments() {
        let mut parser = SseParser::new();
        let input = ": stream start\n\nevent: user\ndata: Alice\n\n: heartbeat\n\ndata: ping\n\n";
        let events = parser.feed(input);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "user");
        assert_eq!(events[0].data, "Alice");
        assert_eq!(events[1].event_type, "message");
        assert_eq!(events[1].data, "ping");
    }

    #[test]
    fn test_event_serialization() {
        let event = SseEvent {
            event_type: "message".to_string(),
            data: "hello world".to_string(),
            id: Some("abc".to_string()),
            retry: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event_type\":\"message\""));
        assert!(json.contains("\"data\":\"hello world\""));
        assert!(json.contains("\"id\":\"abc\""));
        assert!(!json.contains("retry"));
    }

    #[test]
    fn test_event_deserialization() {
        let json = r#"{"event_type":"custom","data":"test","id":"123","retry":5000}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "custom");
        assert_eq!(event.data, "test");
        assert_eq!(event.id.as_deref(), Some("123"));
        assert_eq!(event.retry, Some(5000));
    }

    #[test]
    fn test_event_deserialization_minimal() {
        let json = r#"{"event_type":"message","data":"hi","id":null}"#;
        let event: SseEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "message");
        assert_eq!(event.data, "hi");
        assert!(event.id.is_none());
        assert!(event.retry.is_none());
    }

    #[test]
    fn test_only_empty_lines_no_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("\n\n\n");
        assert!(events.is_empty());
    }

    #[test]
    fn test_multiple_empty_events_dispatched() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: a\n\ndata: b\n\ndata: c\n\ndata: d\n\n");
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].data, "a");
        assert_eq!(events[1].data, "b");
        assert_eq!(events[2].data, "c");
        assert_eq!(events[3].data, "d");
    }

    #[test]
    fn test_event_type_resets_between_events() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: alpha\ndata: 1\n\ndata: 2\n\nevent: beta\ndata: 3\n\n");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, "alpha");
        assert_eq!(events[0].data, "1");
        assert_eq!(events[1].event_type, "message");
        assert_eq!(events[1].data, "2");
        assert_eq!(events[2].event_type, "beta");
        assert_eq!(events[2].data, "3");
    }

    #[test]
    fn test_id_overwritten_by_subsequent_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("id: aaa\ndata: 1\n\nid: bbb\ndata: 2\n\n");
        assert_eq!(events[0].id.as_deref(), Some("aaa"));
        assert_eq!(events[1].id.as_deref(), Some("bbb"));
        assert_eq!(parser.last_event_id(), Some("bbb"));
    }

    #[test]
    fn test_retry_only_from_last_event() {
        let mut parser = SseParser::new();
        let events = parser.feed("retry: 1000\ndata: 1\n\nretry: 9000\ndata: 2\n\n");
        assert_eq!(events[0].retry, Some(1000));
        assert_eq!(events[1].retry, Some(9000));
        assert_eq!(parser.reconnect_ms(), 9000);
    }

    #[test]
    fn test_retry_invalid_ignored() {
        let mut parser = SseParser::new();
        let events = parser.feed("retry: abc\ndata: test\n\n");
        assert_eq!(parser.reconnect_ms(), 3000);
        assert!(events[0].retry.is_none());
    }

    #[test]
    fn test_retry_zero_accepted() {
        let mut parser = SseParser::new();
        parser.feed("retry: 0\ndata: x\n\n");
        assert_eq!(parser.reconnect_ms(), 0);
    }

    #[test]
    fn test_data_with_colon_in_value() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: time: 12:30:00\n\n");
        assert_eq!(events[0].data, "time: 12:30:00");
    }

    #[test]
    fn test_multiline_data_with_empty_data_line() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: line1\ndata:\ndata: line3\n\n");
        assert_eq!(events[0].data, "line1\n\nline3");
    }

    #[test]
    fn test_event_with_all_fields() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: update\nid: seq-99\nretry: 2000\ndata: payload\n\n");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "update");
        assert_eq!(events[0].data, "payload");
        assert_eq!(events[0].id.as_deref(), Some("seq-99"));
        assert_eq!(events[0].retry, Some(2000));
        assert_eq!(parser.last_event_id(), Some("seq-99"));
        assert_eq!(parser.reconnect_ms(), 2000);
    }

    #[test]
    fn test_event_type_empty_string_falls_back() {
        let mut parser = SseParser::new();
        let events = parser.feed("event:\ndata: test\n\n");
        assert_eq!(events[0].event_type, "message");
    }

    #[test]
    fn test_byte_by_byte_feeding() {
        let mut parser = SseParser::new();
        let input = "data: hello\n\n";
        let mut count = 0;
        for ch in input.chars() {
            count += parser.feed(&ch.to_string()).len();
        }
        assert_eq!(count, 1);
    }

    #[test]
    fn test_large_event_data() {
        let mut parser = SseParser::new();
        let large = "x".repeat(100_000);
        let events = parser.feed(&format!("data: {}\n\n", large));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data.len(), 100_000);
    }

    #[test]
    fn test_finish_after_complete_events() {
        let mut parser = SseParser::new();
        parser.feed("data: done\n\n");
        assert!(parser.finish().is_none());
    }

    #[test]
    fn test_finish_after_fields_no_data() {
        let mut parser = SseParser::new();
        parser.feed("event: orphan");
        assert!(parser.finish().is_none());
    }

    #[test]
    fn test_finish_after_partial_data() {
        let mut parser = SseParser::new();
        parser.feed("data: partial\nevent: custom\n");
        let event = parser.finish().unwrap();
        assert_eq!(event.event_type, "custom");
        assert_eq!(event.data, "partial");
    }

    #[test]
    fn test_default_reconnect_ms() {
        let parser = SseParser::new();
        assert_eq!(parser.reconnect_ms(), 3000);
    }

    #[test]
    fn test_default_last_event_id() {
        let parser = SseParser::new();
        assert!(parser.last_event_id().is_none());
    }

    #[test]
    fn test_comment_does_not_reset_event_type() {
        let mut parser = SseParser::new();
        let events = parser.feed("event: keep\ndata: a\n\n: comment\ndata: b\n\n");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "keep");
        assert_eq!(events[1].event_type, "message");
    }

    #[test]
    fn test_unicode_data() {
        let mut parser = SseParser::new();
        let events = parser.feed("data: hello \u{1F600} world\n\n");
        assert_eq!(events[0].data, "hello \u{1F600} world");
    }

    #[test]
    fn test_event_type_case_sensitive() {
        let mut parser = SseParser::new();
        let events = parser.feed("EVENT: ignored\ndata: test\n\n");
        assert_eq!(events[0].event_type, "message");
        assert_eq!(events[0].data, "test");
    }
}
