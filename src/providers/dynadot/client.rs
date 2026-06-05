use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, instrument};

use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const PROD_URL: &str = "https://api.dynadot.com/api3.xml";
const SANDBOX_URL: &str = "https://api-sandbox.dynadot.com/api3.xml";

pub struct DynadotClient {
    api_key: String,
    base_url: String,
    http: Client,
}

impl DynadotClient {
    pub fn new(api_key: String, sandbox: bool, http: Client) -> Self {
        let base_url = if sandbox { SANDBOX_URL } else { PROD_URL }.to_string();
        Self::with_base_url(api_key, http, base_url)
    }

    pub fn with_base_url(api_key: String, http: Client, base_url: String) -> Self {
        Self { api_key, base_url, http }
    }

    async fn call(&self, command: &str, params: Vec<(String, String)>) -> Result<String, Error> {
        let mut url = reqwest::Url::parse(&self.base_url)
            .map_err(|e| Error::Provider(format!("URL parse: {}", e)))?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("key", &self.api_key);
            q.append_pair("command", command);
            for (k, v) in &params {
                q.append_pair(k, v);
            }
        }
        debug!("GET {}", url);
        let xml = self.http.get(url).send().await?.text().await?;
        debug!("Response: {}", &xml[..xml.len().min(512)]);
        Self::check_errors(&xml)?;
        Ok(xml)
    }

    fn check_errors(xml: &str) -> Result<(), Error> {
        let doc = roxmltree::Document::parse(xml)
            .map_err(|e| Error::Provider(format!("XML parse: {}", e)))?;
        let code = doc.descendants()
            .find(|n| n.has_tag_name("SuccessCode"))
            .and_then(|n| n.text())
            .unwrap_or("0");
        if code.trim() != "0" {
            let msg = doc.descendants()
                .find(|n| n.has_tag_name("Error"))
                .and_then(|n| n.text())
                .unwrap_or("Dynadot API error")
                .trim()
                .to_string();
            return Err(Error::Provider(msg));
        }
        Ok(())
    }

    fn parse_dns(xml: &str) -> Result<Vec<DnsRecord>, Error> {
        let doc = roxmltree::Document::parse(xml)
            .map_err(|e| Error::Provider(format!("XML parse: {}", e)))?;
        let mut records = Vec::new();
        for node in doc.descendants() {
            if !node.has_tag_name("Dns2Record") { continue; }

            let rtype_str = child_text(&node, "RecordType").unwrap_or_default();
            let record_type = match rtype_str {
                "A"     => DnsRecordType::A,
                "AAAA"  => DnsRecordType::Aaaa,
                "CNAME" => DnsRecordType::Cname,
                "MX"    => DnsRecordType::Mx,
                "TXT"   => DnsRecordType::Txt,
                "NS"    => DnsRecordType::Ns,
                "SRV"   => DnsRecordType::Srv,
                "CAA"   => DnsRecordType::Caa,
                _ => continue,
            };
            let name     = child_text(&node, "Subdomain").unwrap_or("@").to_string();
            let content  = child_text(&node, "Value").unwrap_or("").to_string();
            let ttl_raw  = child_text(&node, "Ttl").unwrap_or("default");
            let ttl      = if ttl_raw == "default" { None } else { ttl_raw.parse().ok() };
            let priority = child_text(&node, "Distance").and_then(|t| t.parse().ok());
            let id       = Some(format!("{}:{}", record_type, name));
            records.push(DnsRecord { id, record_type, name, content, ttl, priority });
        }
        Ok(records)
    }

    // Dynadot replaces all DNS records atomically via set_dns2.
    async fn push_dns(&self, domain: &str, records: &[DnsRecord]) -> Result<(), Error> {
        let mut params = vec![("domain".to_string(), domain.to_string())];
        for (i, rec) in records.iter().enumerate() {
            let ttl = rec.ttl.map(|t| t.to_string()).unwrap_or_else(|| "default".to_string());
            params.push((format!("main_record_type{}", i), rec.record_type.to_string()));
            params.push((format!("main_subdomain{}", i),   rec.name.clone()));
            params.push((format!("main_value{}", i),       rec.content.clone()));
            params.push((format!("main_ttl{}", i),         ttl));
            if let Some(p) = rec.priority {
                params.push((format!("main_distance{}", i), p.to_string()));
            }
        }
        self.call("set_dns2", params).await?;
        Ok(())
    }
}

fn child_text<'a>(node: &roxmltree::Node<'a, '_>, tag: &str) -> Option<&'a str> {
    node.children()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
}

fn desc_text(doc: &roxmltree::Document<'_>, tag: &str) -> Option<String> {
    doc.descendants()
        .find(|n| n.has_tag_name(tag))
        .and_then(|n| n.text())
        .map(str::to_owned)
}

