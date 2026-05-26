use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{debug, instrument};
use async_trait::async_trait;

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const BASE_URL: &str = "https://porkbun.com/api/json/v3";

pub struct PorkbunClient {
    api_key: String,
    secret_api_key: String,
    base_url: String,
    http: Client,
}

impl PorkbunClient {
    pub fn new(api_key: String, secret_api_key: String, http: Client) -> Self {
        Self::with_base_url(api_key, secret_api_key, http, BASE_URL.to_string())
    }

    pub fn with_base_url(api_key: String, secret_api_key: String, http: Client, base_url: String) -> Self {
        Self { api_key, secret_api_key, base_url, http }
    }

    fn auth_body(&self) -> Value {
        json!({
            "apikey": self.api_key,
            "secretapikey": self.secret_api_key
        })
    }

    async fn post<T: for<'de> Deserialize<'de>>(&self, path: &str, body: Value) -> Result<T, Error> {
        let url = format!("{}{}", self.base_url, path);
        debug!("POST {}", url);
        let resp = self.http.post(&url).json(&body).send().await?;
        let status = resp.status();
        let text = resp.text().await?;
        debug!("Response: {}", text);

        if !status.is_success() {
            return Err(Error::Api { status: status.as_u16(), message: text });
        }

        let val: Value = serde_json::from_str(&text)?;
        if val.get("status").and_then(|s| s.as_str()) == Some("ERROR") {
            let msg = val.get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error")
                .to_string();
            return Err(Error::Api { status: 400, message: msg });
        }

        serde_json::from_value(val).map_err(Error::Parse)
    }
}

#[derive(Debug, Deserialize)]
struct PorkbunDnsRecord {
    id: String,
    name: String,
    #[serde(rename = "type")]
    record_type: String,
    content: String,
    ttl: Option<String>,
    prio: Option<String>,
}

fn map_dns_record(r: PorkbunDnsRecord) -> DnsRecord {
    let record_type = match r.record_type.as_str() {
        "A" => DnsRecordType::A,
        "AAAA" => DnsRecordType::Aaaa,
        "CNAME" => DnsRecordType::Cname,
        "MX" => DnsRecordType::Mx,
        "TXT" => DnsRecordType::Txt,
        "NS" => DnsRecordType::Ns,
        "SRV" => DnsRecordType::Srv,
        "CAA" => DnsRecordType::Caa,
        "ALIAS" => DnsRecordType::Alias,
        _ => DnsRecordType::A,
    };
    DnsRecord {
        id: Some(r.id),
        record_type,
        name: r.name,
        content: r.content,
        ttl: r.ttl.as_deref().and_then(|t| t.parse().ok()),
        priority: r.prio.as_deref().and_then(|p| p.parse().ok()),
    }
}

#[derive(Debug, Deserialize)]
struct DnsListResponse {
    status: String,
    records: Option<Vec<PorkbunDnsRecord>>,
}

#[derive(Debug, Deserialize)]
struct DnsCreateResponse {
    status: String,
    id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PricingResponse {
    status: String,
    pricing: Option<std::collections::HashMap<String, TldPricing>>,
}

#[derive(Debug, Deserialize)]
struct TldPricing {
    registration: Option<String>,
    renewal: Option<String>,
    transfer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NsResponse {
    status: String,
    ns: Option<Vec<String>>,
}

#[async_trait]
impl DomainRegistrar for PorkbunClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let mut body = self.auth_body();
        body["domain"] = json!(domain);

        let val: Value = self.post("/domain/checkAvailability", body).await?;
        let avail = val.get("avail")
            .and_then(|v| v.as_str())
            .map(|s| s.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let price = val.get("price")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok());

        Ok(Availability { available: avail, premium: false, price })
    }

