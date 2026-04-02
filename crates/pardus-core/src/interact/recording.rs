use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::browser::Browser;
use crate::interact::actions::InteractionResult;
use crate::interact::form::FormState;

/// A single recorded action in a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedAction {
    pub timestamp_ms: u64,
    pub step: usize,
    pub action_type: RecordedActionType,
    pub target: Option<String>,
    pub value: Option<String>,
    pub url_before: String,
    pub url_after: Option<String>,
    pub success: bool,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RecordedActionType {
    Navigate,
    Click,
    ClickById,
    Type,
    TypeById,
    Submit,
    Wait,
    Scroll,
    Toggle,
    SelectOption,
}

/// A recorded session: an ordered sequence of actions that can be replayed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecording {
    pub id: String,
    pub started_at: String,
    pub actions: Vec<RecordedAction>,
    pub metadata: HashMap<String, String>,
}

impl SessionRecording {
    pub fn new() -> Self {
        let id = blake3::hash(&std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes()
        ).to_hex()[..16].to_string();

        Self {
            id,
            started_at: chrono::Utc::now().to_rfc3339(),
            actions: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn record(&mut self, action: RecordedAction) {
        self.actions.push(action);
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn last_url(&self) -> Option<&str> {
        self.actions.iter().rev().find_map(|a| a.url_after.as_deref())
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

impl Default for SessionRecording {
    fn default() -> Self {
        Self::new()
    }
}

/// Recorder that wraps a Browser and records all interactions.
pub struct SessionRecorder {
    recording: SessionRecording,
    start_time: Instant,
}

impl SessionRecorder {
    pub fn new() -> Self {
        Self {
            recording: SessionRecording::new(),
            start_time: Instant::now(),
        }
    }

    pub fn with_metadata(self, key: &str, value: &str) -> Self {
        let rec = self.recording.with_metadata(key, value);
        Self { recording: rec, start_time: self.start_time }
    }

    pub fn recording(&self) -> &SessionRecording {
        &self.recording
    }

    pub fn into_recording(self) -> SessionRecording {
        self.recording
    }

    fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }

    fn next_step(&self) -> usize {
        self.recording.actions.len() + 1
    }

    pub fn record_navigate(&mut self, url: &str, success: bool) {
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Navigate,
            target: Some(url.to_string()),
            value: None,
            url_before: url.to_string(),
            url_after: if success { Some(url.to_string()) } else { None },
            success,
            duration_ms: None,
        });
    }

    pub fn record_click(&mut self, selector: &str, url_before: &str, result: &InteractionResult, duration: Duration) {
        let (url_after, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Click,
            target: Some(selector.to_string()),
            value: None,
            url_before: url_before.to_string(),
            url_after,
            success,
            duration_ms: Some(duration.as_millis() as u64),
        });
    }

    pub fn record_click_by_id(&mut self, id: usize, url_before: &str, result: &InteractionResult, duration: Duration) {
        let (url_after, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::ClickById,
            target: Some(id.to_string()),
            value: None,
            url_before: url_before.to_string(),
            url_after,
            success,
            duration_ms: Some(duration.as_millis() as u64),
        });
    }

    pub fn record_type(&mut self, selector: &str, value: &str, url_before: &str, result: &InteractionResult) {
        let (_, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Type,
            target: Some(selector.to_string()),
            value: Some(value.to_string()),
            url_before: url_before.to_string(),
            url_after: Some(url_before.to_string()),
            success,
            duration_ms: None,
        });
    }

    pub fn record_type_by_id(&mut self, id: usize, value: &str, url_before: &str, result: &InteractionResult) {
        let (_, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::TypeById,
            target: Some(id.to_string()),
            value: Some(value.to_string()),
            url_before: url_before.to_string(),
            url_after: Some(url_before.to_string()),
            success,
            duration_ms: None,
        });
    }

    pub fn record_submit(&mut self, selector: &str, url_before: &str, result: &InteractionResult, duration: Duration) {
        let (url_after, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Submit,
            target: Some(selector.to_string()),
            value: None,
            url_before: url_before.to_string(),
            url_after,
            success,
            duration_ms: Some(duration.as_millis() as u64),
        });
    }

    pub fn record_wait(&mut self, selector: &str, url_before: &str, found: bool) {
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Wait,
            target: Some(selector.to_string()),
            value: None,
            url_before: url_before.to_string(),
            url_after: Some(url_before.to_string()),
            success: found,
            duration_ms: None,
        });
    }

    pub fn record_scroll(&mut self, direction: &str, url_before: &str, result: &InteractionResult, duration: Duration) {
        let (url_after, success) = extract_result_url(result);
        self.recording.record(RecordedAction {
            timestamp_ms: self.elapsed_ms(),
            step: self.next_step(),
            action_type: RecordedActionType::Scroll,
            target: Some(direction.to_string()),
            value: None,
            url_before: url_before.to_string(),
            url_after,
            success,
            duration_ms: Some(duration.as_millis() as u64),
        });
    }
}

