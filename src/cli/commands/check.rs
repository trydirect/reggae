use crate::cli::args::{CheckArgs, GlobalArgs};
use crate::cli::commands::output::{print_availability, OutputFormat};
use crate::core::registry::ProviderRegistry;
use crate::core::error::Error;

pub async fn run(args: &CheckArgs, global: &GlobalArgs, registry: &ProviderRegistry) -> Result<(), Error> {
    let provider_name = global.provider.as_deref()
        .unwrap_or("porkbun");
    let provider = registry.get(provider_name)?;
    let avail = provider.check_availability(&args.domain).await?;
    print_availability(&avail, &args.domain, &global.output);
    Ok(())
}
