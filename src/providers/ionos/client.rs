use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, instrument};

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const DNS_BASE: &str = "https://api.hosting.ionos.com/dns/v1";
const DOM_BASE: &str = "https://api.hosting.ionos.com/domains/v1";

pub struct IonosClient {
    api_key: String,
    dns_base: String,
    dom_base: String,
    http: Client,
}

impl IonosClient {
    /// `api_prefix` and `api_secret` are the two parts of the IONOS composite key
    /// (shown as `prefix.secret` on developer.hosting.ionos.com/keys).
    pub fn new(api_prefix: String, api_secret: String, http: Client) -> Self {
        Self::with_base_urls(api_prefix, api_secret, http, DNS_BASE.to_string(), DOM_BASE.to_string())
    }

    pub fn with_base_urls(
        api_prefix: String, api_secret: String, http: Client,
        dns_base: String, dom_base: String,
    ) -> Self {
        let api_key = format!("{}.{}", api_prefix, api_secret);
        Self { api_key, dns_base, dom_base, http }
    }

    fn dns(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dns_base, path);
        debug!("GET/POST/PUT/DELETE {}", url);
        self.http.get(&url).header("X-API-Key", &self.api_key)
    }
    fn dns_get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dns_base, path);
        debug!("GET {}", url);
        self.http.get(url).header("X-API-Key", &self.api_key)
    }
    fn dns_post(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dns_base, path);
        debug!("POST {}", url);
        self.http.post(url).header("X-API-Key", &self.api_key)
    }
    fn dns_put(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dns_base, path);
        debug!("PUT {}", url);
        self.http.put(url).header("X-API-Key", &self.api_key)
    }
    fn dns_delete(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dns_base, path);
        debug!("DELETE {}", url);
        self.http.delete(url).header("X-API-Key", &self.api_key)
    }
    fn dom_get(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dom_base, path);
        debug!("GET {}", url);
        self.http.get(url).header("X-API-Key", &self.api_key)
    }
    fn dom_patch(&self, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{}", self.dom_base, path);
        debug!("PATCH {}", url);
        self.http.patch(url).header("X-API-Key", &self.api_key)
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<Value, Error> {
        let resp = rb.send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response {}: {}", status, &text[..text.len().min(256)]);
        if !status.is_success() {
            let msg = ionos_error(&text);
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        if text.is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(&text).map_err(Error::Parse)
        }
    }

    // Resolves the IONOS zone ID for a domain name.
    async fn zone_id(&self, domain: &str) -> Result<String, Error> {
        let val = self.send(self.dns_get("/zones")).await?;
        let zones = val.as_array()
            .ok_or_else(|| Error::Provider("Expected array from /zones".to_string()))?;
        zones.iter()
            .find(|z| z["name"].as_str() == Some(domain))
            .and_then(|z| z["id"].as_str().map(String::from))
            .ok_or_else(|| Error::Provider(format!("Zone not found for '{}'", domain)))
    }
}

fn ionos_error(text: &str) -> String {
    if let Ok(val) = serde_json::from_str::<Value>(text) {
        if let Some(s) = val.get("message").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        // IONOS sometimes wraps errors in an array
        if let Some(arr) = val.as_array() {
            if let Some(s) = arr.first().and_then(|e| e.get("message")).and_then(|v| v.as_str()) {
                return s.to_string();
            }
        }
    }
    text.to_string()
}

#[derive(Debug, Deserialize)]
struct IonosRecord {
    id: Option<String>,
    #[serde(rename = "type")]
    record_type: String,
    name: String,
    content: String,
    ttl: Option<u32>,
    #[serde(rename = "prio")]
    priority: Option<u16>,
}

fn map_record(r: IonosRecord) -> DnsRecord {
    let rtype = match r.record_type.as_str() {
        "A"     => DnsRecordType::A,
        "AAAA"  => DnsRecordType::Aaaa,
        "CNAME" => DnsRecordType::Cname,
        "MX"    => DnsRecordType::Mx,
        "TXT"   => DnsRecordType::Txt,
        "NS"    => DnsRecordType::Ns,
        "SRV"   => DnsRecordType::Srv,
        "CAA"   => DnsRecordType::Caa,
        _       => DnsRecordType::A,
    };
    DnsRecord { id: r.id, record_type: rtype, name: r.name, content: r.content, ttl: r.ttl, priority: r.priority }
}

fn record_body(domain: &str, zone_id: &str, record: &DnsRecord) -> Value {
    let mut body = json!({
        "name":    record.name,
        "type":    record.record_type.to_string(),
        "content": record.content,
        "ttl":     record.ttl.unwrap_or(3600),
        "zoneId":  zone_id,
    });
    if let Some(p) = record.priority {
        body["prio"] = json!(p);
    }
    let _ = domain; // kept for symmetry
    body
}

#[async_trait]
impl DomainRegistrar for IonosClient {
    async fn check_availability(&self, _domain: &str) -> Result<Availability, Error> {
        // IONOS does not provide a public domain availability check endpoint.
        Err(Error::Unsupported)
    }

    async fn register_domain(&self, _domain: &str, _years: u32, _contact: &RegistrantContact, _records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        Err(Error::Unsupported)
    }

