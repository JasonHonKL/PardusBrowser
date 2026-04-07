use std::collections::HashSet;
use serde::Serialize;
use serde_json::Value;

/// Represents a CDP target (maps 1:1 to a open-core Tab).
#[derive(Debug, Clone, Serialize)]
pub struct CdpTarget {
    pub target_id: String,
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub opener_id: Option<String>,
    pub browser_context_id: String,
    pub can_access_opener: bool,
}

impl CdpTarget {
    pub fn new(target_id: String, url: String) -> Self {
        Self {
            target_id,
            target_type: "page".to_string(),
            title: String::new(),
            url,
            opener_id: None,
            browser_context_id: "default".to_string(),
            can_access_opener: false,
        }
    }
}

/// Per-connection session state.
pub struct CdpSession {
    pub session_id: String,
    pub target_id: Option<String>,
    pub enabled_domains: HashSet<String>,
    pub next_execution_context_id: u64,
    pub execution_contexts: Vec<ExecutionContextDescription>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecutionContextDescription {
    pub id: u64,
    pub origin: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aux_data: Option<Value>,
}

impl CdpSession {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            target_id: None,
            enabled_domains: HashSet::new(),
            next_execution_context_id: 1,
            execution_contexts: Vec::new(),
        }
    }

    pub fn is_domain_enabled(&self, domain: &str) -> bool {
        self.enabled_domains.contains(domain)
    }

    pub fn enable_domain(&mut self, domain: &str) {
        self.enabled_domains.insert(domain.to_string());
    }

    pub fn disable_domain(&mut self, domain: &str) {
        self.enabled_domains.remove(domain);
    }

    pub fn create_execution_context(&mut self, origin: String, name: String) -> u64 {
        let id = self.next_execution_context_id;
        self.next_execution_context_id += 1;
        self.execution_contexts.push(ExecutionContextDescription {
            id,
            origin,
            name,
            aux_data: Some(serde_json::json!({ "isDefault": true })),
        });
        id
    }
}

/// Browser context isolation boundary.
#[derive(Debug)]
pub struct BrowserContext {
    pub id: String,
    pub target_ids: Vec<String>,
}

impl BrowserContext {
    pub fn new(id: String) -> Self {
        Self {
            id,
            target_ids: Vec::new(),
        }
    }
}
