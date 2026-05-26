use crate::cli::args::{NsCommand, NsSubcommand, GlobalArgs};
use crate::cli::commands::output::print_nameservers;
use crate::config::Settings;
use crate::core::{registry::ProviderRegistry, error::Error};

pub async fn run(args: &NsCommand, global: &GlobalArgs, registry: &ProviderRegistry, settings: &Settings) -> Result<(), Error> {
    let provider_name = global.provider.as_deref().unwrap_or(&settings.default_provider);
    let provider = registry.get(provider_name)?;

    match &args.subcommand {
        NsSubcommand::Get { domain } => {
            let ns = provider.get_nameservers(domain).await?;
            print_nameservers(&ns, domain, &global.output);
        }
        NsSubcommand::Set { domain, nameservers } => {
            provider.set_nameservers(domain, nameservers).await?;
            println!("Nameservers updated for {domain}");
        }
    }
    Ok(())
}
