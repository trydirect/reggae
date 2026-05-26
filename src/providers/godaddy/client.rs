use reqwest::Client;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, instrument};

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const PROD_URL: &str = "https://api.godaddy.com";
const SANDBOX_URL: &str = "https://api.ote-godaddy.com";

pub struct GodaddyClient {
    api_key: String,
    api_secret: String,
    base_url: String,
    http: Client,
}

impl GodaddyClient {
    pub fn new(api_key: String, api_secret: String, http: Client, sandbox: bool) -> Self {
        let base_url = if sandbox { SANDBOX_URL } else { PROD_URL }.to_string();
        Self { api_key, api_secret, base_url, http }
    }

    pub fn with_base_url(api_key: String, api_secret: String, http: Client, base_url: String) -> Self {
        Self { api_key, api_secret, base_url, http }
    }

    fn auth_header(&self) -> String {
        format!("sso-key {}:{}", self.api_key, self.api_secret)
    }

    async fn get_val(&self, path: &str) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("GET {}", url);
        let resp = self.http.get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/json")
            .send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        self.check_error(status.as_u16(), &val)?;
        Ok(val)
    }

    async fn post_val(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);
        let resp = self.http.post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        self.check_error(status.as_u16(), &val)?;
        Ok(val)
    }

    async fn put_val(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("PUT {}", url);
        let resp = self.http.put(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send().await?;
        let status = resp.status();
        // GoDaddy returns 204 for successful updates with no body
        if status.as_u16() == 204 {
            return Ok(Value::Null);
        }
        let val: Value = resp.json().await?;
        self.check_error(status.as_u16(), &val)?;
        Ok(val)
    }

    async fn patch_val(&self, path: &str, body: Value) -> Result<Value, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("PATCH {}", url);
        let resp = self.http.patch(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send().await?;
        let status = resp.status();
        if status.as_u16() == 204 {
            return Ok(Value::Null);
        }
        let val: Value = resp.json().await?;
        self.check_error(status.as_u16(), &val)?;
        Ok(val)
    }

    async fn delete_val(&self, path: &str) -> Result<(), Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("DELETE {}", url);
        let resp = self.http.delete(&url)
            .header("Authorization", self.auth_header())
            .send().await?;
        let status = resp.status();
        if status.is_success() {
            return Ok(());
        }
        let val: Value = resp.json().await?;
        self.check_error(status.as_u16(), &val)?;
        Ok(())
    }

    fn check_error(&self, status: u16, val: &Value) -> Result<(), Error> {
        if status >= 400 {
            let msg = val.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown GoDaddy error")
                .to_string();
            return Err(Error::Api { status, message: msg });
        }
        Ok(())
    }

    /// Build a GoDaddy contact object from a RegistrantContact.
    fn build_contact(c: &RegistrantContact) -> Value {
        json!({
            "nameFirst": c.first_name,
            "nameLast": c.last_name,
            "email": c.email,
            "phone": c.phone,
            "organization": c.organization.clone().unwrap_or_default(),
            "addressMailing": {
                "address1": c.address1,
                "address2": c.address2.clone().unwrap_or_default(),
                "city": c.city,
                "state": c.state,
                "country": c.country,
                "postalCode": c.postal_code,
            }
        })
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
        // GoDaddy uses "data" instead of "content"
        let content = r.get("data").or_else(|| r.get("content"))
            .and_then(|v| v.as_str())?.to_string();
        Some(DnsRecord {
            // GoDaddy does not return individual record IDs in list responses;
            // records are identified by type+name+data combination
            id: None,
            record_type,
            name: r.get("name")?.as_str()?.to_string(),
            content,
            ttl: r.get("ttl").and_then(|v| v.as_u64()).map(|t| t as u32),
            priority: r.get("priority").and_then(|v| v.as_u64()).map(|p| p as u16),
        })
    }
}

