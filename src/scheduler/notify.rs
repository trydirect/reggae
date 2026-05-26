use async_trait::async_trait;
use serde_json::json;
use colored::Colorize;
use crate::core::error::Error;

#[derive(Debug, Clone)]
pub struct ExpiryNotification {
    pub domain: String,
    pub expiry_date: String,
    pub days_left: i64,
}

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, notification: &ExpiryNotification) -> Result<(), Error>;
}

pub struct StdoutNotifier;

#[async_trait]
impl Notifier for StdoutNotifier {
    async fn notify(&self, n: &ExpiryNotification) -> Result<(), Error> {
        let msg = format!(
            "⚠  Domain '{}' expires on {} ({} days left)",
            n.domain, n.expiry_date, n.days_left
        );
        println!("{}", msg.yellow().bold());
        Ok(())
    }
}

pub struct WebhookNotifier {
    pub url: String,
    http: reqwest::Client,
}

impl WebhookNotifier {
    pub fn new(url: String) -> Self {
        Self { url, http: reqwest::Client::new() }
    }
}

#[async_trait]
impl Notifier for WebhookNotifier {
    async fn notify(&self, n: &ExpiryNotification) -> Result<(), Error> {
        let payload = json!({
            "domain": n.domain,
            "expiry_date": n.expiry_date,
            "days_left": n.days_left,
        });
        self.http.post(&self.url).json(&payload).send().await
            .map_err(Error::Network)?;
        Ok(())
    }
}
