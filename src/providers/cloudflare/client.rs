use reqwest::Client;
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::{debug, instrument};

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

    async fn get_val(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);
        let resp = self.http.get(&url).bearer_auth(&self.api_token).send().await?;
        let val: Value = resp.json().await?;
        self.check_cf_error(&val)?;
        Ok(val)
    }

    async fn post_val(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);
        let resp = self.http.post(&url).bearer_auth(&self.api_token).json(&body).send().await?;
        let val: Value = resp.json().await?;
        self.check_cf_error(&val)?;
        Ok(val)
    }

    async fn patch_val(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("PATCH {}", url);
        let resp = self.http.patch(&url).bearer_auth(&self.api_token).json(&body).send().await?;
        let val: Value = resp.json().await?;
        self.check_cf_error(&val)?;
        Ok(val)
    }

    async fn delete_val(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);
        let resp = self.http.delete(&url).bearer_auth(&self.api_token).send().await?;
        let val: Value = resp.json().await?;
        self.check_cf_error(&val)?;
        Ok(val)
    }

    fn check_cf_error(&self, val: &Value) -> Result<(), Error> {
        if val.get("success").and_then(|v| v.as_bool()) == Some(false) {
            let msg = val.get("errors")
                .and_then(|e| e.as_array())
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown Cloudflare error")
                .to_string();
            return Err(Error::Api { status: 400, message: msg });
        }
        Ok(())
    }

    async fn get_zone_id(&self, domain: &str) -> Result<String, Error> {
        for candidate in self.zone_candidates(domain) {
            let path = format!("/zones?name={}&status=active", candidate);
            let val = self.get_val(&path).await?;
            if let Some(zone_id) = val.get("result")
                .and_then(|r| r.as_array())
                .and_then(|arr| arr.first())
                .and_then(|z| z.get("id"))
                .and_then(|id| id.as_str())
            {
                return Ok(zone_id.to_string());
            }
        }
        Err(Error::Provider(format!(
            "No active Cloudflare zone found for '{}'. Add the domain to your Cloudflare account first.",
            domain
        )))
    }

    fn zone_candidates(&self, domain: &str) -> Vec<String> {
        let mut candidates = vec![domain.to_string()];
        let parts: Vec<&str> = domain.splitn(3, '.').collect();
        if parts.len() == 3 {
            candidates.push(format!("{}.{}", parts[1], parts[2]));
        }
        candidates
    }

    fn map_dns_record(r: &Value) -> Option<DnsRecord> {
        let record_type = match r.get("type")?.as_str()? {
            "A" => DnsRecordType::A,
            "AAAA" => DnsRecordType::Aaaa,
            "CNAME" => DnsRecordType::Cname,
            "MX" => DnsRecordType::Mx,
            "TXT" => DnsRecordType::Txt,
            "NS" => DnsRecordType::Ns,
            "SRV" => DnsRecordType::Srv,
            "CAA" => DnsRecordType::Caa,
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
    }
}

#[async_trait]
impl DomainRegistrar for CloudflareClient {
    async fn check_availability(&self, _domain: &str) -> Result<Availability, Error> {
        Err(Error::Provider(
            "Cloudflare does not provide a public domain availability check API. \
             Use a dedicated registrar provider for availability checks."
                .to_string(),
        ))
    }

