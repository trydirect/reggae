use reqwest::{Client, StatusCode};
use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::instrument;
use chrono::{DateTime, Utc};

use crate::core::{error::Error, traits::DomainRegistrar, types::{*, RegistrantContact}};


const BASE_URL: &str = "https://api.godaddy.com";
const OTE_URL: &str  = "https://api.ote-godaddy.com";

pub struct GodaddyClient {
    api_key:    String,
    api_secret: String,
    consent_ip: String,
    contact:    RegistrantContact,
    base_url:   String,
    http:       Client,
}

impl GodaddyClient {
    pub fn new(
        api_key: String,
        api_secret: String,
        consent_ip: String,
        contact: RegistrantContact,
        sandbox: bool,
        http: Client,
    ) -> Self {
        let base_url = if sandbox { OTE_URL } else { BASE_URL }.to_string();
        Self::with_base_url(api_key, api_secret, consent_ip, contact, http, base_url)
    }

    pub fn with_base_url(
        api_key: String,
        api_secret: String,
        consent_ip: String,
        contact: RegistrantContact,
        http: Client,
        base_url: String,
    ) -> Self {
        Self { api_key, api_secret, consent_ip, contact, base_url, http }
    }

    fn auth(&self) -> String {
        format!("sso-key {}:{}", self.api_key, self.api_secret)
    }

    fn make_contact_body(c: &RegistrantContact) -> Value {
        json!({
            "nameFirst":    c.first_name,
            "nameLast":     c.last_name,
            "email":        c.email,
            "phone":        c.phone,
            "addressMailing": {
                "address1":    c.address1,
                "city":        c.city,
                "state":       c.state,
                "postalCode":  c.postal_code,
                "country":     c.country,
            }
        })
    }

    /// GoDaddy returns record-id-less records; encode type+name as stable id.
    fn record_id(r: &Value) -> Option<String> {
        let t = r["type"].as_str()?;
        let n = r["name"].as_str()?;
        Some(format!("{}:{}", t, n))
    }

    fn map_record(r: &Value) -> Option<DnsRecord> {
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
            id:          Self::record_id(r),
            record_type,
            name:        r["name"].as_str()?.to_string(),
            content:     r["data"].as_str()?.to_string(),
            ttl:         r["ttl"].as_u64().map(|t| t as u32),
            priority:    r["priority"].as_u64().map(|p| p as u16),
        })
    }

    /// Parse `type:name` record id
    fn parse_record_id(id: &str) -> Result<(&str, &str), Error> {
        let mut parts = id.splitn(2, ':');
        let rtype = parts.next().ok_or_else(|| Error::Provider("Invalid record id".into()))?;
        let rname = parts.next().ok_or_else(|| Error::Provider("Invalid record id — expected type:name".into()))?;
        Ok((rtype, rname))
    }
}

