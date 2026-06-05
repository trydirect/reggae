use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::{debug, instrument};

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const PROD_BASE_URL: &str = "https://api.gandi.net/v5";
const SANDBOX_BASE_URL: &str = "https://api.sandbox.gandi.net/v5";

pub struct GandiClient {
    pat: String,
    base_url: String,
    http: Client,
}

impl GandiClient {
    pub fn new(pat: String, sandbox: bool, http: Client) -> Self {
        let base_url = if sandbox { SANDBOX_BASE_URL } else { PROD_BASE_URL }.to_string();
        Self::with_base_url(pat, http, base_url)
    }

    pub fn with_base_url(pat: String, http: Client, base_url: String) -> Self {
        Self { pat, base_url, http }
    }

    async fn get_json(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);
        let resp = self.http.get(&url).bearer_auth(&self.pat).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response {}: {}", status, text);
        if !status.is_success() {
            return Err(Error::Api { status: status.as_u16(), message: gandi_error(&text) });
        }
        serde_json::from_str(&text).map_err(Error::Parse)
    }

    async fn post_json(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);
        let resp = self.http.post(&url).bearer_auth(&self.pat).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response {}: {}", status, text);
        if !status.is_success() {
            return Err(Error::Api { status: status.as_u16(), message: gandi_error(&text) });
        }
        if text.is_empty() {
            Ok(json!({}))
        } else {
            serde_json::from_str(&text).map_err(Error::Parse)
        }
    }

    async fn put_json(&self, path: &str, body: Value) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("PUT {}", url);
        let resp = self.http.put(&url).bearer_auth(&self.pat).json(&body).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await?;
            return Err(Error::Api { status: status.as_u16(), message: gandi_error(&text) });
        }
        Ok(())
    }

    async fn delete_path(&self, path: &str) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);
        let resp = self.http.delete(&url).bearer_auth(&self.pat).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await?;
            return Err(Error::Api { status: status.as_u16(), message: gandi_error(&text) });
        }
        Ok(())
    }
}

