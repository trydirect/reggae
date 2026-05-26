Create a RUST "CLI" app that will help users to manage domains (DNS records, NS records,  and other resources) using command line interface. Create a Rust wrapper for Domain Registrar services like Porkbun, Cloudflare, Godaddy, Namecheap, and others who have API documentation available. Make a review of available API's domain registrar services and based on that create a unified wrapper. Specific functions or features must be added into "extensions" directory.
So first task is to make a research of API's for 3-4 providers.
For example you can check the Porkbun first as most advanced I think: https://porkbun.com/api/json/v3/documentation#tag/pricing/GET/pricing/get. Then you can design structs, enums and impls. App should be configurable and read settings from ENV. ENV vars is highest priority, second priority can be a config.yaml file.
Keep DRY, KISS principles. Start with BDD tests. Follow TDD.

Example command:  "API_KEY=..  register --provider "porkbun" --domain "example.com" --A 198.148.5.21 ...."
Add cron scheduler abilities to the app so it can send a notification in advance before domain is expired.
Suggest features so users find it usefull in cli app.
I am going to use this app  as addition to Stacker for domain registration




# reggae - Domain Manager CLI – Implementation Plan

## 1. Overview
A cross‑registrar CLI tool written in Rust that unifies domain and DNS management across multiple providers (Porkbun, Cloudflare, GoDaddy, Namecheap, …). The application can be used standalone or as a sub‑command of the “Stacker” deployment workflow. It follows KISS/DRY, is driven by BDD/TDD, and ships a built‑in cron scheduler for expiry notifications.

## 2. Research Phase – `research.md`
Before coding, inspect the public REST APIs of at least four providers. For each provider document:

- Authentication scheme (API key, token, OAuth2)
- Base URL and content type (JSON/XML)
- Endpoints:
  - Domain availability check
  - Domain registration / renewal / transfer
  - DNS record CRUD (A, AAAA, CNAME, MX, TXT, NS, etc.)
  - Nameserver management
  - Pricing retrieval
  - Domain info (expiration date, status)
- Rate limits, pagination
- Notable differences (e.g., Cloudflare uses zones, Porkbun is flat)

Start with **Porkbun** (API: `https://porkbun.com/api/json/v3/documentation`) because its REST/JSON interface is straightforward. Summarise findings in a table to drive the trait design.

## 3. Project Structure

reggae/
├── .github/
│ └── workflows/
│ └── release.yml # CI/CD: build + publish binaries
├── installer.sh # curl installer script
├── Cargo.toml
├── config.yaml.example
├── features/ # BDD feature files
│ ├── domain_registration.feature
│ ├── dns_management.feature
│ └── expiry_check.feature
├── src/
│ ├── main.rs # CLI entry point
│ ├── lib.rs # public API for Stacker integration
│ ├── cli/
│ │ ├── mod.rs
│ │ ├── args.rs # clap command structures
│ │ └── commands/ # handler functions per subcommand
│ │ ├── mod.rs
│ │ ├── register.rs
│ │ ├── dns.rs
│ │ ├── nameserver.rs
│ │ ├── check.rs
│ │ ├── schedule.rs
│ │ └── output.rs # JSON / table formatting
│ ├── config/
│ │ ├── mod.rs
│ │ └── settings.rs # merged ENV + YAML config
│ ├── core/
│ │ ├── mod.rs
│ │ ├── traits.rs # DomainRegistrar trait
│ │ ├── types.rs # Domain, DnsRecord, etc.
│ │ ├── error.rs # unified error type
│ │ └── registry.rs # provider factory
│ ├── providers/
│ │ ├── mod.rs
│ │ ├── porkbun/
│ │ │ ├── client.rs
│ │ │ └── api.rs # endpoints mapping
│ │ ├── cloudflare/
│ │ ├── godaddy/
│ │ └── namecheap/
│ ├── extensions/ # extension developer docs + examples
│ │ └── README.md
│ ├── scheduler/
│ │ ├── mod.rs
│ │ ├── cron.rs # tokio-cron-scheduler setup
│ │ └── notify.rs # stdout, webhook
│ └── utils/
│ ├── mod.rs
│ └── http.rs # reqwest client factory with retries
├── tests/
│ ├── integration/
│ │ ├── cli_tests.rs
│ │ └── provider_tests.rs
│ └── cucumber/ # step definitions for BDD
└── plan.md