    /// Cloudflare Registrar does not have a public API for domain registration.
    /// Domains must be registered or transferred via https://dash.cloudflare.com.
    async fn register_domain(
        &self,
        domain: &str,
        _years: u32,
        _contact: &RegistrantContact,
        _records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error> {
        Err(Error::Provider(format!(
            "Cloudflare Registrar does not expose a public registration API for '{}'. \
             Register or transfer the domain via https://dash.cloudflare.com, \
             then use `reggae dns` to manage DNS records.",
            domain
        )))
    }

    async fn renew_domain(&self, domain: &str, _years: u32) -> Result<(), Error> {
        Err(Error::Provider(format!(
            "Cloudflare domain renewal for '{}' must be performed via the dashboard.",
            domain
        )))
    }

    async fn transfer_domain(&self, domain: &str, _auth_code: &str) -> Result<(), Error> {
        Err(Error::Provider(format!(
            "Cloudflare domain transfer for '{}' must be initiated via the dashboard.",
            domain
        )))
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let val = self.get_val(&format!("/zones/{}", zone_id)).await?;
        let result = val.get("result")
            .ok_or_else(|| Error::Provider("No result in zone response".to_string()))?;

        let status = match result.get("status").and_then(|s| s.as_str()) {
            Some("active") => DomainStatus::Active,
            Some("pending") => DomainStatus::Pending,
            Some("suspended") => DomainStatus::Suspended,
            _ => DomainStatus::Unknown,
        };

        let nameservers: Vec<String> = result.get("name_servers")
            .and_then(|ns| ns.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(Domain {
            name: domain.to_string(),
            expiry_date: None, // Not available via Zones API
            status,
            nameservers,
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let mut records = Vec::new();
        let mut page = 1u32;

        loop {
            let path = format!("/zones/{}/dns_records?per_page=100&page={}", zone_id, page);
            let val = self.get_val(&path).await?;
            let result_arr = val.get("result")
                .and_then(|r| r.as_array())
                .ok_or_else(|| Error::Provider("No result array in DNS records response".to_string()))?;

            records.extend(result_arr.iter().filter_map(Self::map_dns_record));

            let total_pages = val.get("result_info")
                .and_then(|ri| ri.get("total_pages"))
                .and_then(|tp| tp.as_u64())
                .unwrap_or(1);
            if page as u64 >= total_pages { break; }
            page += 1;
        }

        Ok(records)
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let mut body = json!({
            "type": record.record_type.to_string(),
            "name": record.name,
            "content": record.content,
            "ttl": record.ttl.unwrap_or(1),
        });
        if let Some(prio) = record.priority {
            body["priority"] = json!(prio);
        }

        let val = self.post_val(&format!("/zones/{}/dns_records", zone_id), body).await?;
        let result = val.get("result").unwrap_or(&val);
        let mut created = record.clone();
        created.id = result.get("id").and_then(|v| v.as_str()).map(String::from);
        Ok(created)
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.get_zone_id(domain).await?;
        let mut body = json!({
            "type": record.record_type.to_string(),
            "name": record.name,
            "content": record.content,
            "ttl": record.ttl.unwrap_or(1),
        });
        if let Some(prio) = record.priority {
            body["priority"] = json!(prio);
        }

        self.patch_val(&format!("/zones/{}/dns_records/{}", zone_id, record_id), body).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let zone_id = self.get_zone_id(domain).await?;
        self.delete_val(&format!("/zones/{}/dns_records/{}", zone_id, record_id)).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let info = self.get_domain_info(domain).await?;
        Ok(info.nameservers)
    }

    async fn set_nameservers(&self, domain: &str, _nameservers: &[String]) -> Result<(), Error> {
        Err(Error::Provider(format!(
            "Cloudflare assigns nameservers automatically for '{}'. They cannot be changed via the API.",
            domain
        )))
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Provider(
            "Cloudflare does not provide a public pricing API. \
             See https://www.cloudflare.com/products/registrar/ for pricing."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn make_client(base_url: &str) -> CloudflareClient {
        CloudflareClient::with_base_url("test_token".to_string(), reqwest::Client::new(), base_url.to_string())
    }

    fn zone_resp(zone_id: &str) -> String {
        format!(r#"{{"success":true,"result":[{{"id":"{zone_id}","name":"example.com","status":"active"}}]}}"#)
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _mz = server.mock("GET", "/zones?name=example.com&status=active")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(zone_resp("z1")).create_async().await;
        let _mr = server.mock("GET", "/zones/z1/dns_records?per_page=100&page=1")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"success":true,"result":[{"id":"r1","type":"A","name":"example.com","content":"1.2.3.4","ttl":300}],"result_info":{"total_pages":1}}"#)
            .create_async().await;

        let records = make_client(&server.url()).list_dns_records("example.com").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _mz = server.mock("GET", "/zones?name=example.com&status=active")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(zone_resp("z1")).create_async().await;
        let _mr = server.mock("POST", "/zones/z1/dns_records")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"success":true,"result":{"id":"newid","type":"A","name":"example.com","content":"5.6.7.8","ttl":300}}"#)
            .create_async().await;

        let record = DnsRecord { id: None, record_type: DnsRecordType::A, name: "example.com".to_string(), content: "5.6.7.8".to_string(), ttl: Some(300), priority: None };
        let created = make_client(&server.url()).create_dns_record("example.com", &record).await.unwrap();
        assert_eq!(created.id.unwrap(), "newid");
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _mz = server.mock("GET", "/zones?name=example.com&status=active")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(zone_resp("z1")).create_async().await;
        let _md = server.mock("DELETE", "/zones/z1/dns_records/r99")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"success":true,"result":{"id":"r99"}}"#)
            .create_async().await;

        make_client(&server.url()).delete_dns_record("example.com", "r99").await.unwrap();
    }

    #[tokio::test]
    async fn test_register_domain_returns_descriptive_error() {
        let contact = RegistrantContact::default();
        let result = make_client("http://unused").register_domain("example.com", 1, &contact, None).await;
        assert!(matches!(result, Err(Error::Provider(_))));
    }

    #[tokio::test]
    async fn test_get_nameservers() {
        let mut server = Server::new_async().await;
        let _mz = server.mock("GET", "/zones?name=example.com&status=active")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(zone_resp("z1")).create_async().await;
        let _mi = server.mock("GET", "/zones/z1")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"success":true,"result":{"id":"z1","name":"example.com","status":"active","name_servers":["ns1.cloudflare.com","ns2.cloudflare.com"]}}"#)
            .create_async().await;

        let ns = make_client(&server.url()).get_nameservers("example.com").await.unwrap();
        assert_eq!(ns.len(), 2);
    }
}
