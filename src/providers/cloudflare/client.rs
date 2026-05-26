use reqwest::Client;
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::debug;

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const BASE_URL: &str = "https://api.cloudflare.com/client/v4";

pub struct CloudflareClient {
    api_token: String,
    base_url: String,
    http: Client,
}

impl CloudflareClient {
    pub fn new(api_token: String, http: Client) -> Self {
        Self { api_token, base_url: BASE_URL.to_string(), http }
    }

    pub fn with_base_url(api_token: String, http: Client, base_url: String) -> Self {
        Self { api_token, base_url, http }
    }

    async fn get_zone_id(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{}/zones?name={}", self.base_url, domain);
        let resp = self.http.get(&url)
            .bearer_auth(&self.api_token)
            .send()
            .await?;
        let val: Value = resp.json().await?;
        val.get("result")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|z| z.get("id"))
            .and_then(|id| id.as_str())
            .map(String::from)
            .ok_or_else(|| Error::Provider(format!("Zone not found for domain '{}'", domain)))
    }
}

#[async_trait]
impl DomainRegistrar for CloudflareClient {
    async fn check_availability(&self, _domain: &str) -> Result<Availability, Error> {
        Err(Error::Unsupported)
    }

    async fn register_domain(&self, _domain: &str, _records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        Err(Error::Unsupported)
    }

    async fn renew_domain(&self, _domain: &str, _years: u32) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn transfer_domain(&self, _domain: &str, _auth_code: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}", self.base_url, zone_id);
        let resp = self.http.get(&url).bearer_auth(&self.api_token).send().await?;
        let val: Value = resp.json().await?;
        let result = val.get("result").ok_or_else(|| Error::Provider("No result field".to_string()))?;
        Ok(Domain {
            name: domain.to_string(),
            expiry_date: None,
            status: DomainStatus::Active,
            nameservers: result.get("name_servers")
                .and_then(|ns| ns.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        })
    }

    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records", self.base_url, zone_id);
        let resp = self.http.get(&url).bearer_auth(&self.api_token).send().await?;
        let val: Value = resp.json().await?;
        let records: Vec<DnsRecord> = val.get("result")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|r| {
                let record_type = match r.get("type")?.as_str()? {
                    "A" => DnsRecordType::A,
                    "AAAA" => DnsRecordType::Aaaa,
                    "CNAME" => DnsRecordType::Cname,
                    "MX" => DnsRecordType::Mx,
                    "TXT" => DnsRecordType::Txt,
                    "NS" => DnsRecordType::Ns,
                    _ => return None,
                };
                Some(DnsRecord {
                    id: r.get("id").and_then(|v| v.as_str()).map(String::from),
                    record_type,
                    name: r.get("name")?.as_str()?.to_string(),
                    content: r.get("content")?.as_str()?.to_string(),
                    ttl: r.get("ttl").and_then(|v| v.as_u64()).map(|t| t as u32),
                    priority: r.get("priority").and_then(|v| v.as_u64()).map(|p| p as u16),
                })
            }).collect())
            .unwrap_or_default();
        Ok(records)
    }

    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records", self.base_url, zone_id);
        let body = json!({
            "type": record.record_type.to_string(),
            "name": record.name,
            "content": record.content,
            "ttl": record.ttl.unwrap_or(1),
            "priority": record.priority
        });
        let resp = self.http.post(&url).bearer_auth(&self.api_token).json(&body).send().await?;
        let val: Value = resp.json().await?;
        let mut created = record.clone();
        created.id = val.get("result").and_then(|r| r.get("id")).and_then(|id| id.as_str()).map(String::from);
        Ok(created)
    }

    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records/{}", self.base_url, zone_id, record_id);
        let body = json!({
            "type": record.record_type.to_string(),
            "name": record.name,
            "content": record.content,
            "ttl": record.ttl.unwrap_or(1),
        });
        self.http.put(&url).bearer_auth(&self.api_token).json(&body).send().await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records/{}", self.base_url, zone_id, record_id);
        self.http.delete(&url).bearer_auth(&self.api_token).send().await?;
        Ok(())
    }

    async fn set_nameservers(&self, _domain: &str, _nameservers: &[String]) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let info = self.get_domain_info(domain).await?;
        Ok(info.nameservers)
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Unsupported)
    }
}
