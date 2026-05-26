use crate::cli::args::{DnsCommand, DnsSubcommand, GlobalArgs};
use crate::cli::commands::output::{print_dns_records, OutputFormat};
use crate::core::{registry::ProviderRegistry, error::Error, types::DnsRecord};

pub async fn run(args: &DnsCommand, global: &GlobalArgs, registry: &ProviderRegistry) -> Result<(), Error> {
    let provider_name = global.provider.as_deref().unwrap_or("porkbun");
    let provider = registry.get(provider_name)?;

    match &args.subcommand {
        DnsSubcommand::List { domain } => {
            let records = provider.list_dns_records(domain).await?;
            print_dns_records(&records, &global.output);
        }
        DnsSubcommand::Add { domain, record_type, name, content, ttl, priority } => {
            let record = DnsRecord {
                id: None,
                record_type: record_type.clone(),
                name: name.clone(),
                content: content.clone(),
                ttl: *ttl,
                priority: *priority,
            };
            let created = provider.create_dns_record(domain, &record).await?;
            println!("Created record with ID: {}", created.id.unwrap_or_else(|| "unknown".to_string()));
        }
        DnsSubcommand::Update { domain, id, record_type, name, content, ttl, priority } => {
            let record = DnsRecord {
                id: Some(id.clone()),
                record_type: record_type.clone(),
                name: name.clone(),
                content: content.clone(),
                ttl: *ttl,
                priority: *priority,
            };
            provider.update_dns_record(domain, id, &record).await?;
            println!("Updated record {id}");
        }
        DnsSubcommand::Delete { domain, id } => {
            provider.delete_dns_record(domain, id).await?;
            println!("Deleted record {id}");
        }
    }
    Ok(())
}
