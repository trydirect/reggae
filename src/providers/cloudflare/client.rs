use reqwest::Client;
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::instrument;
use chrono::DateTime;

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const BASE_URL: &str = "https://api.cloudflare.com/client/v4";

pub struct CloudflareClient {
    api_token:  String,
    account_id: String,
    base_url:   String,
    http:       Client,
}

impl CloudflareClient {
    pub fn new(api_token: String, account_id: String, http: Client) -> Self {
        Self::with_base_url(api_token, account_id, http, BASE_URL.to_string())
    }

    pub fn with_base_url(api_token: String, account_id: String, http: Client, base_url: String) -> Self {
        Self { api_token, account_id, base_url, http }
    }

    async fn get_zone_id(&self, domain: &str) -> Result<String, Error> {
        let url = format!("{}/zones?name={}", self.base_url, domain);
        let val: Value = self.http.get(&url)
            .bearer_auth(&self.api_token)
            .send().await?.json().await?;
        check_cf_error(&val)?;
        val["result"].as_array()
            .and_then(|a| a.first())
            .and_then(|z| z["id"].as_str())
            .map(String::from)
            .ok_or_else(|| Error::Provider(format!("Zone not found for '{}'", domain)))
    }

    async fn get_or_create_zone(&self, domain: &str) -> Result<String, Error> {
        match self.get_zone_id(domain).await {
            Ok(id) => Ok(id),
            Err(_) => {
                let url = format!("{}/zones", self.base_url);
                let body = json!({
                    "name": domain,
                    "account": { "id": self.account_id },
                    "jump_start": true
                });
                let val: Value = self.http.post(&url)
                    .bearer_auth(&self.api_token)
                    .json(&body)
                    .send().await?.json().await?;
                check_cf_error(&val)?;
                val["result"]["id"].as_str()
                    .map(String::from)
                    .ok_or_else(|| Error::Provider("No zone id in create response".to_string()))
            }
        }
    }

    fn map_dns_record(r: &Value) -> Option<DnsRecord> {
        let record_type = match r["type"].as_str()? {
            "A"     => DnsRecordType::A,
            "AAAA"  => DnsRecordType::Aaaa,
            "CNAME" => DnsRecordType::Cname,
            "MX"    => DnsRecordType::Mx,
            "TXT"   => DnsRecordType::Txt,
            "NS"    => DnsRecordType::Ns,
            "SRV"   => DnsRecordType::Srv,
            "CAA"   => DnsRecordType::Caa,
            _       => return None,
        };
        Some(DnsRecord {
            id:          r["id"].as_str().map(String::from),
            record_type,
            name:        r["name"].as_str()?.to_string(),
            content:     r["content"].as_str()?.to_string(),
            ttl:         r["ttl"].as_u64().map(|t| t as u32),
            priority:    r["priority"].as_u64().map(|p| p as u16),
        })
    }
}

fn check_cf_error(val: &Value) -> Result<(), Error> {
    if val["success"].as_bool() == Some(false) {
        let msg = val["errors"].as_array()
            .and_then(|e| e.first())
            .and_then(|e| e["message"].as_str())
            .unwrap_or("unknown error")
            .to_string();
        return Err(Error::Api { status: 400, message: msg });
    }
    Ok(())
}