Cargo.toml
## 4. Key Dependencies (`Cargo.toml`)
```toml
[package]
name = "reggae"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive", "env"] }
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "retry"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
dotenvy = "0.15"
cucumber = { version = "0.19", features = ["output-json"] }
tokio-cron-scheduler = "0.10"
chrono = { version = "0.4", features = ["serde"] }
url = "2"

[dev-dependencies]
mockito = "1"
assert_cmd = "2"
predicates = "3"
tempfile = "3"


5. Configuration Design

Configuration is loaded in this order (later wins):

config.yaml (or path given by --config flag)
Environment variables prefixed with DM_




config.yaml structure

yaml
default_provider: porkbun
providers:
  porkbun:
    api_key: ""
    secret_api_key: ""
  cloudflare:
    api_token: ""
  godaddy:
    api_key: ""
    api_secret: ""
  namecheap:
    api_user: ""
    api_key: ""
    username: ""
scheduler:
  enabled: false
  check_interval_secs: 86400   # daily
  expiry_lead_days: 30         # notify when < N days left
  notification:
    type: stdout               # stdout | webhook
    webhook_url: ""
logging:
  level: info                  # trace, debug, info, warn, error
Environment variables override individual fields:

DM_DEFAULT_PROVIDER=porkbun
DM_PROVIDERS_PORKBUN_API_KEY=...
DM_SCHEDULER_ENABLED=true
etc.
A Settings struct holds the merged result using serde deserialization with #[serde(default)] and a custom resolver that applies ENV overrides.

6. Core Traits and Types

DomainRegistrar trait (in core/traits.rs)

rust
#[async_trait]
pub trait DomainRegistrar: Send + Sync {
    /// Check if a domain is available for registration.
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error>;

    /// Register a domain with optional initial DNS records.
    async fn register_domain(&self, domain: &str, records: Option<Vec<DnsRecord>>) -> Result<Domain, Error>;

    /// Renew a domain for a given number of years.
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error>;

    /// Transfer a domain (auth code required).
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error>;

    /// Retrieve domain info, including expiry date.
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error>;

    /// List all DNS records for a domain.
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error>;

    /// Create a DNS record.
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error>;

    /// Update an existing DNS record.
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error>;

    /// Delete a DNS record by ID.
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error>;

    /// Manage nameservers (set custom NS).
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error>;

    /// Get pricing for domain registration/renewal/transfer.
    async fn get_pricing(&self, domain: &str) -> Result<Pricing, Error>;
}
Common types (core/types.rs)

Domain { name, expiry_date, status, nameservers }
DnsRecord { id, r#type, name, content, ttl, priority }
Availability { available, premium, price }
Pricing { registration_price, renewal_price, transfer_price, currency }
Error type (core/error.rs)

rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Parsing error: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("Unsupported operation")]
    Unsupported,
}
7. CLI Design (using clap)

Top-level commands:

register – Register a new domain
dns – Manage DNS records (subcommands: list, add, update, delete)
ns – Manage nameservers (subcommands: get, set)
check – Check availability
info – Domain info
renew – Renew domain
transfer – Transfer domain
pricing – Get pricing
schedule – Start background scheduler daemon
run-check – One-shot expiry check (for external cron)
completion – Shell completions
Each command accepts --provider <NAME> (default from config), and global flags --config <PATH>, --output json|table (default: table for humans, JSON for scripts). Credentials come exclusively from config/ENV, never CLI arguments.

Example:

bash
API_KEY=... ./domain-manager register --provider porkbun --domain example.com --A 198.148.5.21
./domain-manager dns list --domain example.com --output json
./domain-manager schedule                    # daemon mode
./domain-manager run-check                   # one-shot check, use with systemd timer
A future integration as stacker domain ... can be achieved by detecting the binary name and parsing appropriately, but initially we focus on the standalone CLI.

8. Provider Implementations

8.1 Porkbun (reference implementation)

Client holds api_key, secret_api_key, base_url.
Each method maps to the endpoints documented.
Parses JSON responses into the unified types.
Meticulous error mapping from Porkbun’s status field.
8.2 Cloudflare, GoDaddy, Namecheap

Follow the same pattern, adapting authentication headers and request/response shapes.

All providers are registered into a ProviderRegistry (core/registry.rs) using the provider name string. The registry holds a HashMap<String, Box<dyn DomainRegistrar>> that is lazily initialised with credentials from the active configuration. The CLI command handler fetches the correct provider from the registry.

9. Extensions System

The extensions/ directory contains a README.md explaining how third‑party providers can be added:

Implement the DomainRegistrar trait for the new provider.
Place the module inside extensions/.
Register it in the registry using a public function that the binary calls at startup (e.g., registry.register("newprov", Box::new(NewProviderClient::new(...)))).
If the extension requires extra dependencies, it can be compiled as a dynamic library (though for simplicity, static compilation inside the same workspace is preferred – new providers are added as pull requests to the main repo).
The extensions/ directory itself is not compiled automatically; developers can copy a template provider file and follow the guide.

10. Scheduler & Notifications

10.1 Configuration

scheduler section in config.yaml (see above). enabled toggles scheduler, check_interval_secs sets the frequency, expiry_lead_days determines when a notification is triggered.

10.2 Daemon Mode (schedule subcommand)

Spawns a tokio-cron-scheduler task that runs the check at the given interval.
On each run, iterates over all domains listed in the config (or discovered via a --domains flag) and uses the configured provider to get expiry dates.
Compares to expiry_lead_days. If today + lead_days >= expiry, fires a notification.
10.3 One‑shot Mode (run-check subcommand)

Executes the same check once and exits; perfect for external cron or systemd.timer.
10.4 Notifications

stdout: prints a coloured warning with domain name and days left.
webhook: sends a JSON payload ({"domain": "...", "expiry_date": "...", "days_left": N}) to the configured URL via HTTP POST.
Implementation uses a trait Notifier with StdoutNotifier and WebhookNotifier. The scheduler constructs the appropriate notifier from config.

11. Output Formatting

The --output flag controls the presentation. A lightweight OutputFormatter trait is used. We provide JsonOutput (just prints the serialized response) and TableOutput (using a simple table crate like tabled). The CLI command handler picks the formatter and prints the result.

For Stacker integration, the JSON output mode is essential. In the future, a stacker domain subcommand can directly use the library functions and return structured data, bypassing the CLI output layer entirely.

12. Testing Strategy (TDD/BDD)

12.1 BDD with Cucumber

Feature files in features/ describe scenarios in Gherkin. For example:

gherkin
Feature: Domain registration
  Scenario: Register a free domain
    Given the Porkbun provider is configured with valid credentials
    And the domain "example123.com" is available
    When I run `domain-manager register --provider porkbun --domain example123.com`
    Then the output should indicate success
    And the domain expiry date should be one year from now
Step definitions in tests/cucumber/ use mockito to mock HTTP responses. The world holds a test configuration and mock server.

12.2 Unit Tests

Provider client methods are tested against mocked HTTP endpoints using mockito. Each method is tested for success and common error conditions.
Configuration merging logic is tested with temporary YAML files and environment variable overrides.
12.3 Integration Tests

assert_cmd tests run the binary with specific arguments and check stdout/stderr/exit code.
A temporary config.yaml is used along with environment variables; a mock HTTP server stands in for the provider APIs.
13. CI/CD and Distribution

13.1 GitHub Actions Workflow (.github/workflows/release.yml)

Trigger: push of a version tag (v*).

Jobs:

build matrix: os: [ubuntu-latest, macos-latest], target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-apple-darwin, aarch64-apple-darwin] (careful with macos arm). Using cross-compilation or dedicated runners.
Steps: checkout, install Rust, build with --release, archive binary.
release: create GitHub Release and upload all artifacts.
13.2 Installer Script (installer.sh)

A curl-pipeable script that:

Detects OS and architecture.
Fetches the latest release tag from GitHub API.
Downloads the appropriate binary.
Installs it to /usr/local/bin (or ~/.local/bin).
Optionally sets up shell completions.
Example:

bash
curl -sSL https://raw.githubusercontent.com/.../installer.sh | bash
14. Implementation Roadmap

Phase 1: Foundation (week 1)

Project scaffold, dependencies, config loading (settings.rs).
Core traits and types.
Provider registry and factory.
CLI skeleton with clap (help texts, no real logic).
BDD feature files written.
Phase 2: Porkbun Provider & CLI Integration (week 2)

Porkbun client implementation.
Mock tests for all trait methods.
Wire up CLI commands (register, dns, ns, check, info, etc.) for Porkbun.
JSON and table output formatters.
Integration tests.
Phase 3: Additional Providers & Extensions (week 3)

Implement Cloudflare, GoDaddy, Namecheap clients.
Extensions directory documentation and template.
Unified error handling and edge cases.
Phase 4: Scheduler & Notifications (week 4)

Scheduler daemon mode.
One-shot expiry check.
Stdout and webhook notifiers.
Tests using fake time (tokio::time::advance).
Phase 5: Polish, CI/CD, Documentation (week 5)

GitHub Actions release workflow with multi‑arch builds.
Installer script.
User documentation (README, examples).
Performance tweaks, linting, and final BDD runs.
15. Clarifications Incorporated

Scheduler: both daemon (schedule) and one-shot (run-check) are configurable.
Notifications: only stdout and generic webhook initially.
Stacker integration: JSON output mode serves as the parser; future stacker domain subcommand will reuse library code.
Provider credentials: one set per provider in config/ENV, no multi‑account support.
CI/CD: GitHub Actions matrix build for amd64/arm64, plus a curl installer.
This plan provides a complete blueprint. All tasks are broken down with file placement, types, and testing approach so that development can commence immediately.


