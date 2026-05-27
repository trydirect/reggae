use serde::{Deserialize, Serialize};
use std::path::Path;
use crate::core::{error::Error, types::RegistrantContact};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default)]
    pub providers: ProvidersConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Default registrant contact used for domain registration (GoDaddy, Namecheap, Porkbun).
    #[serde(default)]
    pub registrant: RegistrantContact,
}

fn default_provider() -> String {
    "porkbun".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub porkbun: PorkbunConfig,
    #[serde(default)]
    pub cloudflare: CloudflareConfig,
    #[serde(default)]
    pub godaddy: GodaddyConfig,
    #[serde(default)]
    pub namecheap: NamecheapConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PorkbunConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub secret_api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CloudflareConfig {
    #[serde(default)]
    pub api_token: String,
    /// Account ID — required for Registrar API operations
    #[serde(default)]
    pub account_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GodaddyConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_secret: String,
    /// Use OTE (sandbox) endpoint instead of production
    #[serde(default)]
    pub sandbox: bool,
    /// Client IP included in GoDaddy purchase consent payload
    #[serde(default = "default_consent_ip")]
    pub consent_ip: String,
    #[serde(default)]
    pub contact: RegistrantContact,
}

fn default_consent_ip() -> String {
    "0.0.0.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NamecheapConfig {
    #[serde(default)]
    pub api_user: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub username: String,
    /// Your whitelisted public IP for Namecheap API access
    #[serde(default)]
    pub client_ip: String,
    /// Use sandbox endpoint (api.sandbox.namecheap.com)
    #[serde(default)]
    pub sandbox: bool,
    #[serde(default)]
    pub contact: RegistrantContact,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_check_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_expiry_lead_days")]
    pub expiry_lead_days: u32,
    #[serde(default)]
    pub notification: NotificationConfig,
    #[serde(default)]
    pub domains: Vec<String>,
}

fn default_check_interval() -> u64 { 86400 }
fn default_expiry_lead_days() -> u32 { 30 }

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            check_interval_secs: default_check_interval(),
            expiry_lead_days: default_expiry_lead_days(),
            notification: NotificationConfig::default(),
            domains: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NotificationConfig {
    #[serde(rename = "type", default = "default_notification_type")]
    pub notification_type: NotificationType,
    #[serde(default)]
    pub webhook_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    #[default]
    Stdout,
    Webhook,
}

fn default_notification_type() -> NotificationType {
    NotificationType::Stdout
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String { "info".to_string() }

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: default_log_level() }
    }
}

impl Settings {
    pub fn load(config_path: Option<&Path>) -> Result<Self, Error> {
        let mut settings = Self::from_file(config_path)?;
        settings.apply_env_overrides();
        Ok(settings)
    }

    fn from_file(path: Option<&Path>) -> Result<Self, Error> {
        let search_paths: Vec<std::path::PathBuf> = if let Some(p) = path {
            vec![p.to_path_buf()]
        } else {
            let mut paths = vec![std::path::PathBuf::from("config.yaml")];
            if let Some(home) = dirs::home_dir() {
                paths.push(home.join(".reggae").join("config.yaml"));
            }
            paths.push(std::path::PathBuf::from("/etc/reggae/config.yaml"));
            paths
        };

        for p in &search_paths {
            if p.exists() {
                let content = std::fs::read_to_string(p)
                    .map_err(|e| Error::Config(format!("Cannot read {}: {}", p.display(), e)))?;
                let s: Settings = serde_yaml::from_str(&content)?;
                tracing::debug!("Loaded config from {}", p.display());
                return Ok(s);
            }
        }

        tracing::debug!("No config file found, using defaults");
        Ok(Settings::default())
    }

    fn apply_env_overrides(&mut self) {
        use std::env;

        if let Ok(v) = env::var("DM_DEFAULT_PROVIDER") { self.default_provider = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_PORKBUN_API_KEY") { self.providers.porkbun.api_key = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_PORKBUN_SECRET_API_KEY") { self.providers.porkbun.secret_api_key = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_CLOUDFLARE_API_TOKEN") { self.providers.cloudflare.api_token = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_GODADDY_API_KEY") { self.providers.godaddy.api_key = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_GODADDY_API_SECRET") { self.providers.godaddy.api_secret = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_GODADDY_SANDBOX") {
            self.providers.godaddy.sandbox = v.to_lowercase() == "true" || v == "1";
        }
        if let Ok(v) = env::var("DM_PROVIDERS_NAMECHEAP_API_USER") { self.providers.namecheap.api_user = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_NAMECHEAP_API_KEY") { self.providers.namecheap.api_key = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_NAMECHEAP_USERNAME") { self.providers.namecheap.username = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_NAMECHEAP_CLIENT_IP") { self.providers.namecheap.client_ip = v; }
        if let Ok(v) = env::var("DM_PROVIDERS_NAMECHEAP_SANDBOX") {
            self.providers.namecheap.sandbox = v.to_lowercase() == "true" || v == "1";
        }
        if let Ok(v) = env::var("DM_SCHEDULER_ENABLED") {
            self.scheduler.enabled = v.to_lowercase() == "true" || v == "1";
        }
        if let Ok(v) = env::var("DM_SCHEDULER_CHECK_INTERVAL_SECS") {
            if let Ok(n) = v.parse() { self.scheduler.check_interval_secs = n; }
        }
        if let Ok(v) = env::var("DM_SCHEDULER_EXPIRY_LEAD_DAYS") {
            if let Ok(n) = v.parse() { self.scheduler.expiry_lead_days = n; }
        }
        if let Ok(v) = env::var("DM_LOGGING_LEVEL") { self.logging.level = v; }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_settings() {
        let s = Settings::default();
        assert_eq!(s.default_provider, "");
        assert_eq!(s.logging.level, "info");
        assert!(!s.scheduler.enabled);
    }

    #[test]
    fn test_load_from_yaml() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "default_provider: cloudflare").unwrap();
        writeln!(f, "logging:").unwrap();
        writeln!(f, "  level: debug").unwrap();
        let s = Settings::load(Some(f.path())).unwrap();
        assert_eq!(s.default_provider, "cloudflare");
        assert_eq!(s.logging.level, "debug");
    }

    #[test]
    fn test_env_overrides() {
        std::env::set_var("DM_DEFAULT_PROVIDER", "godaddy");
        std::env::set_var("DM_LOGGING_LEVEL", "warn");
        let s = Settings::load(None).unwrap();
        assert_eq!(s.default_provider, "godaddy");
        assert_eq!(s.logging.level, "warn");
        std::env::remove_var("DM_DEFAULT_PROVIDER");
        std::env::remove_var("DM_LOGGING_LEVEL");
    }
}
