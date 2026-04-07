use std::collections::HashMap;
use crate::domain::CdpDomainHandler;

/// Registry mapping domain names to their handlers.
pub struct DomainRegistry {
    handlers: HashMap<&'static str, Box<dyn CdpDomainHandler>>,
}

impl DomainRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: Box<dyn CdpDomainHandler>) {
        self.handlers.insert(handler.domain_name(), handler);
    }

    pub fn get(&self, domain: &str) -> Option<&dyn CdpDomainHandler> {
        self.handlers.get(domain).map(|h| h.as_ref())
    }
}
