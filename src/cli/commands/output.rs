use serde::Serialize;
use tabled::{Table, Tabled};
use crate::core::types::{Domain, DnsRecord, Availability, Pricing};

#[derive(Debug, Clone, clap::ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}

pub fn print_json<T: Serialize>(value: &T) {
    println!("{}", serde_json::to_string_pretty(value).unwrap_or_default());
}

// DNS records table row
#[derive(Tabled)]
struct DnsRow {
    #[tabled(rename = "ID")]
    id: String,
    #[tabled(rename = "Type")]
    record_type: String,
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Content")]
    content: String,
    #[tabled(rename = "TTL")]
    ttl: String,
    #[tabled(rename = "Priority")]
    priority: String,
}

pub fn print_dns_records(records: &[DnsRecord], format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(&records.to_vec()),
        OutputFormat::Table => {
            let rows: Vec<DnsRow> = records.iter().map(|r| DnsRow {
                id: r.id.clone().unwrap_or_else(|| "-".to_string()),
                record_type: r.record_type.to_string(),
                name: r.name.clone(),
                content: r.content.clone(),
                ttl: r.ttl.map(|t| t.to_string()).unwrap_or_else(|| "-".to_string()),
                priority: r.priority.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string()),
            }).collect();
            println!("{}", Table::new(rows));
        }
    }
}

#[derive(Tabled)]
struct DomainRow {
    #[tabled(rename = "Name")]
    name: String,
    #[tabled(rename = "Status")]
    status: String,
    #[tabled(rename = "Expires")]
    expiry: String,
    #[tabled(rename = "Nameservers")]
    nameservers: String,
}

pub fn print_domain(domain: &Domain, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(domain),
        OutputFormat::Table => {
            let row = DomainRow {
                name: domain.name.clone(),
                status: domain.status.to_string(),
                expiry: domain.expiry_date
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
                nameservers: domain.nameservers.join(", "),
            };
            println!("{}", Table::new(vec![row]));
        }
    }
}

pub fn print_availability(avail: &Availability, domain: &str, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(avail),
        OutputFormat::Table => {
            let status = if avail.available { "✓ Available" } else { "✗ Taken" };
            let price = avail.price
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "N/A".to_string());
            println!("{domain}: {status}  (price: {price})");
        }
    }
}

pub fn print_pricing(pricing: &Pricing, tld: &str, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(pricing),
        OutputFormat::Table => {
            println!("Pricing for .{tld} ({}):", pricing.currency);
            println!("  Registration : ${:.2}", pricing.registration_price);
            println!("  Renewal      : ${:.2}", pricing.renewal_price);
            println!("  Transfer     : ${:.2}", pricing.transfer_price);
        }
    }
}

pub fn print_nameservers(nameservers: &[String], domain: &str, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(&nameservers.to_vec()),
        OutputFormat::Table => {
            println!("Nameservers for {domain}:");
            for ns in nameservers {
                println!("  {ns}");
            }
        }
    }
}