fn gandi_error(text: &str) -> String {
    if let Ok(val) = serde_json::from_str::<Value>(text) {
        if let Some(s) = val.get("message").and_then(|v| v.as_str()) {
            return s.to_string();
        }
        if let Some(s) = val.get("cause").and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    text.to_string()
}

// Gandi LiveDNS operates on RRsets (one entry per name+type, potentially multi-value).
// We encode the record ID as "rrset_name/rrset_type" so the update/delete calls can
// reconstruct the path without requiring a separate lookup.
fn parse_record_id(record_id: &str) -> Option<(String, String)> {
    // Strip "[N]" suffix that may appear on expanded multi-value rrsets.
    let id = record_id.find('[').map_or(record_id, |i| &record_id[..i]);
    let slash = id.rfind('/')?;
    Some((id[..slash].to_string(), id[slash + 1..].to_string()))
}

fn rrset_record_id(name: &str, rtype: &DnsRecordType) -> String {
    format!("{}/{}", name, rtype)
}

// MX values in Gandi wire format: "10 mail.example.com."
fn mx_to_content(value: &str) -> (String, Option<u16>) {
    let mut parts = value.splitn(2, ' ');
    let prio = parts.next().and_then(|s| s.parse().ok());
    let host = parts.next().unwrap_or(value).trim_end_matches('.').to_string();
    (host, prio)
}

fn content_to_mx(content: &str, priority: Option<u16>) -> String {
    format!("{} {}.", priority.unwrap_or(10), content.trim_end_matches('.'))
}

fn rrset_value(record: &DnsRecord) -> String {
    if record.record_type == DnsRecordType::Mx {
        content_to_mx(&record.content, record.priority)
    } else {
        record.content.clone()
    }
}

fn parse_dns_type(s: &str) -> DnsRecordType {
    match s {
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
    }
}

#[derive(Debug, Deserialize)]
struct GandiRrset {
    rrset_name: String,
    rrset_type: String,
    rrset_ttl: Option<u32>,
    rrset_values: Vec<String>,
}

fn expand_rrset(rrset: GandiRrset) -> Vec<DnsRecord> {
    let rtype = parse_dns_type(&rrset.rrset_type);
    let base_id = rrset_record_id(&rrset.rrset_name, &rtype);
    rrset.rrset_values.into_iter().enumerate().map(|(i, val)| {
        let (content, priority) = if rtype == DnsRecordType::Mx {
            mx_to_content(&val)
        } else {
            (val, None)
        };
        DnsRecord {
            id: Some(if i == 0 { base_id.clone() } else { format!("{}[{}]", base_id, i) }),
            record_type: rtype.clone(),
            name: rrset.rrset_name.clone(),
            content,
            ttl: rrset.rrset_ttl,
            priority,
        }
    }).collect()
}

#[derive(Debug, Deserialize)]
struct GandiDomainInfo {
    fqdn: String,
    dates: Option<GandiDates>,
    status: Option<Vec<String>>,
    nameservers: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GandiDates {
    registry_ends_at: Option<String>,
}

fn map_domain_status(statuses: &[String]) -> DomainStatus {
    if statuses.iter().any(|s| s.contains("expired")) {
        DomainStatus::Expired
    } else if statuses.iter().any(|s| {
        s == "clientTransferProhibited" || s == "serverTransferProhibited" || s == "active"
    }) {
        DomainStatus::Active
    } else if statuses.is_empty() {
        DomainStatus::Unknown
    } else {
        DomainStatus::Active
    }
}

#[async_trait]
impl DomainRegistrar for GandiClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        // Use reqwest query builder so the `name[]` key is properly percent-encoded.
        let url = format!("{}/domain/available", self.base_url);
        debug!("GET {} ?name[]={}", url, domain);
        let resp = self.http.get(&url)
            .bearer_auth(&self.pat)
            .query(&[("name[]", domain)])
            .send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response {}: {}", status, text);
        if !status.is_success() {
            return Err(Error::Api { status: status.as_u16(), message: gandi_error(&text) });
        }
        let val: Value = serde_json::from_str(&text)?;
        let avail_status = val.get(domain).and_then(|v| v.as_str()).unwrap_or("unavailable");
        // "available" = registrable now; "available-at" = registrable at a future date
        let available = avail_status == "available" || avail_status.starts_with("available-at");
        Ok(Availability { available, premium: false, price: None })
    }

    #[instrument(skip(self, contact, records))]
    async fn register_domain(
        &self,
        domain: &str,
        years: u32,
        contact: &RegistrantContact,
        records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error> {
        let owner = json!({
            "given":      contact.first_name,
            "family":     contact.last_name,
            "email":      contact.email,
            "phone":      contact.phone,
            "streetaddr": contact.address1,
            "city":       contact.city,
            "state":      contact.state,
            "zip":        contact.postal_code,
            "country":    contact.country,
            "type":       0,
        });
        let body = json!({
            "fqdn":     domain,
            "duration": years,
            "owner":    owner,
            "admin":    owner,
            "tech":     owner,
            "bill":     owner,
        });
        self.post_json("/domain/domains", body).await?;

        if let Some(dns_records) = records {
            for rec in dns_records {
                self.create_dns_record(domain, &rec).await?;
            }
        }

        // Registration is asynchronous at Gandi; return a pending placeholder if the
        // domain is not yet visible via the info endpoint.
        match self.get_domain_info(domain).await {
            Ok(d) => Ok(d),
            Err(_) => Ok(Domain {
                name: domain.to_string(),
                expiry_date: None,
                status: DomainStatus::Pending,
                nameservers: vec![],
            }),
        }
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let path = format!("/domain/domains/{}/renew", domain);
        self.post_json(&path, json!({ "duration": years })).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let body = json!({ "fqdn": domain, "authinfo": auth_code });
        self.post_json("/domain/transfers", body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let path = format!("/domain/domains/{}", domain);
        let val = self.get_json(&path).await?;
        let info: GandiDomainInfo = serde_json::from_value(val).map_err(Error::Parse)?;

        let expiry = info.dates.as_ref()
            .and_then(|d| d.registry_ends_at.as_deref())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let status = info.status.as_deref()
            .map(map_domain_status)
            .unwrap_or(DomainStatus::Unknown);

        Ok(Domain {
            name: info.fqdn,
            expiry_date: expiry,
            status,
            nameservers: info.nameservers.unwrap_or_default(),
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let path = format!("/livedns/domains/{}/records", domain);
        let val = self.get_json(&path).await?;
        let rrsets: Vec<GandiRrset> = serde_json::from_value(val).map_err(Error::Parse)?;
        Ok(rrsets.into_iter().flat_map(expand_rrset).collect())
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let body = json!({
            "rrset_name":   record.name,
            "rrset_type":   record.record_type.to_string(),
            "rrset_ttl":    record.ttl.unwrap_or(300),
            "rrset_values": [rrset_value(record)],
        });
        let path = format!("/livedns/domains/{}/records", domain);
        self.post_json(&path, body).await?;
        let mut created = record.clone();
        created.id = Some(rrset_record_id(&record.name, &record.record_type));
        Ok(created)
    }

    // Replaces the entire rrset identified by `record_id` ("rrset_name/rrset_type")
    // with a single-value rrset containing `record`.
    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let (rrset_name, rrset_type) = parse_record_id(record_id)
            .ok_or_else(|| Error::Provider(format!("invalid record_id '{}'; expected 'name/TYPE'", record_id)))?;
        let body = json!({
            "rrset_ttl":    record.ttl.unwrap_or(300),
            "rrset_values": [rrset_value(record)],
        });
        let path = format!("/livedns/domains/{}/records/{}/{}", domain, rrset_name, rrset_type);
        self.put_json(&path, body).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let (rrset_name, rrset_type) = parse_record_id(record_id)
            .ok_or_else(|| Error::Provider(format!("invalid record_id '{}'; expected 'name/TYPE'", record_id)))?;
        let path = format!("/livedns/domains/{}/records/{}/{}", domain, rrset_name, rrset_type);
        self.delete_path(&path).await
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        Ok(self.get_domain_info(domain).await?.nameservers)
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let path = format!("/domain/domains/{}/nameservers", domain);
        self.put_json(&path, json!({ "nameservers": nameservers })).await
    }

    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn client(base_url: &str) -> GandiClient {
        GandiClient::with_base_url("test_pat".to_string(), reqwest::Client::new(), base_url.to_string())
    }

    #[test]
    fn test_parse_record_id_simple() {
        let (name, rtype) = parse_record_id("www/A").unwrap();
        assert_eq!(name, "www");
        assert_eq!(rtype, "A");
    }

    #[test]
    fn test_parse_record_id_with_index() {
        let (name, rtype) = parse_record_id("www/A[1]").unwrap();
        assert_eq!(name, "www");
        assert_eq!(rtype, "A");
    }

    #[test]
    fn test_parse_record_id_at() {
        let (name, rtype) = parse_record_id("@/MX").unwrap();
        assert_eq!(name, "@");
        assert_eq!(rtype, "MX");
    }

    #[test]
    fn test_parse_record_id_invalid() {
        assert!(parse_record_id("noSlash").is_none());
    }

    #[test]
    fn test_mx_roundtrip() {
        let wire = "10 mail.example.com.";
        let (content, prio) = mx_to_content(wire);
        assert_eq!(content, "mail.example.com");
        assert_eq!(prio, Some(10));
        let back = content_to_mx(&content, prio);
        assert_eq!(back, wire);
    }

    #[test]
    fn test_expand_rrset_single() {
        let rrset = GandiRrset {
            rrset_name: "www".to_string(),
            rrset_type: "A".to_string(),
            rrset_ttl: Some(300),
            rrset_values: vec!["1.2.3.4".to_string()],
        };
        let records = expand_rrset(rrset);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "1.2.3.4");
        assert_eq!(records[0].id.as_deref(), Some("www/A"));
    }

    #[test]
    fn test_expand_rrset_multi() {
        let rrset = GandiRrset {
            rrset_name: "www".to_string(),
            rrset_type: "A".to_string(),
            rrset_ttl: Some(300),
            rrset_values: vec!["1.2.3.4".to_string(), "5.6.7.8".to_string()],
        };
        let records = expand_rrset(rrset);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id.as_deref(), Some("www/A"));
        assert_eq!(records[1].id.as_deref(), Some("www/A[1]"));
    }

    #[test]
    fn test_expand_rrset_mx() {
        let rrset = GandiRrset {
            rrset_name: "@".to_string(),
            rrset_type: "MX".to_string(),
            rrset_ttl: Some(3600),
            rrset_values: vec!["10 mail.example.com.".to_string()],
        };
        let records = expand_rrset(rrset);
        assert_eq!(records[0].content, "mail.example.com");
        assert_eq!(records[0].priority, Some(10));
    }

    #[tokio::test]
    async fn test_check_availability_available() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domain/available")
            .match_query(mockito::Matcher::UrlEncoded("name[]".into(), "example.com".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"example.com":"available"}"#)
            .create_async().await;

        let c = client(&server.url());
        let avail = c.check_availability("example.com").await.unwrap();
        assert!(avail.available);
    }

    #[tokio::test]
    async fn test_check_availability_unavailable() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domain/available")
            .match_query(mockito::Matcher::UrlEncoded("name[]".into(), "taken.com".into()))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"taken.com":"not available"}"#)
            .create_async().await;

        let c = client(&server.url());
        let avail = c.check_availability("taken.com").await.unwrap();
        assert!(!avail.available);
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/domain/domains/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{
                "fqdn": "example.com",
                "dates": {"registry_ends_at": "2026-01-01T00:00:00Z"},
                "status": ["clientTransferProhibited"],
                "nameservers": ["ns1.gandi.net", "ns2.gandi.net"]
            }"#)
            .create_async().await;

        let c = client(&server.url());
        let domain = c.get_domain_info("example.com").await.unwrap();
        assert_eq!(domain.name, "example.com");
        assert!(domain.expiry_date.is_some());
        assert_eq!(domain.status, DomainStatus::Active);
        assert_eq!(domain.nameservers.len(), 2);
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/livedns/domains/example.com/records")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[
                {"rrset_name":"www","rrset_type":"A","rrset_ttl":300,"rrset_values":["1.2.3.4"]},
                {"rrset_name":"@","rrset_type":"MX","rrset_ttl":3600,"rrset_values":["10 mail.example.com."]}
            ]"#)
            .create_async().await;

        let c = client(&server.url());
        let records = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "1.2.3.4");
        assert_eq!(records[0].id.as_deref(), Some("www/A"));
        assert_eq!(records[1].content, "mail.example.com");
        assert_eq!(records[1].priority, Some(10));
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/livedns/domains/example.com/records")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"DNS Record Created"}"#)
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
        assert_eq!(created.id.as_deref(), Some("www/A"));
        assert_eq!(created.content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_update_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PUT", "/livedns/domains/example.com/records/www/A")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"DNS Record Updated"}"#)
            .create_async().await;

        let c = client(&server.url());
        let rec = DnsRecord {
            id: None,
            record_type: DnsRecordType::A,
            name: "www".to_string(),
            content: "5.5.5.5".to_string(),
            ttl: Some(300),
            priority: None,
        };
        let updated = c.update_dns_record("example.com", "www/A", &rec).await.unwrap();
        assert_eq!(updated.content, "5.5.5.5");
        assert_eq!(updated.id.as_deref(), Some("www/A"));
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("DELETE", "/livedns/domains/example.com/records/www/A")
            .with_status(204)
            .create_async().await;

        let c = client(&server.url());
        c.delete_dns_record("example.com", "www/A").await.unwrap();
    }

    #[tokio::test]
    async fn test_delete_dns_record_with_index() {
        let mut server = Server::new_async().await;
        // Index suffix is stripped; both [0] and [1] records resolve to the same rrset.
        let _m = server.mock("DELETE", "/livedns/domains/example.com/records/www/A")
            .with_status(204)
            .create_async().await;

        let c = client(&server.url());
        c.delete_dns_record("example.com", "www/A[1]").await.unwrap();
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PUT", "/domain/domains/example.com/nameservers")
            .with_status(201)
            .with_body(r#"{"message":"Nameservers Updated"}"#)
            .create_async().await;

        let c = client(&server.url());
        let ns = vec!["ns1.example.com".to_string(), "ns2.example.com".to_string()];
        c.set_nameservers("example.com", &ns).await.unwrap();
    }

    #[tokio::test]
    async fn test_api_error_response() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/livedns/domains/notfound.com/records")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"Domain not found","cause":"Domain does not exist"}"#)
            .create_async().await;

        let c = client(&server.url());
        let err = c.list_dns_records("notfound.com").await.unwrap_err();
        match err {
            Error::Api { status, message } => {
                assert_eq!(status, 404);
                assert!(message.contains("Domain not found"));
            }
            other => panic!("Expected Api error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_renew_domain() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domain/domains/example.com/renew")
            .with_status(202)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message":"Domain renewal in progress."}"#)
            .create_async().await;

        let c = client(&server.url());
        c.renew_domain("example.com", 1).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_pricing_unsupported() {
        let c = client("http://unused");
        let err = c.get_pricing("com").await.unwrap_err();
        assert!(matches!(err, Error::Unsupported));
    }
}
