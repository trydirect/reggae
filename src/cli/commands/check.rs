use crate::cli::args::{CheckArgs, GlobalArgs};
use crate::cli::commands::output::print_availability;
use crate::config::Settings;
use crate::core::{registry::ProviderRegistry, error::Error};

pub async fn run(args: &CheckArgs, global: &GlobalArgs, registry: &ProviderRegistry, settings: &Settings) -> Result<(), Error> {
    let provider_name = global.provider.as_deref()
        .unwrap_or(&settings.default_provider);
    let provider = registry.get(provider_name)?;
    let avail = provider.check_availability(&args.domain).await?;
    print_availability(&avail, &args.domain, &global.output);
    Ok(())
}