    #[instrument(skip(self, records))]
    async fn register_domain(&self, domain: &str, records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        let mut body = self.auth_body();
        body["domain"] = json!(domain);
        body["years"] = json!(1);

        let val: Value = self.post("/domain/register", body).await?;
        let expiry = val.get("expireDate")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc));

        // Optionally create initial DNS records
        if let Some(dns_records) = records {
            for rec in dns_records {
                self.create_dns_record(domain, &rec).await?;
            }
        }

        Ok(Domain {
            name: domain.to_string(),
            expiry_date: expiry,
            status: DomainStatus::Active,
            nameservers: vec![],
        })
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let mut body = self.auth_body();
        body["domain"] = json!(domain);
        body["years"] = json!(years);
        let _: Value = self.post("/domain/renew", body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let mut body = self.auth_body();
        body["domain"] = json!(domain);
        body["authCode"] = json!(auth_code);
        let _: Value = self.post("/domain/transfer", body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let mut body = self.auth_body();
        body["domain"] = json!(domain);
        let val: Value = self.post("/domain/getInfo", body).await?;
        let info = val.get("domain").unwrap_or(&val);

        let expiry = info.get("expire_date")
            .or_else(|| info.get("expireDate"))
            .and_then(|v| v.as_str())
            .and_then(|s| {
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            });

        let nameservers: Vec<String> = info.get("ns")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(Domain {
            name: domain.to_string(),
            expiry_date: expiry,
            status: DomainStatus::Active,
            nameservers,
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let body = self.auth_body();
        let resp: DnsListResponse = self.post(&format!("/dns/retrieve/{}", domain), body).await?;
        Ok(resp.records.unwrap_or_default().into_iter().map(map_dns_record).collect())
    }

    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut body = self.auth_body();
        body["name"] = json!(record.name);
        body["type"] = json!(record.record_type.to_string());
        body["content"] = json!(record.content);
        body["ttl"] = json!(record.ttl.unwrap_or(300).to_string());
        if let Some(prio) = record.priority {
            body["prio"] = json!(prio.to_string());
        }

        let resp: DnsCreateResponse = self.post(&format!("/dns/create/{}", domain), body).await?;
        let mut created = record.clone();
        created.id = resp.id.map(|id| id.to_string());
        Ok(created)
    }

    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut body = self.auth_body();
        body["name"] = json!(record.name);
        body["type"] = json!(record.record_type.to_string());
        body["content"] = json!(record.content);
        body["ttl"] = json!(record.ttl.unwrap_or(300).to_string());
        if let Some(prio) = record.priority {
            body["prio"] = json!(prio.to_string());
        }

        let _: Value = self.post(&format!("/dns/edit/{}/{}", domain, record_id), body).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let body = self.auth_body();
        let _: Value = self.post(&format!("/dns/delete/{}/{}", domain, record_id), body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let body = self.auth_body();
        let resp: NsResponse = self.post(&format!("/domain/getNs/{}", domain), body).await?;
        Ok(resp.ns.unwrap_or_default())
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let mut body = self.auth_body();
        body["ns"] = json!(nameservers);
        let _: Value = self.post(&format!("/domain/updateNs/{}", domain), body).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_pricing(&self, tld: &str) -> Result<Pricing, Error> {
        let url = format!("{}/pricing/get", self.base_url);
        debug!("GET {}", url);
        let resp = self.http.get(&url).send().await?;
        let val: Value = resp.json().await?;

        let pricing = val.get("pricing")
            .and_then(|p| p.get(tld))
            .ok_or_else(|| Error::Provider(format!("No pricing for TLD '{}'", tld)))?;

        let reg = pricing.get("registration")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let ren = pricing.get("renewal")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let tra = pricing.get("transfer")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        Ok(Pricing {
            registration_price: reg,
            renewal_price: ren,
            transfer_price: tra,
            currency: "USD".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    fn make_client(base_url: &str) -> PorkbunClient {
        PorkbunClient::with_base_url(
            "test_key".to_string(),
            "test_secret".to_string(),
            reqwest::Client::new(),
            base_url.to_string(),
        )
    }

    #[tokio::test]
    async fn test_list_dns_records_success() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/dns/retrieve/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"SUCCESS","records":[{"id":"123","name":"example.com","type":"A","content":"1.2.3.4","ttl":"300","prio":"0"}]}"#)
            .create_async().await;

        let client = make_client(&server.url());
        let records = client.list_dns_records("example.com").await.unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_list_dns_records_api_error() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/dns/retrieve/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ERROR","message":"Domain not found"}"#)
            .create_async().await;

        let client = make_client(&server.url());
        let result = client.list_dns_records("example.com").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_availability() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domain/checkAvailability")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"SUCCESS","avail":"yes","price":"9.73"}"#)
            .create_async().await;

        let client = make_client(&server.url());
        let avail = client.check_availability("test123.com").await.unwrap();
        assert!(avail.available);
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/dns/delete/example.com/456")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"SUCCESS"}"#)
            .create_async().await;

        let client = make_client(&server.url());
        client.delete_dns_record("example.com", "456").await.unwrap();
    }
}
