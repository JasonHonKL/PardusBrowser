//! Tab state tracking

/// Lifecycle state of a tab
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum TabState {
    /// Tab created but page not yet loaded
    Loading,
    /// Page loaded successfully and ready for interaction
    Ready,
    /// Currently navigating to a new URL
    Navigating,
    /// Error occurred during load/navigation
    Error(String),
}

impl Default for TabState {
    fn default() -> Self {
        Self::Loading
    }
}

impl TabState {
    /// Returns true if the tab is in a ready state
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns true if the tab is currently loading or navigating
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Loading | Self::Navigating)
    }

    /// Returns true if the tab is in an error state
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Get error message if in error state
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(msg) => Some(msg),
            _ => None,
        }
    }
}
