use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, instrument};

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const PROD_BASE_URL: &str = "https://api.name.com/v4";
const SANDBOX_BASE_URL: &str = "https://api.dev.name.com/v4";

pub struct NamecomClient {
    username: String,
    api_token: String,
    base_url: String,
    http: Client,
}

impl NamecomClient {
    pub fn new(username: String, api_token: String, sandbox: bool, http: Client) -> Self {
        let base_url = if sandbox { SANDBOX_BASE_URL } else { PROD_BASE_URL }.to_string();
        Self::with_base_url(username, api_token, http, base_url)
    }

    pub fn with_base_url(username: String, api_token: String, http: Client, base_url: String) -> Self {
        Self { username, api_token, base_url, http }
    }

    fn req_get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);
        self.http.get(url).basic_auth(&self.username, Some(&self.api_token))
    }

    fn req_post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);
        self.http.post(url).basic_auth(&self.username, Some(&self.api_token))
    }

    fn req_put(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!("PUT {}", url);
        self.http.put(url).basic_auth(&self.username, Some(&self.api_token))
    }

    fn req_delete(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);
        self.http.delete(url).basic_auth(&self.username, Some(&self.api_token))
    }

    async fn send_json(&self, rb: reqwest::RequestBuilder) -> Result<Value, Error> {
        let resp = rb.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response {}: {}", status, text);
        if !status.is_success() {
            let msg = namecom_error(&text);
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        if text.is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(&text).map_err(Error::Parse)
        }
    }
}

fn namecom_error(text: &str) -> String {
    if let Ok(val) = serde_json::from_str::<Value>(text) {
        if let Some(s) = val.get("message").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        if let Some(s) = val.get("details").and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    text.to_string()
}

// Name.com DNS record wire type
#[derive(Debug, Deserialize)]
struct NamecomRecord {
    id: Option<u64>,
    #[serde(rename = "type")]
    record_type: String,
    host: Option<String>,
    answer: String,
    ttl: Option<u32>,
    priority: Option<u16>,
}

fn map_record(r: NamecomRecord) -> DnsRecord {
    let rtype = match r.record_type.as_str() {
        "A"     => DnsRecordType::A,
        "AAAA"  => DnsRecordType::Aaaa,
        "CNAME" => DnsRecordType::Cname,
        "MX"    => DnsRecordType::Mx,
        "TXT"   => DnsRecordType::Txt,
        "NS"    => DnsRecordType::Ns,
        "SRV"   => DnsRecordType::Srv,
        "CAA"   => DnsRecordType::Caa,
        "ALIAS" => DnsRecordType::Alias,
        _       => DnsRecordType::A,
    };
    DnsRecord {
        id:          r.id.map(|n| n.to_string()),
        record_type: rtype,
        name:        r.host.unwrap_or_default(),
        content:     r.answer,
        ttl:         r.ttl,
        priority:    r.priority,
    }
}

fn record_body(record: &DnsRecord) -> Value {
    let mut body = json!({
        "type":   record.record_type.to_string(),
        "host":   record.name,
        "answer": record.content,
        "ttl":    record.ttl.unwrap_or(300),
    });
    if let Some(p) = record.priority {
        body["priority"] = json!(p);
    }
    body
}

#[derive(Debug, Deserialize)]
struct DomainInfo {
    #[serde(rename = "domainName")]
    domain_name: String,
    #[serde(rename = "expireDate")]
    expire_date: Option<String>,
    locked: Option<bool>,
    nameservers: Option<Vec<String>>,
}

fn map_domain_info(info: DomainInfo) -> Domain {
    let expiry = info.expire_date.as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc));
    let status = if info.locked == Some(true) {
        DomainStatus::Active
    } else {
        DomainStatus::Active
    };
    Domain {
        name:        info.domain_name,
        expiry_date: expiry,
        status,
        nameservers: info.nameservers.unwrap_or_default(),
    }
}