    async fn renew_domain(&self, _domain: &str, _years: u32) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    async fn transfer_domain(&self, _domain: &str, _auth_code: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        // IONOS Domains API lists owned domains with expiry.
        let val = self.send(self.dom_get("/domains")).await?;
        let domains = val.as_array()
            .ok_or_else(|| Error::Provider("Expected array from /domains".to_string()))?;

        let entry = domains.iter()
            .find(|d| d["name"].as_str() == Some(domain))
            .ok_or_else(|| Error::Provider(format!("Domain '{}' not found in account", domain)))?;

        let expiry = entry.get("expirationDate")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let ns: Vec<String> = entry.get("nameServers")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(Domain {
            name: domain.to_string(),
            expiry_date: expiry,
            status: DomainStatus::Active,
            nameservers: ns,
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let zone_id = self.zone_id(domain).await?;
        let val = self.send(self.dns_get(&format!("/zones/{}", zone_id))).await?;
        let records: Vec<IonosRecord> = serde_json::from_value(
            val.get("records").cloned().unwrap_or(json!([]))
        ).map_err(Error::Parse)?;
        Ok(records.into_iter().map(map_record).collect())
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.zone_id(domain).await?;
        let body = json!([record_body(domain, &zone_id, record)]);
        let val = self.send(self.dns_post(&format!("/zones/{}/records", zone_id)).json(&body)).await?;
        // IONOS returns an array of created records
        let created: IonosRecord = val.as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| Error::Provider("Empty create response".to_string()))
            .and_then(|v| serde_json::from_value(v.clone()).map_err(Error::Parse))?;
        Ok(map_record(created))
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let zone_id = self.zone_id(domain).await?;
        let body = record_body(domain, &zone_id, record);
        self.send(self.dns_put(&format!("/zones/{}/records/{}", zone_id, record_id)).json(&body)).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let zone_id = self.zone_id(domain).await?;
        self.send(self.dns_delete(&format!("/zones/{}/records/{}", zone_id, record_id))).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        Ok(self.get_domain_info(domain).await?.nameservers)
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let body = json!({ "nameServers": nameservers });
        self.send(self.dom_patch(&format!("/domains/{}", domain)).json(&body)).await?;
        Ok(())
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn client(dns_base: &str, dom_base: &str) -> IonosClient {
        IonosClient::with_base_urls(
            "prefix".to_string(), "secret".to_string(),
            reqwest::Client::new(),
            dns_base.to_string(), dom_base.to_string(),
        )
    }

    #[tokio::test]
    async fn test_check_availability_unsupported() {
        let c = client("http://unused", "http://unused");
        assert!(matches!(c.check_availability("example.com").await, Err(Error::Unsupported)));
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":"zone123","name":"example.com"}]"#)
            .create_async().await;
        let _m2 = server.mock("GET", "/zones/zone123")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"zone123","name":"example.com","records":[
                {"id":"rec1","type":"A","name":"www","content":"1.2.3.4","ttl":300},
                {"id":"rec2","type":"MX","name":"@","content":"mail.example.com","ttl":3600,"prio":10}
            ]}"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].content, "1.2.3.4");
        assert_eq!(recs[0].id.as_deref(), Some("rec1"));
        assert_eq!(recs[1].priority, Some(10));
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones")
            .with_status(200)
            .with_body(r#"[{"id":"zone123","name":"example.com"}]"#)
            .create_async().await;
        let _m2 = server.mock("POST", "/zones/zone123/records")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":"rec42","type":"A","name":"www","content":"5.5.5.5","ttl":300}]"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        let rec = DnsRecord {
            id: None, record_type: DnsRecordType::A,
            name: "www".into(), content: "5.5.5.5".into(),
            ttl: Some(300), priority: None,
        };
        let created = c.create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.as_deref(), Some("rec42"));
    }

    #[tokio::test]
    async fn test_update_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones")
            .with_status(200)
            .with_body(r#"[{"id":"zone123","name":"example.com"}]"#)
            .create_async().await;
        let _m2 = server.mock("PUT", "/zones/zone123/records/rec1")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"rec1","type":"A","name":"www","content":"9.9.9.9","ttl":300}"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        let rec = DnsRecord {
            id: None, record_type: DnsRecordType::A,
            name: "www".into(), content: "9.9.9.9".into(),
            ttl: Some(300), priority: None,
        };
        let updated = c.update_dns_record("example.com", "rec1", &rec).await.unwrap();
        assert_eq!(updated.content, "9.9.9.9");
        assert_eq!(updated.id.as_deref(), Some("rec1"));
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", "/zones")
            .with_status(200)
            .with_body(r#"[{"id":"zone123","name":"example.com"}]"#)
            .create_async().await;
        let _m2 = server.mock("DELETE", "/zones/zone123/records/rec1")
            .with_status(200)
            .with_body(r#"{}"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        c.delete_dns_record("example.com", "rec1").await.unwrap();
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domains")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"name":"example.com","expirationDate":"2026-01-01T00:00:00Z","nameServers":["ns1.ionos.com","ns2.ionos.com"]}]"#)
            .create_async().await;

        let c = client("http://unused", &server.url());
        let d = c.get_domain_info("example.com").await.unwrap();
        assert_eq!(d.name, "example.com");
        assert!(d.expiry_date.is_some());
        assert_eq!(d.nameservers.len(), 2);
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PATCH", "/domains/example.com")
            .with_status(200)
            .with_body(r#"{}"#)
            .create_async().await;

        let c = client("http://unused", &server.url());
        c.set_nameservers("example.com", &["ns1.test.com".into()]).await.unwrap();
    }

    #[tokio::test]
    async fn test_zone_not_found() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/zones")
            .with_status(200)
            .with_body(r#"[]"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        let err = c.list_dns_records("notfound.com").await.unwrap_err();
        assert!(matches!(err, Error::Provider(_)));
    }

    #[tokio::test]
    async fn test_api_error() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/zones")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"message":"Unauthorized"}]"#)
            .create_async().await;

        let c = client(&server.url(), "http://unused");
        let err = c.list_dns_records("example.com").await.unwrap_err();
        match err {
            Error::Api { status, .. } => assert_eq!(status, 401),
            other => panic!("Expected Api error, got {:?}", other),
        }
    }
}
