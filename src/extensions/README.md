# Adding a new provider as an extension

Implement the `DomainRegistrar` trait from `reggae::core::traits` for your new provider.

## Example skeleton

```rust
// extensions/myprovider/mod.rs
use async_trait::async_trait;
use reqwest::Client;
use reggae::core::{error::Error, traits::DomainRegistrar, types::*};

pub struct MyProviderClient {
    api_key: String,
    http: Client,
}

impl MyProviderClient {
    pub fn new(api_key: String, http: Client) -> Self {
        Self { api_key, http }
    }
}

#[async_trait]
impl DomainRegistrar for MyProviderClient {
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        // Call your API here
        todo!()
    }
    // Implement remaining methods…
}
```

## Registration

In `main.rs`, register your provider with the `ProviderRegistry`:

```rust
registry.register("myprovider", Box::new(MyProviderClient::new(api_key, http.clone())));
```

Then users can use `--provider myprovider` to select it.

## Configuration

Add credentials to `config.yaml` under `providers.myprovider` and expose matching
`DM_PROVIDERS_MYPROVIDER_*` environment variables in `Settings::apply_env_overrides`.
