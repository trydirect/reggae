use crate::cli::args::{RegisterArgs, GlobalArgs};
use crate::cli::commands::output::print_domain;
use crate::config::Settings;
use crate::core::{registry::ProviderRegistry, error::Error, types::{DnsRecord, DnsRecordType}};

pub async fn run(args: &RegisterArgs, global: &GlobalArgs, registry: &ProviderRegistry, settings: &Settings) -> Result<(), Error> {
    let provider_name = global.provider.as_deref()
        .unwrap_or(&settings.default_provider);
    let provider = registry.get(provider_name)?;

    // Build initial DNS records from CLI flags
    let mut records: Vec<DnsRecord> = Vec::new();
    for ip in &args.a_records {
        records.push(DnsRecord { id: None, record_type: DnsRecordType::A, name: "@".to_string(), content: ip.clone(), ttl: Some(300), priority: None });
    }
    for ip in &args.aaaa_records {
        records.push(DnsRecord { id: None, record_type: DnsRecordType::Aaaa, name: "@".to_string(), content: ip.clone(), ttl: Some(300), priority: None });
    }
    for target in &args.cname_records {
        records.push(DnsRecord { id: None, record_type: DnsRecordType::Cname, name: "www".to_string(), content: target.clone(), ttl: Some(300), priority: None });
    }
    for val in &args.txt_records {
        records.push(DnsRecord { id: None, record_type: DnsRecordType::Txt, name: "@".to_string(), content: val.clone(), ttl: Some(300), priority: None });
    }

    let initial = if records.is_empty() { None } else { Some(records) };
    let domain = provider.register_domain(&args.domain, args.years, &settings.registrant, initial).await?;
    print_domain(&domain, &global.output);
    Ok(())
}
