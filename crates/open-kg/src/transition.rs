use serde::{Deserialize, Serialize};

use crate::state::ViewStateId;

/// A verified transition between two view-states.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// Source view-state.
    pub from: ViewStateId,
    /// Target view-state.
    pub to: ViewStateId,
    /// What triggers this transition.
    pub trigger: Trigger,
    /// Whether we verified this by actually following it.
    pub verified: bool,
    /// If verified, did the actual outcome match prediction?
    pub outcome: Option<TransitionOutcome>,
}

/// What causes a transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Trigger {
    /// Clicking an internal link.
    LinkClick {
        url: String,
        label: Option<String>,
        selector: Option<String>,
    },
    /// Hash/anchor navigation within the same page.
    HashNavigation {
        fragment: String,
        label: Option<String>,
    },
    /// Scroll/pagination to a new page of results.
    Pagination {
        from_url: String,
        to_url: String,
    },
    /// Form submission (predicted, not always verified).
    FormSubmit {
        form_id: Option<String>,
        action: Option<String>,
        method: String,
        field_count: usize,
    },
}

/// Outcome of verifying a transition by following it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionOutcome {
    /// HTTP status of the resulting page.
    pub status: u16,
    /// Final URL (may differ due to redirects).
    pub final_url: String,
    /// Whether the resulting ViewStateId matched the prediction.
    pub matched_prediction: bool,
}
