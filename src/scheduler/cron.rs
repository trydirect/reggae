use chrono::Utc;
use tracing::{info, warn};

use crate::config::Settings;
use crate::core::{registry::ProviderRegistry, traits::DomainRegistrar};
use crate::scheduler::notify::{ExpiryNotification, Notifier, StdoutNotifier, WebhookNotifier};
use crate::config::settings::NotificationType;

/// Run a single expiry check pass across all configured domains.
pub async fn run_expiry_check(
    registry: &ProviderRegistry,
    settings: &Settings,
    notifier: &dyn Notifier,
) {
    let provider_name = &settings.default_provider;
    let domains = &settings.scheduler.domains;
    let lead_days = settings.scheduler.expiry_lead_days as i64;

    if domains.is_empty() {
        info!("No domains configured for expiry check");
        return;
    }

    for domain in domains {
        match registry.get(provider_name) {
            Ok(provider) => {
                match provider.get_domain_info(domain).await {
                    Ok(info) => {
                        if let Some(expiry) = info.expiry_date {
                            let days_left = (expiry - Utc::now()).num_days();
                            info!("Domain '{}' expires in {} days", domain, days_left);
                            if days_left <= lead_days {
                                let notification = ExpiryNotification {
                                    domain: domain.clone(),
                                    expiry_date: expiry.format("%Y-%m-%d").to_string(),
                                    days_left,
                                };
                                if let Err(e) = notifier.notify(&notification).await {
                                    warn!("Failed to send notification for {}: {}", domain, e);
                                }
                            }
                        } else {
                            warn!("No expiry date for domain '{}'", domain);
                        }
                    }
                    Err(e) => warn!("Failed to get info for '{}': {}", domain, e),
                }
            }
            Err(e) => warn!("Provider '{}' not available: {}", provider_name, e),
        }
    }
}

/// Build the appropriate notifier from settings.
pub fn build_notifier(settings: &Settings) -> Box<dyn Notifier> {
    match settings.scheduler.notification.notification_type {
        NotificationType::Webhook if !settings.scheduler.notification.webhook_url.is_empty() => {
            Box::new(WebhookNotifier::new(settings.scheduler.notification.webhook_url.clone()))
        }
        _ => Box::new(StdoutNotifier),
    }
}