#[async_trait]
impl DomainRegistrar for DynadotClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let xml = self.call("domain_search", vec![
            ("domain0".to_string(), domain.to_string()),
        ]).await?;
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| Error::Provider(format!("XML parse: {}", e)))?;
        let available = desc_text(&doc, "Available")
            .map(|t| t.trim().eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let price = desc_text(&doc, "Price")
            .and_then(|t| t.trim().parse::<f64>().ok())
            .filter(|&p| p > 0.0);
        Ok(Availability { available, premium: false, price })
    }

    #[instrument(skip(self, _contact, records))]
    async fn register_domain(
        &self,
        domain: &str,
        years: u32,
        _contact: &RegistrantContact,
        records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error> {
        // Dynadot uses the account's default registrant contact for registration.
        self.call("register", vec![
            ("domain".to_string(),   domain.to_string()),
            ("duration".to_string(), years.to_string()),
        ]).await?;
        if let Some(recs) = records {
            if !recs.is_empty() {
                let _ = self.push_dns(domain, &recs).await;
            }
        }
        self.get_domain_info(domain).await
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        self.call("renew", vec![
            ("domain".to_string(),   domain.to_string()),
            ("duration".to_string(), years.to_string()),
        ]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        self.call("transfer", vec![
            ("domain".to_string(),   domain.to_string()),
            ("authcode".to_string(), auth_code.to_string()),
        ]).await?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let xml = self.call("get_domain_info", vec![
            ("domain".to_string(), domain.to_string()),
        ]).await?;
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| Error::Provider(format!("XML parse: {}", e)))?;

        // Dynadot returns expiration as a Unix timestamp in milliseconds.
        let expiry = desc_text(&doc, "Expiration")
            .and_then(|t| t.trim().parse::<i64>().ok())
            .and_then(|ms| chrono::DateTime::from_timestamp(ms / 1000, 0));

        let nameservers: Vec<String> = doc.descendants()
            .filter(|n| n.has_tag_name("Host"))
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();

        Ok(Domain {
            name: domain.to_string(),
            expiry_date: expiry,
            status: DomainStatus::Active,
            nameservers,
        })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let xml = self.call("get_dns", vec![
            ("domain".to_string(), domain.to_string()),
        ]).await?;
        Self::parse_dns(&xml)
    }

    // fetch current records → append → push all
    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut records = self.list_dns_records(domain).await?;
        records.push(record.clone());
        self.push_dns(domain, &records).await?;
        let mut created = record.clone();
        created.id = Some(format!("{}:{}", record.record_type, record.name));
        Ok(created)
    }

    // fetch current records → replace matching → push all
    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut records = self.list_dns_records(domain).await?;
        let mut found = false;
        for r in &mut records {
            let syn = format!("{}:{}", r.record_type, r.name);
            if r.id.as_deref() == Some(record_id) || syn == record_id {
                *r = record.clone();
                r.id = Some(record_id.to_string());
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::Provider(format!("Record '{}' not found", record_id)));
        }
        self.push_dns(domain, &records).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    // fetch current records → filter out → push remaining
    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let records = self.list_dns_records(domain).await?;
        let filtered: Vec<DnsRecord> = records.into_iter().filter(|r| {
            let syn = format!("{}:{}", r.record_type, r.name);
            r.id.as_deref() != Some(record_id) && syn != record_id
        }).collect();
        self.push_dns(domain, &filtered).await
    }

    #[instrument(skip(self))]
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        Ok(self.get_domain_info(domain).await?.nameservers)
    }

    #[instrument(skip(self))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let mut params = vec![("domain".to_string(), domain.to_string())];
        for (i, ns) in nameservers.iter().enumerate() {
            params.push((format!("ns{}", i), ns.clone()));
        }
        self.call("set_ns", params).await?;
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

    fn client(base_url: &str) -> DynadotClient {
        DynadotClient::with_base_url("test_key".to_string(), reqwest::Client::new(), base_url.to_string())
    }

    const SEARCH_AVAIL_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DomainSearchResponse>
  <DomainSearchHeader><SuccessCode>0</SuccessCode><Status>success</Status></DomainSearchHeader>
  <DomainSearchContent>
    <Results><Available>yes</Available><Price>9.99</Price></Results>
  </DomainSearchContent>
</DomainSearchResponse>"#;

    const SEARCH_TAKEN_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DomainSearchResponse>
  <DomainSearchHeader><SuccessCode>0</SuccessCode><Status>success</Status></DomainSearchHeader>
  <DomainSearchContent>
    <Results><Available>no</Available><Price>0</Price></Results>
  </DomainSearchContent>
</DomainSearchResponse>"#;

    const DOMAIN_INFO_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<GetInfoResponse>
  <GetInfoHeader><SuccessCode>0</SuccessCode><Status>success</Status></GetInfoHeader>
  <GetInfoContent>
    <Domain>
      <Name>example.com</Name>
      <Expiration>1735689600000</Expiration>
      <NameServerSettings>
        <Type>Custom</Type>
        <NameServers>
          <NameServer><Host>ns1.dynadot.com</Host></NameServer>
          <NameServer><Host>ns2.dynadot.com</Host></NameServer>
        </NameServers>
      </NameServerSettings>
    </Domain>
  </GetInfoContent>
