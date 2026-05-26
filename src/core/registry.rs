use std::collections::HashMap;
use crate::core::{error::Error, traits::DomainRegistrar};

pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn DomainRegistrar>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, provider: Box<dyn DomainRegistrar>) {
        self.providers.insert(name.to_lowercase(), provider);
    }

    pub fn get(&self, name: &str) -> Result<&dyn DomainRegistrar, Error> {
        self.providers
            .get(&name.to_lowercase())
            .map(|p| p.as_ref())
            .ok_or_else(|| Error::ProviderNotFound(name.to_string()))
    }

    pub fn available_providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