#[async_trait]
impl DomainRegistrar for CloudflareClient {
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        if self.account_id.is_empty() {
            return Err(Error::Config(
                "account_id is required for Cloudflare Registrar operations".to_string()
            ));
        }
        let url = format!(
            "{}/accounts/{}/registrar/domains/{}",
            self.base_url, self.account_id, domain
        );
        let resp = self.http.get(&url).bearer_auth(&self.api_token).send().await?;
        let status = resp.status().as_u16();
        let val: Value = resp.json().await?;
        if status == 404 {
            return Ok(Availability { available: true, premium: false, price: None });
        }
        check_cf_error(&val)?;
        Ok(Availability { available: false, premium: false, price: None })
    }

    /// Add the domain to Cloudflare by creating a zone.
    /// For actual domain purchase use Cloudflare Registrar via the dashboard.
    #[instrument(skip(self, records))]
    async fn register_domain(&self, domain: &str, _years: u32, _contact: &RegistrantContact, records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        if self.account_id.is_empty() {
            return Err(Error::Config("account_id is required to create a Cloudflare zone".to_string()));
        }
        let zone_id = self.get_or_create_zone(domain).await?;

        if let Some(recs) = records {
            for rec in recs {
                self.create_dns_record(domain, &rec).await?;
            }
        }

        let url = format!("{}/zones/{}", self.base_url, zone_id);
        let val: Value = self.http.get(&url).bearer_auth(&self.api_token).send().await?.json().await?;
        check_cf_error(&val)?;
        let ns: Vec<String> = val["result"]["name_servers"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        Ok(Domain { name: domain.to_string(), expiry_date: None, status: DomainStatus::Active, nameservers: ns })
    }

    async fn renew_domain(&self, domain: &str, _years: u32) -> Result<(), Error> {
        if self.account_id.is_empty() {
            return Err(Error::Config("account_id required for registrar API".to_string()));
        }
        let url = format!("{}/accounts/{}/registrar/domains/{}", self.base_url, self.account_id, domain);
        let val: Value = self.http.put(&url).bearer_auth(&self.api_token)
            .json(&json!({ "auto_renew": true })).send().await?.json().await?;
        check_cf_error(&val)?;
        Ok(())
    }

    async fn transfer_domain(&self, domain: &str, _auth_code: &str) -> Result<(), Error> {
        if self.account_id.is_empty() {
            return Err(Error::Config("account_id required for registrar API".to_string()));
        }
        let url = format!("{}/accounts/{}/registrar/domains/{}/transfer", self.base_url, self.account_id, domain);
        let val: Value = self.http.post(&url).bearer_auth(&self.api_token)
            .json(&json!({})).send().await?.json().await?;
        check_cf_error(&val)?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        if !self.account_id.is_empty() {
            let url = format!("{}/accounts/{}/registrar/domains/{}", self.base_url, self.account_id, domain);
            let resp = self.http.get(&url).bearer_auth(&self.api_token).send().await?;
            if resp.status().is_success() {
                let val: Value = resp.json().await?;
                if check_cf_error(&val).is_ok() {
                    let r = &val["result"];
                    let expiry = r["expires_at"].as_str()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc));
                    let ns: Vec<String> = r["name_servers"].as_array()
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    return Ok(Domain { name: domain.to_string(), expiry_date: expiry, status: DomainStatus::Active, nameservers: ns });
                }
            }
        }
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}", self.base_url, zone_id);
        let val: Value = self.http.get(&url).bearer_auth(&self.api_token).send().await?.json().await?;
        check_cf_error(&val)?;
        let ns: Vec<String> = val["result"]["name_servers"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        Ok(Domain { name: domain.to_string(), expiry_date: None, status: DomainStatus::Active, nameservers: ns })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let mut records = Vec::new();
        let mut page = 1u32;
        loop {
            let url = format!("{}/zones/{}/dns_records?per_page=100&page={}", self.base_url, zone_id, page);
            let val: Value = self.http.get(&url).bearer_auth(&self.api_token).send().await?.json().await?;
            check_cf_error(&val)?;
            let batch: Vec<DnsRecord> = val["result"].as_array()
                .map(|a| a.iter().filter_map(Self::map_dns_record).collect())
                .unwrap_or_default();
            let batch_len = batch.len();
            records.extend(batch);
            let total_pages = val["result_info"]["total_pages"].as_u64().unwrap_or(1);
            if page as u64 >= total_pages || batch_len == 0 { break; }
            page += 1;
        }
        Ok(records)
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records", self.base_url, zone_id);
        let body = json!({
            "type":     record.record_type.to_string(),
            "name":     record.name,
            "content":  record.content,
            "ttl":      record.ttl.unwrap_or(1),
            "priority": record.priority,
            "proxied":  false,
        });
        let val: Value = self.http.post(&url).bearer_auth(&self.api_token).json(&body).send().await?.json().await?;
        check_cf_error(&val)?;
        let mut created = record.clone();
        created.id = val["result"]["id"].as_str().map(String::from);
        Ok(created)
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records/{}", self.base_url, zone_id, record_id);
        let body = json!({
            "type":     record.record_type.to_string(),
            "name":     record.name,
            "content":  record.content,
            "ttl":      record.ttl.unwrap_or(1),
            "priority": record.priority,
        });
        let val: Value = self.http.patch(&url).bearer_auth(&self.api_token).json(&body).send().await?.json().await?;
        check_cf_error(&val)?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let url = format!("{}/zones/{}/dns_records/{}", self.base_url, zone_id, record_id);
        let val: Value = self.http.delete(&url).bearer_auth(&self.api_token).send().await?.json().await?;
        check_cf_error(&val)?;
        Ok(())
    }

    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        Ok(self.get_domain_info(domain).await?.nameservers)
    }

    async fn set_nameservers(&self, _domain: &str, _nameservers: &[String]) -> Result<(), Error> {
        // Cloudflare assigns nameservers automatically; custom NS not supported via API
        Err(Error::Unsupported)
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn client(base_url: &str) -> CloudflareClient {
        CloudflareClient::with_base_url(
            "test_token".to_string(), "acc123".to_string(),
            reqwest::Client::new(), base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones?name=example.com")
            .with_body(r#"{"success":true,"result":[{"id":"zone123"}],"result_info":{"total_pages":1}}"#)
            .create_async().await;
        let _m2 = server.mock("GET", "/zones/zone123/dns_records?per_page=100&page=1")
            .with_body(r#"{"success":true,"result":[{"id":"r1","type":"A","name":"example.com","content":"1.2.3.4","ttl":300}],"result_info":{"total_pages":1}}"#)
            .create_async().await;
        let c = client(&server.url());
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones?name=example.com")
            .with_body(r#"{"success":true,"result":[{"id":"zone123"}]}"#)
            .create_async().await;
        let _m2 = server.mock("POST", "/zones/zone123/dns_records")
            .with_body(r#"{"success":true,"result":{"id":"rec456"}}"#)
            .create_async().await;
        let c = client(&server.url());
        let rec = DnsRecord { id: None, record_type: DnsRecordType::A, name: "@".into(), content: "5.5.5.5".into(), ttl: Some(300), priority: None };
        let created = c.create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.unwrap(), "rec456");
    }

    #[tokio::test]
    async fn test_register_domain_creates_zone() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones?name=new.com")
            .with_body(r#"{"success":true,"result":[]}"#)
            .create_async().await;
        let _m2 = server.mock("POST", "/zones")
            .with_body(r#"{"success":true,"result":{"id":"zone789"}}"#)
            .create_async().await;
        let _m3 = server.mock("GET", "/zones/zone789")
            .with_body(r#"{"success":true,"result":{"id":"zone789","name_servers":["ns1.cf.com","ns2.cf.com"]}}"#)
            .create_async().await;
        let c = client(&server.url());
        let domain = c.register_domain("new.com", 1, &RegistrantContact::default(), None).await.unwrap();
        assert_eq!(domain.nameservers, vec!["ns1.cf.com", "ns2.cf.com"]);
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones?name=example.com")
            .with_body(r#"{"success":true,"result":[{"id":"zone123"}]}"#)
            .create_async().await;
        let _m2 = server.mock("DELETE", "/zones/zone123/dns_records/rec1")
            .with_body(r#"{"success":true,"result":{"id":"rec1"}}"#)
            .create_async().await;
        let c = client(&server.url());
        c.delete_dns_record("example.com", "rec1").await.unwrap();
    }
}
