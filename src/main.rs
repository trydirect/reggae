use anyhow::Result;
use clap::Parser;
use clap_complete::generate;
use std::io;

use reggae::{
    cli::args::{Cli, Command, GlobalArgs},
    cli::commands::{check, dns, nameserver, output::print_domain, output::print_pricing, register, schedule},
    config::Settings,
    core::registry::ProviderRegistry,
    providers::{
        cloudflare::CloudflareClient,
        godaddy::GodaddyClient,
        namecheap::NamecheapClient,
        porkbun::PorkbunClient,
    },
    utils::build_http_client,
};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let settings = Settings::load(cli.config.as_deref())
        .map_err(|e| anyhow::anyhow!("Config error: {}", e))?;

    // Initialise tracing
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new(&settings.logging.level)
        });
    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Optionally load .env file (non-fatal)
    let _ = dotenvy::dotenv();

    let http = build_http_client();
    let mut registry = ProviderRegistry::new();

    // Register providers that have credentials configured
    if !settings.providers.porkbun.api_key.is_empty() {
        registry.register(
            "porkbun",
            Box::new(PorkbunClient::new(
                settings.providers.porkbun.api_key.clone(),
                settings.providers.porkbun.secret_api_key.clone(),
                http.clone(),
            )),
        );
    }
    if !settings.providers.cloudflare.api_token.is_empty() {
        registry.register(
            "cloudflare",
            Box::new(CloudflareClient::new(
                settings.providers.cloudflare.api_token.clone(),
                settings.providers.cloudflare.account_id.clone(),
                http.clone(),
            )),
        );
    }
    if !settings.providers.godaddy.api_key.is_empty() {
        registry.register(
            "godaddy",
            Box::new(GodaddyClient::new(
                settings.providers.godaddy.api_key.clone(),
                settings.providers.godaddy.api_secret.clone(),
                settings.providers.godaddy.consent_ip.clone(),
                settings.providers.godaddy.contact.clone(),
                settings.providers.godaddy.sandbox,
                http.clone(),
            )),
        );
    }
    if !settings.providers.namecheap.api_key.is_empty() {
        registry.register(
            "namecheap",
            Box::new(NamecheapClient::new(
                settings.providers.namecheap.api_user.clone(),
                settings.providers.namecheap.api_key.clone(),
                settings.providers.namecheap.username.clone(),
                settings.providers.namecheap.client_ip.clone(),
                settings.providers.namecheap.contact.clone(),
                settings.providers.namecheap.sandbox,
                http.clone(),
            )),
        );
    }

    let global = GlobalArgs::from(&cli);

    match &cli.command {
        Command::Check(args) => {
            check::run(args, &global, &registry, &settings).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::Register(args) => {
            register::run(args, &global, &registry, &settings).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::Info(args) => {
            let provider_name = global.provider.as_deref()
                .unwrap_or(&settings.default_provider);
            let provider = registry.get(provider_name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let domain = provider.get_domain_info(&args.domain).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            print_domain(&domain, &global.output);
        }
        Command::Renew(args) => {
            let provider_name = global.provider.as_deref()
                .unwrap_or(&settings.default_provider);
            let provider = registry.get(provider_name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            provider.renew_domain(&args.domain, args.years).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("Domain '{}' renewed for {} year(s)", args.domain, args.years);
        }
        Command::Transfer(args) => {
            let provider_name = global.provider.as_deref()
                .unwrap_or(&settings.default_provider);
            let provider = registry.get(provider_name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            provider.transfer_domain(&args.domain, &args.auth_code).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            println!("Transfer initiated for '{}'", args.domain);
        }
        Command::Dns(args) => {
            dns::run(args, &global, &registry, &settings).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::Ns(args) => {
            nameserver::run(args, &global, &registry, &settings).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::Pricing(args) => {
            let provider_name = global.provider.as_deref()
                .unwrap_or(&settings.default_provider);
            let provider = registry.get(provider_name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            let pricing = provider.get_pricing(&args.tld).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            print_pricing(&pricing, &args.tld, &global.output);
        }
        Command::Schedule => {
            schedule::run_daemon(&settings, &registry).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::RunCheck => {
            schedule::run_once(&settings, &registry).await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
        }
        Command::Completion(args) => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            generate(args.shell, &mut cmd, "reggae", &mut io::stdout());
        }
    }

    Ok(())
}