impl Default for SessionRecorder {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_result_url(result: &InteractionResult) -> (Option<String>, bool) {
    match result {
        InteractionResult::Navigated(page) => (Some(page.url.clone()), true),
        InteractionResult::Scrolled { url, .. } => (Some(url.clone()), true),
        InteractionResult::Typed { .. } => (None, true),
        InteractionResult::Toggled { .. } => (None, true),
        InteractionResult::Selected { .. } => (None, true),
        InteractionResult::WaitSatisfied { found, .. } => (None, *found),
        InteractionResult::ElementNotFound { reason: _, .. } => (None, false),
    }
}

/// Replay a recording against a Browser instance.
///
/// Executes each action sequentially. Stops on first failure if `stop_on_error` is true.
pub async fn replay(
    browser: &mut Browser,
    recording: &SessionRecording,
    stop_on_error: bool,
) -> Vec<ReplayStepResult> {
    let mut results = Vec::new();

    for action in &recording.actions {
        let step_result = replay_action(browser, action).await;
        let success = step_result.success;

        results.push(step_result);

        if !success && stop_on_error {
            break;
        }
    }

    results
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayStepResult {
    pub step: usize,
    pub action_type: RecordedActionType,
    pub success: bool,
    pub error: Option<String>,
}

async fn replay_action(browser: &mut Browser, action: &RecordedAction) -> ReplayStepResult {
    let target = match &action.target {
        Some(t) => t.as_str(),
        None => {
            return ReplayStepResult {
                step: action.step,
                action_type: action.action_type.clone(),
                success: false,
                error: Some("no target specified".to_string()),
            };
        }
    };

    match action.action_type {
        RecordedActionType::Navigate => {
            match browser.navigate(target).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::Click => {
            match browser.click(target).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::ClickById => {
            if let Ok(id) = target.parse::<usize>() {
                match browser.click_by_id(id).await {
                    Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                    Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
                }
            } else {
                ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some("invalid element ID".to_string()) }
            }
        }
        RecordedActionType::Type => {
            let value = action.value.as_deref().unwrap_or("");
            match browser.type_text(target, value).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::TypeById => {
            if let Ok(id) = target.parse::<usize>() {
                let value = action.value.as_deref().unwrap_or("");
                match browser.type_by_id(id, value).await {
                    Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                    Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
                }
            } else {
                ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some("invalid element ID".to_string()) }
            }
        }
        RecordedActionType::Submit => {
            let form_state = FormState::new();
            match browser.submit(target, &form_state).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::Wait => {
            match browser.wait_for(target, 5000).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::Scroll => {
            use crate::interact::ScrollDirection;
            let direction = match target {
                "down" => ScrollDirection::Down,
                "up" => ScrollDirection::Up,
                "to-top" => ScrollDirection::ToTop,
                "to-bottom" => ScrollDirection::ToBottom,
                _ => ScrollDirection::Down,
            };
            match browser.scroll(direction).await {
                Ok(_) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: true, error: None },
                Err(e) => ReplayStepResult { step: action.step, action_type: action.action_type.clone(), success: false, error: Some(e.to_string()) },
            }
        }
        RecordedActionType::Toggle | RecordedActionType::SelectOption => {
            ReplayStepResult {
                step: action.step,
                action_type: action.action_type.clone(),
                success: false,
                error: Some("toggle/select replay not yet supported".to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page::Page;

    #[test]
    fn test_recording_new() {
        let rec = SessionRecording::new();
        assert!(!rec.id.is_empty());
        assert!(rec.actions.is_empty());
        assert!(rec.started_at.contains("T") || rec.started_at.contains("-"));
    }

    #[test]
    fn test_recording_with_metadata() {
        let rec = SessionRecording::new()
            .with_metadata("task", "login-test");
        assert_eq!(rec.metadata.get("task"), Some(&"login-test".to_string()));
    }

    #[test]
    fn test_record_actions() {
        let mut recorder = SessionRecorder::new();
        recorder.record_navigate("https://example.com", true);

        assert_eq!(recorder.recording().action_count(), 1);
        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Navigate);
        assert!(action.success);
    }

    #[test]
    fn test_serialization_round_trip() {
        let mut rec = SessionRecording::new().with_metadata("test", "roundtrip");
        rec.record(RecordedAction {
            timestamp_ms: 0,
            step: 1,
            action_type: RecordedActionType::Navigate,
            target: Some("https://example.com".to_string()),
            value: None,
            url_before: "https://example.com".to_string(),
            url_after: Some("https://example.com".to_string()),
            success: true,
            duration_ms: None,
        });

        let json = rec.to_json().unwrap();
        let deserialized = SessionRecording::from_json(&json).unwrap();

        assert_eq!(deserialized.id, rec.id);
        assert_eq!(deserialized.action_count(), 1);
        assert_eq!(deserialized.metadata.get("test"), Some(&"roundtrip".to_string()));
    }

    #[test]
    fn test_recorder_records_click() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Typed {
            selector: "#email".to_string(),
            value: "test".to_string(),
        };
        recorder.record_click("#btn", "https://example.com", &result, Duration::from_millis(100));

        assert_eq!(recorder.recording().action_count(), 1);
        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Click);
        assert_eq!(action.target.as_deref(), Some("#btn"));
        assert_eq!(action.duration_ms, Some(100));
    }

    #[test]
    fn test_recorder_records_type() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Typed {
            selector: "#field".to_string(),
            value: "hello".to_string(),
        };
        recorder.record_type("#field", "hello", "https://example.com", &result);

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Type);
        assert_eq!(action.value.as_deref(), Some("hello"));
        assert!(action.success);
    }

    #[test]
    fn test_recorder_records_type_by_id() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Typed {
            selector: "#field".to_string(),
            value: "hi".to_string(),
        };
        recorder.record_type_by_id(3, "hi", "https://example.com", &result);

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::TypeById);
        assert_eq!(action.target.as_deref(), Some("3"));
    }

    #[test]
    fn test_recorder_records_click_by_id() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Navigated(Page::from_html("<html><body>Done</body></html>", "https://example.com/done"));
        recorder.record_click_by_id(5, "https://example.com", &result, Duration::from_millis(50));

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::ClickById);
        assert_eq!(action.target.as_deref(), Some("5"));
        assert_eq!(action.url_after.as_deref(), Some("https://example.com/done"));
        assert!(action.success);
    }

    #[test]
    fn test_recorder_records_submit() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Navigated(Page::from_html("<html><body>OK</body></html>", "https://example.com/ok"));
        recorder.record_submit("form", "https://example.com", &result, Duration::from_millis(200));

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Submit);
        assert_eq!(action.duration_ms, Some(200));
    }

    #[test]
    fn test_recorder_records_wait() {
        let mut recorder = SessionRecorder::new();
        recorder.record_wait("#loaded", "https://example.com", true);

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Wait);
        assert!(action.success);
    }

    #[test]
    fn test_recorder_records_wait_not_found() {
        let mut recorder = SessionRecorder::new();
        recorder.record_wait("#never-appears", "https://example.com", false);

        let action = &recorder.recording().actions[0];
        assert!(!action.success);
    }

    #[test]
    fn test_recorder_records_scroll() {
        let mut recorder = SessionRecorder::new();
        let result = InteractionResult::Scrolled {
            url: "https://example.com?page=2".to_string(),
            page: Page::from_html("<html><body>Page 2</body></html>", "https://example.com?page=2"),
        };
        recorder.record_scroll("down", "https://example.com", &result, Duration::from_millis(300));

        let action = &recorder.recording().actions[0];
        assert_eq!(action.action_type, RecordedActionType::Scroll);
        assert_eq!(action.target.as_deref(), Some("down"));
        assert_eq!(action.duration_ms, Some(300));
    }

    #[test]
    fn test_recorder_step_numbers_sequential() {
        let mut recorder = SessionRecorder::new();
        recorder.record_navigate("https://example.com", true);
        recorder.record_navigate("https://example.com/about", true);
        recorder.record_navigate("https://example.com/contact", true);

        assert_eq!(recorder.recording().actions[0].step, 1);
        assert_eq!(recorder.recording().actions[1].step, 2);
        assert_eq!(recorder.recording().actions[2].step, 3);
    }

    #[test]
    fn test_recorder_navigate_failure() {
        let mut recorder = SessionRecorder::new();
        recorder.record_navigate("https://example.com", false);

        let action = &recorder.recording().actions[0];
        assert!(!action.success);
        assert!(action.url_after.is_none());
    }

    #[test]
    fn test_recording_last_url() {
        let mut rec = SessionRecording::new();
        rec.record(RecordedAction {
            timestamp_ms: 0, step: 1,
            action_type: RecordedActionType::Navigate,
            target: Some("https://a.com".to_string()),
            value: None,
            url_before: "https://a.com".to_string(),
            url_after: Some("https://a.com".to_string()),
            success: true, duration_ms: None,
        });
        rec.record(RecordedAction {
            timestamp_ms: 0, step: 2,
            action_type: RecordedActionType::Navigate,
            target: Some("https://b.com".to_string()),
            value: None,
            url_before: "https://a.com".to_string(),
            url_after: Some("https://b.com".to_string()),
            success: true, duration_ms: None,
        });

        assert_eq!(rec.last_url(), Some("https://b.com"));
    }

    #[test]
    fn test_recording_last_url_none() {
        let rec = SessionRecording::new();
        assert!(rec.last_url().is_none());
    }

    #[test]
    fn test_recording_is_empty() {
        let rec = SessionRecording::new();
        assert!(rec.is_empty());
    }

    #[test]
    fn test_recording_not_empty() {
        let mut rec = SessionRecording::new();
        rec.record(RecordedAction {
            timestamp_ms: 0, step: 1,
            action_type: RecordedActionType::Navigate,
            target: Some("https://example.com".to_string()),
            value: None,
            url_before: "https://example.com".to_string(),
            url_after: Some("https://example.com".to_string()),
            success: true, duration_ms: None,
        });
        assert!(!rec.is_empty());
    }

    #[test]
    fn test_recording_action_count() {
        let mut rec = SessionRecording::new();
        assert_eq!(rec.action_count(), 0);
        rec.record(RecordedAction {
            timestamp_ms: 0, step: 1,
            action_type: RecordedActionType::Navigate,
            target: Some("https://example.com".to_string()),
            value: None,
            url_before: "https://example.com".to_string(),
            url_after: Some("https://example.com".to_string()),
            success: true, duration_ms: None,
        });
        rec.record(RecordedAction {
            timestamp_ms: 0, step: 2,
            action_type: RecordedActionType::Click,
            target: Some("#btn".to_string()),
            value: None,
            url_before: "https://example.com".to_string(),
            url_after: None,
            success: true, duration_ms: None,
        });
        assert_eq!(rec.action_count(), 2);
    }

    #[test]
    fn test_recorder_with_metadata() {
        let recorder = SessionRecorder::new()
            .with_metadata("session_id", "abc-123");

        assert_eq!(recorder.recording().metadata.get("session_id"), Some(&"abc-123".to_string()));
    }

    #[test]
    fn test_recorder_into_recording() {
        let mut recorder = SessionRecorder::new();
        recorder.record_navigate("https://example.com", true);
        let rec = recorder.into_recording();

        assert_eq!(rec.action_count(), 1);
    }

    #[test]
    fn test_extract_result_url_navigated() {
        let page = Page::from_html("<html><body>Test</body></html>", "https://example.com/result");
        let result = InteractionResult::Navigated(page);
        let (url, success) = extract_result_url(&result);
        assert_eq!(url, Some("https://example.com/result".to_string()));
        assert!(success);
    }

    #[test]
    fn test_extract_result_url_typed() {
        let result = InteractionResult::Typed {
            selector: "#f".to_string(),
            value: "val".to_string(),
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(success);
    }

    #[test]
    fn test_extract_result_url_toggled() {
        let result = InteractionResult::Toggled {
            selector: "#c".to_string(),
            checked: true,
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(success);
    }

    #[test]
    fn test_extract_result_url_selected() {
        let result = InteractionResult::Selected {
            selector: "#s".to_string(),
            value: "opt1".to_string(),
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(success);
    }

    #[test]
    fn test_extract_result_url_element_not_found() {
        let result = InteractionResult::ElementNotFound {
            selector: "#missing".to_string(),
            reason: "not found".to_string(),
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(!success);
    }

    #[test]
    fn test_extract_result_url_wait_satisfied_found() {
        let result = InteractionResult::WaitSatisfied {
            selector: "#loader".to_string(),
            found: true,
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(success);
    }

    #[test]
    fn test_extract_result_url_wait_satisfied_not_found() {
        let result = InteractionResult::WaitSatisfied {
            selector: "#loader".to_string(),
            found: false,
        };
        let (url, success) = extract_result_url(&result);
        assert!(url.is_none());
        assert!(!success);
    }

    #[test]
    fn test_extract_result_url_scrolled() {
        let result = InteractionResult::Scrolled {
            url: "https://example.com?page=2".to_string(),
            page: Page::from_html("<html><body>P2</body></html>", "https://example.com?page=2"),
        };
        let (url, success) = extract_result_url(&result);
        assert_eq!(url, Some("https://example.com?page=2".to_string()));
        assert!(success);
    }

    #[test]
    fn test_recording_default() {
        let rec = SessionRecording::default();
        assert!(rec.is_empty());
    }

    #[test]
    fn test_recorder_default() {
        let recorder = SessionRecorder::default();
        assert!(recorder.recording().is_empty());
    }

    #[test]
    fn test_serialization_all_action_types() {
        let mut rec = SessionRecording::new();
        let types = vec![
            RecordedActionType::Navigate,
            RecordedActionType::Click,
            RecordedActionType::ClickById,
            RecordedActionType::Type,
            RecordedActionType::TypeById,
            RecordedActionType::Submit,
            RecordedActionType::Wait,
            RecordedActionType::Scroll,
            RecordedActionType::Toggle,
            RecordedActionType::SelectOption,
        ];
        for at in &types {
            rec.record(RecordedAction {
                timestamp_ms: 0,
                step: rec.action_count() + 1,
                action_type: at.clone(),
                target: Some("test".to_string()),
                value: None,
                url_before: "https://example.com".to_string(),
                url_after: None,
                success: true,
                duration_ms: None,
            });
        }

        let json = rec.to_json().unwrap();
        let deserialized = SessionRecording::from_json(&json).unwrap();
        assert_eq!(deserialized.action_count(), types.len());
    }

    #[test]
    fn test_recorder_elapsed_increases() {
        let mut recorder = SessionRecorder::new();
        recorder.record_navigate("https://example.com", true);
        let ts1 = recorder.recording().actions[0].timestamp_ms;

        std::thread::sleep(std::time::Duration::from_millis(5));
        recorder.record_navigate("https://example.com/about", true);
        let ts2 = recorder.recording().actions[1].timestamp_ms;

        assert!(ts2 >= ts1);
    }

    #[test]
    fn test_recording_from_json_invalid() {
        let result = SessionRecording::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_recording_to_json_empty() {
        let rec = SessionRecording::new();
        let json = rec.to_json().unwrap();
        let deserialized = SessionRecording::from_json(&json).unwrap();
        assert!(deserialized.is_empty());
    }
}
