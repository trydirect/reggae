use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use crate::cli::commands::output::OutputFormat;
use crate::core::types::DnsRecordType;

#[derive(Parser, Debug)]
#[command(
    name = "reggae",
    version,
    about = "Cross-registrar domain manager CLI",
    long_about = "Manage domains and DNS records across multiple registrar providers.\nCredentials are loaded from environment variables or config.yaml."
)]
pub struct Cli {
    /// Path to config file (default: ./config.yaml or ~/.reggae/config.yaml)
    #[arg(long, global = true, env = "DM_CONFIG")]
    pub config: Option<PathBuf>,

    /// Override the default provider
    #[arg(long, short = 'p', global = true, env = "DM_DEFAULT_PROVIDER")]
    pub provider: Option<String>,

    /// Output format
    #[arg(long, short = 'o', global = true, default_value = "table")]
    pub output: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

/// Re-export global flags so command handlers can access them.
#[derive(Debug, Clone)]
pub struct GlobalArgs {
    pub provider: Option<String>,
    pub output: OutputFormat,
}

impl From<&Cli> for GlobalArgs {
    fn from(cli: &Cli) -> Self {
        Self {
            provider: cli.provider.clone(),
            output: cli.output.clone(),
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Check if a domain is available for registration
    Check(CheckArgs),
    /// Register a new domain
    Register(RegisterArgs),
    /// Get domain information (status, expiry, nameservers)
    Info(InfoArgs),
    /// Renew a domain registration
    Renew(RenewArgs),
    /// Initiate a domain transfer
    Transfer(TransferArgs),
    /// Manage DNS records
    Dns(DnsCommand),
    /// Manage nameservers
    Ns(NsCommand),
    /// Get pricing for a domain TLD
    Pricing(PricingArgs),
    /// Start background expiry notification daemon
    Schedule,
    /// Run a one-shot expiry check (suitable for cron / systemd timers)
    RunCheck,
    /// Generate shell completions
    Completion(CompletionArgs),
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Domain name to check (e.g., example.com)
    #[arg(required = true)]
    pub domain: String,
}

#[derive(Args, Debug)]
pub struct RegisterArgs {
    /// Domain name to register
    #[arg(long, required = true)]
    pub domain: String,

    /// Number of years to register for
    #[arg(long, default_value = "1")]
    pub years: u32,

    /// Add an A record pointing to this IP
    #[arg(long = "A", value_name = "IP")]
    pub a_records: Vec<String>,

    /// Add an AAAA record pointing to this IPv6 address
    #[arg(long = "AAAA", value_name = "IP")]
    pub aaaa_records: Vec<String>,

    /// Add a CNAME record pointing to this target
    #[arg(long = "CNAME", value_name = "TARGET")]
    pub cname_records: Vec<String>,

    /// Add a TXT record with this value
    #[arg(long = "TXT", value_name = "VALUE")]
    pub txt_records: Vec<String>,
}

#[derive(Args, Debug)]
pub struct InfoArgs {
    /// Domain name
    #[arg(required = true)]
    pub domain: String,
}

#[derive(Args, Debug)]
pub struct RenewArgs {
    /// Domain name
    #[arg(long, required = true)]
    pub domain: String,

    /// Number of years to renew
    #[arg(long, default_value = "1")]
    pub years: u32,
}

#[derive(Args, Debug)]
pub struct TransferArgs {
    /// Domain name
    #[arg(long, required = true)]
    pub domain: String,

    /// Authorization code from the losing registrar
    #[arg(long, required = true)]
    pub auth_code: String,
}

#[derive(Args, Debug)]
pub struct DnsCommand {
    #[command(subcommand)]
    pub subcommand: DnsSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum DnsSubcommand {
    /// List all DNS records for a domain
    List {
        /// Domain name
        #[arg(long, required = true)]
        domain: String,
    },
    /// Add a new DNS record
    Add {
        /// Domain name
        #[arg(long, required = true)]
        domain: String,
        /// Record type (A, AAAA, CNAME, MX, TXT, etc.)
        #[arg(long, required = true)]
        record_type: DnsRecordType,
        /// Record name (use @ for apex)
        #[arg(long, required = true)]
        name: String,
        /// Record content / value
        #[arg(long, required = true)]
        content: String,
        /// TTL in seconds
        #[arg(long)]
        ttl: Option<u32>,
        /// MX/SRV priority
        #[arg(long)]
        priority: Option<u16>,
    },
    /// Update an existing DNS record
    Update {
        /// Domain name
        #[arg(long, required = true)]
        domain: String,
        /// Record ID
        #[arg(long, required = true)]
        id: String,
        /// Record type
        #[arg(long, required = true)]
        record_type: DnsRecordType,
        /// Record name
        #[arg(long, required = true)]
        name: String,
        /// Record content
        #[arg(long, required = true)]
        content: String,
        /// TTL in seconds
        #[arg(long)]
        ttl: Option<u32>,
        /// Priority
        #[arg(long)]
        priority: Option<u16>,
    },
    /// Delete a DNS record by ID
    Delete {
        /// Domain name
        #[arg(long, required = true)]
        domain: String,
        /// Record ID
        #[arg(long, required = true)]
        id: String,
    },
}

#[derive(Args, Debug)]
pub struct NsCommand {
    #[command(subcommand)]
    pub subcommand: NsSubcommand,
}

#[derive(Subcommand, Debug)]
pub enum NsSubcommand {
    /// Get current nameservers for a domain
    Get {
        #[arg(required = true)]
        domain: String,
    },
    /// Set nameservers for a domain
    Set {
        #[arg(required = true)]
        domain: String,
        /// Nameservers (space-separated)
        #[arg(required = true, num_args = 1..)]
        nameservers: Vec<String>,
    },
}

#[derive(Args, Debug)]
pub struct PricingArgs {
    /// TLD to query pricing for (e.g., com, net, io)
    #[arg(required = true)]
    pub tld: String,
}

#[derive(Args, Debug)]
pub struct CompletionArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}