#[async_trait]
impl DomainRegistrar for NamecomClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let path = format!("/domains:search?domainName={}", domain);
        let val = self.send_json(self.req_get(&path)).await?;

        // Results is an array; find the exact domain match.
        let results = val.get("results").and_then(|v| v.as_array());
        if let Some(results) = results {
            for item in results {
                if item.get("domainName").and_then(|v| v.as_str()) == Some(domain) {
                    let purchasable = item.get("purchasable").and_then(|v| v.as_bool()).unwrap_or(false);
                    let premium = item.get("premium").and_then(|v| v.as_bool()).unwrap_or(false);
                    let price = item.get("purchasePrice").and_then(|v| v.as_f64());
                    return Ok(Availability { available: purchasable, premium, price });
                }
            }
        }
        Ok(Availability { available: false, premium: false, price: None })
    }

    #[instrument(skip(self, contact, records))]
    async fn register_domain(
        &self,
        domain: &str,
        years: u32,
        contact: &RegistrantContact,
        records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error> {
        let contact_obj = json!({
            "firstName": contact.first_name,
            "lastName":  contact.last_name,
            "companyName": contact.organization.as_deref().unwrap_or(""),
            "address1":  contact.address1,
            "address2":  contact.address2.as_deref().unwrap_or(""),
            "city":      contact.city,
            "state":     contact.state,
            "zip":       contact.postal_code,
            "country":   contact.country,
            "phone":     contact.phone,
            "email":     contact.email,
        });
        let body = json!({
            "domain": { "domainName": domain },
            "purchasePrice": 0,
            "yearsToRegister": years,
            "privacy": false,
            "contacts": {
                "registrant": contact_obj,
                "admin":      contact_obj,
                "tech":       contact_obj,
                "billing":    contact_obj,
            },
        });
        let val = self.send_json(self.req_post("/domains").json(&body)).await?;
        let info: DomainInfo = serde_json::from_value(
            val.get("domain").cloned().unwrap_or(val)
        ).map_err(Error::Parse)?;
        let domain_result = map_domain_info(info);

        if let Some(dns_records) = records {
            for rec in dns_records {
                self.create_dns_record(domain, &rec).await?;
            }
        }

        Ok(domain_result)
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let path = format!("/domains/{}:renew", domain);
        let body = json!({ "period": years, "purchasePrice": 0 });
        self.send_json(self.req_post(&path).json(&body)).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let body = json!({
            "domainName":       domain,
            "authCode":         auth_code,
            "purchasePrice":    0,
            "yearsToRegister":  1,
        });
        self.send_json(self.req_post("/transfers").json(&body)).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let path = format!("/domains/{}", domain);
        let val = self.send_json(self.req_get(&path)).await?;
        let info: DomainInfo = serde_json::from_value(val).map_err(Error::Parse)?;
        Ok(map_domain_info(info))
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let mut records = Vec::new();
        let mut page: Option<u32> = None;

        loop {
            let path = match page {
                Some(p) => format!("/domains/{}/records?page={}", domain, p),
                None    => format!("/domains/{}/records", domain),
            };
            let val = self.send_json(self.req_get(&path)).await?;
            let batch: Vec<NamecomRecord> = serde_json::from_value(
                val.get("records").cloned().unwrap_or(json!([]))
            ).map_err(Error::Parse)?;
            records.extend(batch.into_iter().map(map_record));

            page = val.get("nextPage").and_then(|v| v.as_u64()).map(|n| n as u32);
            if page.is_none() { break; }
        }
        Ok(records)
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let path = format!("/domains/{}/records", domain);
        let val = self.send_json(self.req_post(&path).json(&record_body(record))).await?;
        let created: NamecomRecord = serde_json::from_value(val).map_err(Error::Parse)?;
        Ok(map_record(created))
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let path = format!("/domains/{}/records/{}", domain, record_id);
        let val = self.send_json(self.req_put(&path).json(&record_body(record))).await?;
        let updated: NamecomRecord = serde_json::from_value(val).map_err(Error::Parse)?;
        Ok(map_record(updated))
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let path = format!("/domains/{}/records/{}", domain, record_id);
        self.send_json(self.req_delete(&path)).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        Ok(self.get_domain_info(domain).await?.nameservers)
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let path = format!("/domains/{}:setNameservers", domain);
        let body = json!({ "nameservers": nameservers });
        self.send_json(self.req_post(&path).json(&body)).await?;
        Ok(())
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        // Name.com v4 has no standalone TLD pricing endpoint; use check_availability
        // to get per-domain pricing when needed.
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn client(base_url: &str) -> NamecomClient {
        NamecomClient::with_base_url(
            "testuser".to_string(),
            "testtoken".to_string(),
            reqwest::Client::new(),
            base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_check_availability_available() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains:search?domainName=example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"results":[{"domainName":"example.com","purchasable":true,"purchasePrice":9.99,"premium":false}]}"#)
            .create_async().await;

        let c = client(&server.url());
        let avail = c.check_availability("example.com").await.unwrap();
        assert!(avail.available);
        assert!(!avail.premium);
        assert_eq!(avail.price, Some(9.99));
    }

    #[tokio::test]
    async fn test_check_availability_taken() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains:search?domainName=taken.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"results":[{"domainName":"taken.com","purchasable":false,"purchasePrice":0,"premium":false}]}"#)
            .create_async().await;

        let c = client(&server.url());
        let avail = c.check_availability("taken.com").await.unwrap();
        assert!(!avail.available);
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"domainName":"example.com","expireDate":"2026-01-01T00:00:00Z","locked":true,"nameservers":["ns1.name.com","ns2.name.com"]}"#)
            .create_async().await;

        let c = client(&server.url());
        let d = c.get_domain_info("example.com").await.unwrap();
        assert_eq!(d.name, "example.com");
        assert!(d.expiry_date.is_some());
        assert_eq!(d.nameservers.len(), 2);
    }

    #[tokio::test]
    async fn test_list_dns_records_single_page() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains/example.com/records")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"records":[
                {"id":1,"type":"A","host":"www","answer":"1.2.3.4","ttl":300},
                {"id":2,"type":"MX","host":"@","answer":"mail.example.com","ttl":3600,"priority":10}
            ]}"#)
            .create_async().await;

        let c = client(&server.url());
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].id.as_deref(), Some("1"));
        assert_eq!(recs[0].content, "1.2.3.4");
        assert_eq!(recs[1].record_type, DnsRecordType::Mx);
        assert_eq!(recs[1].priority, Some(10));
    }

    #[tokio::test]
    async fn test_list_dns_records_paginated() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/domains/example.com/records")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"records":[{"id":1,"type":"A","host":"www","answer":"1.2.3.4","ttl":300}],"nextPage":2}"#)
            .create_async().await;
        let _m2 = server.mock("GET", "/domains/example.com/records?page=2")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"records":[{"id":2,"type":"A","host":"mail","answer":"5.6.7.8","ttl":300}]}"#)
            .create_async().await;

        let c = client(&server.url());
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].name, "mail");
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domains/example.com/records")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":42,"type":"A","host":"www","answer":"1.2.3.4","ttl":300}"#)
            .create_async().await;

        let c = client(&server.url());
        let rec = DnsRecord {
            id: None,
            record_type: DnsRecordType::A,
            name: "www".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: Some(300),
            priority: None,
        };
        let created = c.create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.as_deref(), Some("42"));
        assert_eq!(created.content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_update_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PUT", "/domains/example.com/records/42")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":42,"type":"A","host":"www","answer":"9.9.9.9","ttl":300}"#)
            .create_async().await;

        let c = client(&server.url());
        let rec = DnsRecord {
            id: None,
            record_type: DnsRecordType::A,
            name: "www".to_string(),
            content: "9.9.9.9".to_string(),
            ttl: Some(300),
            priority: None,
        };
        let updated = c.update_dns_record("example.com", "42", &rec).await.unwrap();
        assert_eq!(updated.content, "9.9.9.9");
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("DELETE", "/domains/example.com/records/42")
            .with_status(204)
            .create_async().await;

        let c = client(&server.url());
        c.delete_dns_record("example.com", "42").await.unwrap();
    }

    #[tokio::test]
    async fn test_renew_domain() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domains/example.com:renew")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"order":{"totalPaid":9.99}}"#)
            .create_async().await;

        let c = client(&server.url());
        c.renew_domain("example.com", 1).await.unwrap();
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domains/example.com:setNameservers")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{}"#)
            .create_async().await;

        let c = client(&server.url());
        let ns = vec!["ns1.example.com".to_string(), "ns2.example.com".to_string()];
        c.set_nameservers("example.com", &ns).await.unwrap();
    }

    #[tokio::test]
    async fn test_transfer_domain() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/transfers")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"transfer":{"domainName":"example.com","status":"pending"}}"#)
            .create_async().await;

        let c = client(&server.url());
        c.transfer_domain("example.com", "auth123").await.unwrap();
    }

    #[tokio::test]
    async fn test_api_error() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains/notfound.com")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"Domain not found","details":"Domain does not exist in your account"}"#)
            .create_async().await;

        let c = client(&server.url());
        let err = c.get_domain_info("notfound.com").await.unwrap_err();
        match err {
            Error::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("Domain not found"));
            }
            other => panic!("Expected Api error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_pricing_unsupported() {
        let c = client("http://unused");
        assert!(matches!(c.get_pricing("com").await, Err(Error::Unsupported)));
    }
}
