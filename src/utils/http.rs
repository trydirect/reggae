use reqwest::{Client, ClientBuilder};
use std::time::Duration;

/// Build a shared HTTP client with sensible defaults.
pub fn build_http_client() -> Client {
    ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .user_agent(concat!("reggae/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("Failed to build HTTP client")
}
