use std::collections::HashMap;

pub struct NodeMap {
    next_id: i64,
    id_to_selector: HashMap<i64, String>,
    selector_to_id: HashMap<String, i64>,
    document_version: u64,
}

impl NodeMap {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            id_to_selector: HashMap::new(),
            selector_to_id: HashMap::new(),
            document_version: 0,
        }
    }

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

    pub fn get_or_assign_indexed(&mut self, selector: &str, index: usize) -> i64 {
        let key = format!("{}[{}]", selector, index);
        self.get_or_assign(&key)
    }

    pub fn get_selector(&self, node_id: i64) -> Option<&str> {
        self.id_to_selector.get(&node_id).map(|s| s.as_str())
    }

    pub fn get_id(&self, selector: &str) -> Option<i64> {
        self.selector_to_id.get(selector).copied()
    }

    pub fn remove(&mut self, node_id: i64) {
        if let Some(selector) = self.id_to_selector.remove(&node_id) {
            self.selector_to_id.remove(&selector);
        }
    }

    pub fn invalidate_on_navigation(&mut self) {
        self.document_version += 1;
        self.id_to_selector.clear();
        self.selector_to_id.clear();
        self.next_id = 1;
    }

    pub fn document_version(&self) -> u64 {
        self.document_version
    }

    pub fn len(&self) -> usize {
        self.id_to_selector.len()
    }

    pub fn is_empty(&self) -> bool {
        self.id_to_selector.is_empty()
    }
}
