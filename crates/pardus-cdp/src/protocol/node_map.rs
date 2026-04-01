use std::collections::HashMap;

/// Bidirectional mapping between CDP backendNodeId and CSS selectors.
/// Each session maintains its own map.
pub struct NodeMap {
    next_id: i64,
    id_to_selector: HashMap<i64, String>,
    selector_to_id: HashMap<String, i64>,
}

impl NodeMap {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            id_to_selector: HashMap::new(),
            selector_to_id: HashMap::new(),
        }
    }

    /// Get or assign a backendNodeId for a CSS selector.
    pub fn get_or_assign(&mut self, selector: &str) -> i64 {
        if let Some(&id) = self.selector_to_id.get(selector) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.id_to_selector.insert(id, selector.to_string());
        self.selector_to_id.insert(selector.to_string(), id);
        id
    }

    /// Look up the CSS selector for a backendNodeId.
    pub fn get_selector(&self, node_id: i64) -> Option<&str> {
        self.id_to_selector.get(&node_id).map(|s| s.as_str())
    }

    /// Look up the backendNodeId for a CSS selector.
    pub fn get_id(&self, selector: &str) -> Option<i64> {
        self.selector_to_id.get(selector).copied()
    }

    /// Remove a node by ID.
    pub fn remove(&mut self, node_id: i64) {
        if let Some(selector) = self.id_to_selector.remove(&node_id) {
            self.selector_to_id.remove(&selector);
        }
    }

    /// Clear all mappings (called on navigation).
    pub fn clear(&mut self) {
        self.next_id = 1;
        self.id_to_selector.clear();
        self.selector_to_id.clear();
    }
}