#[async_trait]
impl DomainRegistrar for GodaddyClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let url = format!("{}/v1/domains/available?domain={}&checkType=FAST", self.base_url, domain);
        let resp = self.http.get(&url).header("Authorization", self.auth()).send().await?;
        let val: Value = resp.json().await?;
        let available = val["available"].as_bool().unwrap_or(false);
        let premium   = val["definitive"].as_bool().map(|d| !d).unwrap_or(false);
        // GoDaddy price is in micro-units
        let price = val["price"].as_f64().map(|p| p / 1_000_000.0);
        Ok(Availability { available, premium, price })
    }

    #[instrument(skip(self, records))]
    async fn register_domain(&self, domain: &str, _years: u32, contact: &RegistrantContact, records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        let agreed_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let contact = Self::make_contact_body(contact);

        let body = json!({
            "domain":            domain,
            "period":            1,
            "renewAuto":         true,
            "privacy":           false,
            "consent": {
                "agreedAt":        agreed_at,
                "agreedBy":        self.consent_ip,
                "agreementKeys": ["DNRA"],
            },
            "contactAdmin":      contact,
            "contactBilling":    contact,
            "contactRegistrant": contact,
            "contactTech":       contact,
        });

        let resp = self.http.post(&format!("{}/v1/domains/purchase", self.base_url))
            .header("Authorization", self.auth())
            .json(&body)
            .send().await?;

        let status = resp.status();
        let val: Value = resp.json().await?;

        if !status.is_success() {
            let msg = val["message"].as_str()
                .or_else(|| val["fields"].as_array()
                    .and_then(|f| f.first())
                    .and_then(|f| f["message"].as_str()))
                .unwrap_or("registration failed")
                .to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }

        // Add initial DNS records if provided
        if let Some(recs) = records {
            for rec in &recs {
                let _ = self.create_dns_record(domain, rec).await;
            }
        }

        self.get_domain_info(domain).await
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let url = format!("{}/v1/domains/{}/renew", self.base_url, domain);
        let resp = self.http.post(&url)
            .header("Authorization", self.auth())
            .json(&json!({ "period": years }))
            .send().await?;
        let status = resp.status();
        if !status.is_success() {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("renew failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let contact = Self::make_contact_body(&self.contact);
        let body = json!({
            "authCode":          auth_code,
            "period":            1,
            "renewAuto":         true,
            "privacy":           false,
            "consent": {
                "agreedAt":        Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                "agreedBy":        self.consent_ip,
                "agreementKeys":   ["DNRA", "DNTA"],
            },
            "contactAdmin":      contact,
            "contactBilling":    contact,
            "contactRegistrant": contact,
            "contactTech":       contact,
        });
        let resp = self.http.post(&format!("{}/v1/domains/{}/transfer", self.base_url, domain))
            .header("Authorization", self.auth())
            .json(&body)
            .send().await?;
        let status = resp.status();
        if !status.is_success() {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("transfer failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let url = format!("{}/v1/domains/{}", self.base_url, domain);
        let resp = self.http.get(&url).header("Authorization", self.auth()).send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            let msg = val["message"].as_str().unwrap_or("not found").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        let expiry = val["expires"].as_str()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));
        let ns: Vec<String> = val["nameServers"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let status_str = val["status"].as_str().unwrap_or("UNKNOWN");
        let domain_status = if status_str.contains("ACTIVE") {
            DomainStatus::Active
        } else if status_str.contains("EXPIRED") {
            DomainStatus::Expired
        } else {
            DomainStatus::Unknown
        };
        Ok(Domain {
            name: domain.to_string(),
            expiry_date: expiry,
            status: domain_status,
            nameservers: ns,
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let url = format!("{}/v1/domains/{}/records", self.base_url, domain);
        let resp = self.http.get(&url).header("Authorization", self.auth()).send().await?;
        let status = resp.status();
        let val: Value = resp.json().await?;
        if !status.is_success() {
            let msg = val["message"].as_str().unwrap_or("list failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        let records = val.as_array()
            .map(|a| a.iter().filter_map(Self::map_record).collect())
            .unwrap_or_default();
        Ok(records)
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let url = format!("{}/v1/domains/{}/records", self.base_url, domain);
        let body = json!([{
            "type":     record.record_type.to_string(),
            "name":     record.name,
            "data":     record.content,
            "ttl":      record.ttl.unwrap_or(600),
            "priority": record.priority.unwrap_or(0),
        }]);
        let resp = self.http.patch(&url)
            .header("Authorization", self.auth())
            .json(&body)
            .send().await?;
        let status = resp.status();
        if !status.is_success() {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("create failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        let mut created = record.clone();
        created.id = Some(format!("{}:{}", record.record_type, record.name));
        Ok(created)
    }

    /// Update all records of the same type:name (GoDaddy replaces by type+name).
    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let (rtype, rname) = Self::parse_record_id(record_id)?;
        let url = format!("{}/v1/domains/{}/records/{}/{}", self.base_url, domain, rtype, rname);
        let body = json!([{
            "data":     record.content,
            "ttl":      record.ttl.unwrap_or(600),
            "priority": record.priority.unwrap_or(0),
        }]);
        let resp = self.http.put(&url)
            .header("Authorization", self.auth())
            .json(&body)
            .send().await?;
        let status = resp.status();
        if !status.is_success() {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("update failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    /// Delete all records matching type:name.
    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let (rtype, rname) = Self::parse_record_id(record_id)?;
        let url = format!("{}/v1/domains/{}/records/{}/{}", self.base_url, domain, rtype, rname);
        let resp = self.http.delete(&url).header("Authorization", self.auth()).send().await?;
        let status = resp.status();
        if !status.is_success() && status != StatusCode::NO_CONTENT {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("delete failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        Ok(())
    }

    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let info = self.get_domain_info(domain).await?;
        Ok(info.nameservers)
    }

    #[instrument(skip(self, nameservers))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let url = format!("{}/v1/domains/{}", self.base_url, domain);
        let body = json!({ "nameServers": nameservers });
        let resp = self.http.patch(&url)
            .header("Authorization", self.auth())
            .json(&body)
            .send().await?;
        let status = resp.status();
        if !status.is_success() {
            let val: Value = resp.json().await?;
            let msg = val["message"].as_str().unwrap_or("set NS failed").to_string();
            return Err(Error::Api { status: status.as_u16(), message: msg });
        }
        Ok(())
    }

    async fn get_pricing(&self, tld: &str) -> Result<Pricing, Error> {
        let url = format!("{}/v1/domains/tlds", self.base_url);
        let resp = self.http.get(&url).header("Authorization", self.auth()).send().await?;
        let val: Value = resp.json().await?;
        let tlds = val.as_array().ok_or_else(|| Error::Provider("Unexpected pricing response".into()))?;
        let entry = tlds.iter().find(|t| t["name"].as_str().map(|s| s.eq_ignore_ascii_case(tld)).unwrap_or(false))
            .ok_or_else(|| Error::Provider(format!("TLD '{}' not found", tld)))?;
        let register_price = entry["pricing"]["registration"].as_f64()
            .or_else(|| entry["pricing"]["newRegistration"].as_f64())
            .unwrap_or(0.0);
        let renew_price = entry["pricing"]["renewal"].as_f64().unwrap_or(0.0);
        Ok(Pricing {
            registration_price: register_price,
            renewal_price: renew_price,
            transfer_price: 0.0,
            currency: "USD".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    

    fn client(base_url: &str) -> GodaddyClient {
        GodaddyClient::with_base_url(
            "key".into(), "secret".into(),
            "1.2.3.4".into(),
            RegistrantContact {
                first_name: "Jane".into(), last_name: "Doe".into(),
                email: "jane@example.com".into(), phone: "+1.5555555555".into(),
                address1: "123 Main St".into(), address2: None,
                city: "Phoenix".into(), state: "AZ".into(),
                postal_code: "85001".into(), country: "US".into(),
                organization: None,
            },
            reqwest::Client::new(),
            base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_check_availability() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/v1/domains/available?domain=test.com&checkType=FAST")
            .with_body(r#"{"available":true,"currency":"USD","definitive":true,"domain":"test.com","price":11990000}"#)
            .create_async().await;
        let c = client(&server.url());
        let av = c.check_availability("test.com").await.unwrap();
        assert!(av.available);
        assert!((av.price.unwrap() - 11.99).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/v1/domains/example.com/records")
            .with_body(r#"[{"type":"A","name":"@","data":"1.1.1.1","ttl":600}]"#)
            .create_async().await;
        let c = client(&server.url());
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].content, "1.1.1.1");
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PATCH", "/v1/domains/example.com/records")
            .with_status(200)
            .with_body("{}")
            .create_async().await;
        let c = client(&server.url());
        let rec = DnsRecord { id: None, record_type: DnsRecordType::A, name: "@".into(), content: "9.9.9.9".into(), ttl: Some(600), priority: None };
        let created = c.create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.unwrap(), "A:@");
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("DELETE", "/v1/domains/example.com/records/A/@")
            .with_status(204)
            .create_async().await;
        let c = client(&server.url());
        c.delete_dns_record("example.com", "A:@").await.unwrap();
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("PATCH", "/v1/domains/example.com")
            .with_status(200)
            .with_body("{}")
            .create_async().await;
        let c = client(&server.url());
        c.set_nameservers("example.com", &["ns1.example.com".to_string(), "ns2.example.com".to_string()]).await.unwrap();
    }

    #[tokio::test]
    async fn test_register_domain() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("POST", "/v1/domains/purchase")
            .with_status(200)
            .with_body(r#"{"orderId":12345,"currency":"USD","total":11990000}"#)
            .create_async().await;
        let _m2 = server.mock("GET", "/v1/domains/newdomain.com")
            .with_body(r#"{"domain":"newdomain.com","status":"ACTIVE","nameServers":["ns1.godaddy.com","ns2.godaddy.com"]}"#)
            .create_async().await;
        let c = client(&server.url());
        let domain = c.register_domain("newdomain.com", 1, &RegistrantContact::default(), None).await.unwrap();
        assert_eq!(domain.status, DomainStatus::Active);
    }
}