</GetInfoResponse>"#;

    const DNS_RECORDS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<GetDnsResponse>
  <GetDnsHeader><SuccessCode>0</SuccessCode><Status>success</Status></GetDnsHeader>
  <GetDnsContent>
    <DomainDnsInfo>
      <Dns2Records>
        <Dns2Record>
          <RecordType>A</RecordType>
          <Subdomain>@</Subdomain>
          <Value>1.2.3.4</Value>
          <Ttl>300</Ttl>
        </Dns2Record>
        <Dns2Record>
          <RecordType>MX</RecordType>
          <Subdomain>@</Subdomain>
          <Value>mail.example.com</Value>
          <Ttl>3600</Ttl>
          <Distance>10</Distance>
        </Dns2Record>
      </Dns2Records>
    </DomainDnsInfo>
  </GetDnsContent>
</GetDnsResponse>"#;

    const SUCCESS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <ResponseHeader><SuccessCode>0</SuccessCode><Status>success</Status></ResponseHeader>
</Response>"#;

    const ERROR_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Response>
  <ResponseHeader>
    <SuccessCode>-1</SuccessCode>
    <Status>error</Status>
    <Error>Invalid API key</Error>
  </ResponseHeader>
</Response>"#;

    #[tokio::test]
    async fn test_check_availability_available() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(SEARCH_AVAIL_XML).create_async().await;
        let avail = client(&server.url()).check_availability("example.com").await.unwrap();
        assert!(avail.available);
        assert_eq!(avail.price, Some(9.99));
    }

    #[tokio::test]
    async fn test_check_availability_taken() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(SEARCH_TAKEN_XML).create_async().await;
        let avail = client(&server.url()).check_availability("taken.com").await.unwrap();
        assert!(!avail.available);
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(DOMAIN_INFO_XML).create_async().await;
        let d = client(&server.url()).get_domain_info("example.com").await.unwrap();
        assert_eq!(d.name, "example.com");
        assert!(d.expiry_date.is_some());
        assert_eq!(d.nameservers, vec!["ns1.dynadot.com", "ns2.dynadot.com"]);
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(DNS_RECORDS_XML).create_async().await;
        let recs = client(&server.url()).list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].content, "1.2.3.4");
        assert_eq!(recs[0].id.as_deref(), Some("A:@"));
        assert_eq!(recs[1].record_type, DnsRecordType::Mx);
        assert_eq!(recs[1].priority, Some(10));
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", mockito::Matcher::Any)
            .with_body(DNS_RECORDS_XML).expect(1).create_async().await;
        let _m2 = server.mock("GET", mockito::Matcher::Any)
            .with_body(SUCCESS_XML).expect(1).create_async().await;
        let rec = DnsRecord {
            id: None, record_type: DnsRecordType::Txt,
            name: "@".into(), content: "v=spf1 ~all".into(),
            ttl: Some(300), priority: None,
        };
        let created = client(&server.url()).create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.as_deref(), Some("TXT:@"));
    }

    #[tokio::test]
    async fn test_update_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", mockito::Matcher::Any)
            .with_body(DNS_RECORDS_XML).expect(1).create_async().await;
        let _m2 = server.mock("GET", mockito::Matcher::Any)
            .with_body(SUCCESS_XML).expect(1).create_async().await;
        let rec = DnsRecord {
            id: None, record_type: DnsRecordType::A,
            name: "@".into(), content: "5.5.5.5".into(),
            ttl: Some(300), priority: None,
        };
        let updated = client(&server.url()).update_dns_record("example.com", "A:@", &rec).await.unwrap();
        assert_eq!(updated.content, "5.5.5.5");
    }

    #[tokio::test]
    async fn test_delete_dns_record() {
        let mut server = Server::new_async().await;
        let _m1 = server.mock("GET", mockito::Matcher::Any)
            .with_body(DNS_RECORDS_XML).expect(1).create_async().await;
        let _m2 = server.mock("GET", mockito::Matcher::Any)
            .with_body(SUCCESS_XML).expect(1).create_async().await;
        client(&server.url()).delete_dns_record("example.com", "A:@").await.unwrap();
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(SUCCESS_XML).create_async().await;
        client(&server.url())
            .set_nameservers("example.com", &["ns1.test.com".into(), "ns2.test.com".into()])
            .await.unwrap();
    }

    #[tokio::test]
    async fn test_renew_domain() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(SUCCESS_XML).create_async().await;
        client(&server.url()).renew_domain("example.com", 1).await.unwrap();
    }

    #[tokio::test]
    async fn test_error_response() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(ERROR_XML).create_async().await;
        let err = client(&server.url()).list_dns_records("example.com").await.unwrap_err();
        match err {
            Error::Provider(msg) => assert!(msg.contains("Invalid API key")),
            other => panic!("Expected Provider error, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_pricing_unsupported() {
        assert!(matches!(
            client("http://unused").get_pricing("com").await,
            Err(Error::Unsupported)
        ));
    }
}