#[async_trait]
impl DomainRegistrar for GodaddyClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let val = self.get_val(&format!("/v1/domains/available?domain={}", domain)).await?;
        let available = val.get("available").and_then(|v| v.as_bool()).unwrap_or(false);
        let price = val.get("price").and_then(|v| v.as_f64())
            .map(|p| p / 1_000_000.0); // GoDaddy prices in micros
        let premium = val.get("definitive").and_then(|v| v.as_bool())
            .map(|d| !d) // non-definitive = premium
            .unwrap_or(false);
        Ok(Availability { available, premium, price })
    }

    /// Register a domain with GoDaddy.
    /// Requires a fully populated RegistrantContact.
    #[instrument(skip(self, contact, records))]
    async fn register_domain(
        &self,
        domain: &str,
        years: u32,
        contact: &RegistrantContact,
        records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error> {
        let contact_obj = Self::build_contact(contact);
        let body = json!({
            "domain": domain,
            "period": years,
            "renewAuto": true,
            "privacy": false,
            "consent": {
                "agreedAt": chrono::Utc::now().to_rfc3339(),
                "agreedBy": contact.ip_address(),
                "agreementKeys": ["DNRA"]
            },
            "contactAdmin": contact_obj,
            "contactBilling": contact_obj,
            "contactRegistrant": contact_obj,
            "contactTech": contact_obj,
        });

        let val = self.post_val("/v1/domains/purchase", body).await?;

        // Fetch domain info to return a full Domain struct
        self.get_domain_info(domain).await
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let body = json!({ "period": years });
        self.post_val(&format!("/v1/domains/{}/renew", domain), body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let body = json!({ "authCode": auth_code, "period": 1 });
        self.post_val(&format!("/v1/domains/{}/transfer", domain), body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let val = self.get_val(&format!("/v1/domains/{}", domain)).await?;

        let expiry = val.get("expires")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        let status = match val.get("status").and_then(|s| s.as_str()) {
            Some("ACTIVE") => DomainStatus::Active,
            Some("PENDING_SETUP") | Some("PENDING_REGISTER") => DomainStatus::Pending,
            Some("EXPIRED") | Some("REDEMPTION") => DomainStatus::Expired,
            Some("SUSPENDED") => DomainStatus::Suspended,
            _ => DomainStatus::Unknown,
        };

        let nameservers: Vec<String> = val.get("nameServers")
            .and_then(|ns| ns.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(Domain { name: domain.to_string(), expiry_date: expiry, status, nameservers })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let val = self.get_val(&format!("/v1/domains/{}/records", domain)).await?;
        Ok(val.as_array()
            .map(|arr| arr.iter().filter_map(Self::map_dns_record).collect())
            .unwrap_or_default())
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let body = json!([{
            "data": record.content,
            "name": record.name,
            "ttl": record.ttl.unwrap_or(3600),
            "type": record.record_type.to_string(),
            "priority": record.priority,
        }]);
        self.patch_val(&format!("/v1/domains/{}/records", domain), body).await?;
        // GoDaddy doesn't return the created record; return input with no ID
        Ok(record.clone())
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        // GoDaddy identifies records by type and name, not by a numeric ID.
        // record_id should be in format "TYPE/name" (e.g., "A/@")
        let body = json!([{
            "data": record.content,
            "ttl": record.ttl.unwrap_or(3600),
            "priority": record.priority,
        }]);
        let path = format!("/v1/domains/{}/records/{}/{}", domain, record.record_type, record.name);
        self.put_val(&path, body).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    /// Delete all DNS records matching type and name.
    /// record_id should be "TYPE/name" e.g. "A/@" or "TXT/mail"
    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let path = format!("/v1/domains/{}/records/{}", domain, record_id);
        self.delete_val(&path).await
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let info = self.get_domain_info(domain).await?;
        Ok(info.nameservers)
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let ns_list: Vec<Value> = nameservers.iter().map(|ns| json!(ns)).collect();
        let body = json!({ "nameServers": ns_list });
        self.patch_val(&format!("/v1/domains/{}", domain), body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_pricing(&self, tld: &str) -> Result<Pricing, Error> {
        // Fetch pricing for the given TLD via the agreements/tlds endpoint
        let val = self.get_val(&format!("/v1/domains/tlds")).await?;
        let tld_lower = tld.trim_start_matches('.');
        if let Some(arr) = val.as_array() {
            for entry in arr {
                let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name.eq_ignore_ascii_case(tld_lower) {
                    let price = entry.get("pricing")
                        .and_then(|p| p.get("registration"))
                        .and_then(|r| r.get("range"))
                        .and_then(|r| r.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|p| p.get("price"))
                        .and_then(|v| v.as_f64())
                        .map(|p| p / 1_000_000.0)
                        .unwrap_or(0.0);
                    return Ok(Pricing {
                        registration_price: price,
                        renewal_price: price,
                        transfer_price: price,
                        currency: "USD".to_string(),
                    });
                }
            }
        }
        Err(Error::Provider(format!("No pricing found for TLD '{}'", tld)))
    }
}

/// RegistrantContact helper — provides a default IP for GoDaddy consent.
trait ContactExt {
    fn ip_address(&self) -> &str;
}

impl ContactExt for RegistrantContact {
    fn ip_address(&self) -> &str {
        "0.0.0.0"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn make_client(base_url: &str) -> GodaddyClient {
        GodaddyClient::with_base_url(
            "key".to_string(), "secret".to_string(),
            reqwest::Client::new(), base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_check_availability() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/v1/domains/available?domain=example.com")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"available":true,"currency":"USD","definitive":true,"domain":"example.com","period":1,"price":9990000}"#)
            .create_async().await;

        let avail = make_client(&server.url()).check_availability("example.com").await.unwrap();
        assert!(avail.available);
        assert!((avail.price.unwrap() - 9.99).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/v1/domains/example.com/records")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"[{"type":"A","name":"@","data":"1.2.3.4","ttl":3600}]"#)
            .create_async().await;

        let records = make_client(&server.url()).list_dns_records("example.com").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/v1/domains/example.com")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"domain":"example.com","status":"ACTIVE","expires":"2025-12-31T00:00:00Z","nameServers":["ns1.godaddy.com","ns2.godaddy.com"]}"#)
            .create_async().await;

        let domain = make_client(&server.url()).get_domain_info("example.com").await.unwrap();
        assert_eq!(domain.status, DomainStatus::Active);
        assert_eq!(domain.nameservers.len(), 2);
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PATCH", "/v1/domains/example.com")
            .with_status(204)
            .create_async().await;

        make_client(&server.url())
            .set_nameservers("example.com", &["ns1.custom.com".to_string(), "ns2.custom.com".to_string()])
            .await.unwrap();
    }

    #[tokio::test]
    async fn test_register_domain() {
        let mut server = Server::new_async().await;
        let _mreg = server.mock("POST", "/v1/domains/purchase")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"orderId":12345,"itemCount":1,"total":999,"currency":"USD"}"#)
            .create_async().await;
        let _minfo = server.mock("GET", "/v1/domains/newdomain.com")
            .with_status(200).with_header("content-type", "application/json")
            .with_body(r#"{"domain":"newdomain.com","status":"PENDING_REGISTER","nameServers":[]}"#)
            .create_async().await;

        let contact = RegistrantContact {
            first_name: "Jane".to_string(),
            last_name: "Doe".to_string(),
            email: "jane@example.com".to_string(),
            phone: "+1.5555551234".to_string(),
            address1: "123 Main St".to_string(),
            city: "Springfield".to_string(),
            state: "IL".to_string(),
            country: "US".to_string(),
            postal_code: "62701".to_string(),
            ..Default::default()
        };
        let domain = make_client(&server.url())
            .register_domain("newdomain.com", 1, &contact, None)
            .await.unwrap();
        assert_eq!(domain.name, "newdomain.com");
        assert_eq!(domain.status, DomainStatus::Pending);
    }
}
