use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Provider '{0}' not found or not configured")]
    ProviderNotFound(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Parsing error: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("Unsupported operation")]
    Unsupported,

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Scheduler error: {0}")]
    Scheduler(String),
}
