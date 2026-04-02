//! Tab management module for pardus-core
//!
//! Provides orthogonal tab functionality that wraps existing Page/App
//! without modifying them. Tabs share App resources but maintain
//! independent page state.

use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub struct TabId(u64);

impl TabId {
    /// Create a new unique tab ID
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }

    /// Create a TabId from a u64 value
    /// 
    /// Note: This does not guarantee uniqueness. Use `new()` for that.
    pub const fn from_u64(id: u64) -> Self {
        Self(id)
    }

    /// Get the underlying u64 value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for TabId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub mod state;
pub mod tab;
pub mod manager;

pub use state::TabState;
pub use tab::{Tab, TabConfig};
pub use manager::TabManager;
