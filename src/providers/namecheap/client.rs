use reqwest::Client;
use async_trait::async_trait;
use tracing::instrument;
use chrono::{DateTime, Utc, NaiveDateTime};

use crate::core::{error::Error, traits::DomainRegistrar, types::{*, RegistrantContact}};


const PROD_URL:    &str = "https://api.namecheap.com/xml.response";
const SANDBOX_URL: &str = "https://api.sandbox.namecheap.com/xml.response";

pub struct NamecheapClient {
    api_user:   String,
    api_key:    String,
    username:   String,
    client_ip:  String,
    contact:    RegistrantContact,
    base_url:   String,
    http:       Client,
}

impl NamecheapClient {
    pub fn new(
        api_user: String, api_key: String, username: String,
        client_ip: String, contact: RegistrantContact,
        sandbox: bool, http: Client,
    ) -> Self {
        let base_url = if sandbox { SANDBOX_URL } else { PROD_URL }.to_string();
        Self::with_base_url(api_user, api_key, username, client_ip, contact, http, base_url)
    }

    pub fn with_base_url(
        api_user: String, api_key: String, username: String,
        client_ip: String, contact: RegistrantContact,
        http: Client, base_url: String,
    ) -> Self {
        Self { api_user, api_key, username, client_ip, contact, base_url, http }
    }

    /// Build common auth params + command
    fn base_params(&self, command: &str) -> Vec<(&'static str, String)> {
        vec![
            ("ApiUser",  self.api_user.clone()),
            ("ApiKey",   self.api_key.clone()),
            ("UserName", self.username.clone()),
            ("ClientIp", self.client_ip.clone()),
            ("Command",  command.to_string()),
        ]
    }

    async fn call(&self, params: Vec<(impl AsRef<str>, impl AsRef<str>)>) -> Result<String, Error> {
        let mut url = reqwest::Url::parse(&self.base_url).map_err(|e| Error::Provider(format!("URL parse error: {}", e)))?;
        {
            let mut q = url.query_pairs_mut();
            for (k, v) in &params {
                q.append_pair(k.as_ref(), v.as_ref());
            }
        }
        let body = self.http.get(url).send().await?.text().await?;
        Ok(body)
    }

    fn check_xml_errors(xml: &str) -> Result<(), Error> {
        let doc = roxmltree::Document::parse(xml)
            .map_err(|e| Error::Provider(format!("XML parse error: {}", e)))?;
        let root = doc.root_element();
        let status = root.attribute("Status").unwrap_or("ERROR");
        if status == "ERROR" || status == "FAILED" {
            let msg = root.descendants()
                .find(|n| n.has_tag_name("Error"))
                .and_then(|n| n.text())
                .unwrap_or("Namecheap API error")
                .trim()
                .to_string();
            return Err(Error::Provider(msg));
        }
        Ok(())
    }

    /// Split "example.com" → ("example", "com")
    fn sld_tld(domain: &str) -> (String, String) {
        let parts: Vec<&str> = domain.splitn(2, '.').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (domain.to_string(), String::new())
        }
    }

    fn contact_params(prefix: &str, c: &RegistrantContact) -> Vec<(String, String)> {
        vec![
            (format!("{}FirstName",   prefix), c.first_name.clone()),
            (format!("{}LastName",    prefix), c.last_name.clone()),
            (format!("{}Address1",    prefix), c.address1.clone()),
            (format!("{}City",        prefix), c.city.clone()),
            (format!("{}StateProvince", prefix), c.state.clone()),
            (format!("{}PostalCode",  prefix), c.postal_code.clone()),
            (format!("{}Country",     prefix), c.country.clone()),
            (format!("{}Phone",       prefix), c.phone.clone()),
            (format!("{}EmailAddress", prefix), c.email.clone()),
            (format!("{}OrganizationName", prefix), c.organization.clone().unwrap_or_default()),
        ]
    }

    fn parse_hosts(xml: &str) -> Result<Vec<DnsRecord>, Error> {
        let doc = roxmltree::Document::parse(xml)
            .map_err(|e| Error::Provider(format!("XML parse error: {}", e)))?;
        let mut records = Vec::new();
        for node in doc.descendants() {
            if node.has_tag_name("host") {
                let rtype_str = node.attribute("Type").unwrap_or("");
                let record_type = match rtype_str {
                    "A"     => DnsRecordType::A,
                    "AAAA"  => DnsRecordType::Aaaa,
                    "CNAME" => DnsRecordType::Cname,
                    "MX"    => DnsRecordType::Mx,
                    "TXT"   => DnsRecordType::Txt,
                    "NS"    => DnsRecordType::Ns,
                    "SRV"   => DnsRecordType::Srv,
                    "CAA"   => DnsRecordType::Caa,
                    _       => continue,
                };
                let id       = node.attribute("HostId").map(String::from);
                let name     = node.attribute("Name").unwrap_or("@").to_string();
                let content  = node.attribute("Address").unwrap_or("").to_string();
                let ttl      = node.attribute("TTL").and_then(|t| t.parse().ok());
                let priority = node.attribute("MXPref").and_then(|p| p.parse().ok());
                records.push(DnsRecord { id, record_type, name, content, ttl, priority });
            }
        }
        Ok(records)
    }

    /// Namecheap requires pushing the full host set atomically.
    async fn set_all_hosts(&self, domain: &str, records: &[DnsRecord]) -> Result<(), Error> {
        let (sld, tld) = Self::sld_tld(domain);
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.dns.setHosts")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("SLD".into(), sld));
        params.push(("TLD".into(), tld));

        for (i, rec) in records.iter().enumerate() {
            let n = i + 1;
            params.push((format!("HostName{}", n),   rec.name.clone()));
            params.push((format!("RecordType{}", n), rec.record_type.to_string()));
            params.push((format!("Address{}", n),    rec.content.clone()));
            params.push((format!("TTL{}", n),        rec.ttl.unwrap_or(1800).to_string()));
            if let Some(prio) = rec.priority {
                params.push((format!("MXPref{}", n), prio.to_string()));
            }
        }

        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
        Ok(())
    }
}

#[async_trait]
impl DomainRegistrar for NamecheapClient {
    #[instrument(skip(self))]
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error> {
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.check")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("DomainList".into(), domain.to_string()));

        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;

        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| Error::Provider(format!("XML parse error: {}", e)))?;
        let available = doc.descendants()
            .find(|n| n.has_tag_name("DomainCheckResult"))
            .and_then(|n| n.attribute("Available"))
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let premium = doc.descendants()
            .find(|n| n.has_tag_name("DomainCheckResult"))
            .and_then(|n| n.attribute("IsPremiumName"))
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let price = doc.descendants()
            .find(|n| n.has_tag_name("DomainCheckResult"))
            .and_then(|n| n.attribute("PremiumRegistrationPrice"))
            .and_then(|p| p.parse::<f64>().ok())
            .filter(|&p| p > 0.0);
        Ok(Availability { available, premium, price })
    }

    #[instrument(skip(self, records))]
    async fn register_domain(&self, domain: &str, _years: u32, contact: &RegistrantContact, records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        let (sld, tld) = Self::sld_tld(domain);
        let c = contact;

        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.create")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("DomainName".into(), domain.to_string()));
        params.push(("Years".into(), "1".into()));

        // All four contact roles — Namecheap requires all of them
        for prefix in &["Registrant", "Tech", "Admin", "AuxBilling"] {
            params.extend(Self::contact_params(prefix, c));
        }

        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;

        // Push initial DNS records if provided
        if let Some(recs) = records {
            if !recs.is_empty() {
                let _ = self.set_all_hosts(domain, &recs).await;
            }
        }

        self.get_domain_info(domain).await
    }

    #[instrument(skip(self))]
    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error> {
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.renew")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("DomainName".into(), domain.to_string()));
        params.push(("Years".into(), years.to_string()));
        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error> {
        let c = &self.contact;
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.transfer.create")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("DomainName".into(), domain.to_string()));
        params.push(("EPPCode".into(), auth_code.to_string()));
        params.push(("Years".into(), "1".into()));
        params.extend(Self::contact_params("", c));
        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error> {
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.getInfo")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("DomainName".into(), domain.to_string()));

        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;

        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| Error::Provider(format!("XML parse error: {}", e)))?;

        let expiry = doc.descendants()
            .find(|n| n.has_tag_name("DomainDetails"))
            .and_then(|n| n.descendants().find(|c| c.has_tag_name("ExpiredDate")))
            .and_then(|n| n.text())
            .and_then(|s| NaiveDateTime::parse_from_str(s.trim(), "%m/%d/%Y %H:%M:%S").ok())
            .map(|dt| DateTime::from_naive_utc_and_offset(dt, Utc));

        let ns: Vec<String> = doc.descendants()
            .filter(|n| n.has_tag_name("Nameserver"))
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();

        let status_str = doc.descendants()
            .find(|n| n.has_tag_name("DomainGetInfoResult"))
            .and_then(|n| n.attribute("Status"))
            .unwrap_or("ACTIVE");
        let status = match status_str.to_uppercase().as_str() {
            "EXPIRED"  => DomainStatus::Expired,
            "TRANSFER" => DomainStatus::Unknown,
            _          => DomainStatus::Active,
        };

        Ok(Domain { name: domain.to_string(), expiry_date: expiry, status, nameservers: ns })
    }

    #[instrument(skip(self))]
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error> {
        let (sld, tld) = Self::sld_tld(domain);
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.dns.getHosts")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("SLD".into(), sld));
        params.push(("TLD".into(), tld));
        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
        Self::parse_hosts(&xml)
    }

    /// Namecheap uses atomic setHosts: fetch all → append new record → push full set.
    #[instrument(skip(self, record))]
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut records = self.list_dns_records(domain).await?;
        let mut new_rec = record.clone();
        records.push(record.clone());
        self.set_all_hosts(domain, &records).await?;
        // Assign a synthetic id after push
        new_rec.id = Some(format!("{}:{}", record.record_type, record.name));
        Ok(new_rec)
    }

    /// Update matching record by id (type:name) then push full set.
    #[instrument(skip(self, record))]
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error> {
        let mut records = self.list_dns_records(domain).await?;
        let mut found = false;
        for r in &mut records {
            let synthetic_id = format!("{}:{}", r.record_type, r.name);
            if r.id.as_deref() == Some(record_id) || synthetic_id == record_id {
                *r = record.clone();
                r.id = Some(record_id.to_string());
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::Provider(format!("Record '{}' not found", record_id)));
        }
        self.set_all_hosts(domain, &records).await?;
        let mut updated = record.clone();
        updated.id = Some(record_id.to_string());
        Ok(updated)
    }

    /// Remove matching record by id (type:name) then push remaining set.
    #[instrument(skip(self))]
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error> {
        let records = self.list_dns_records(domain).await?;
        let filtered: Vec<DnsRecord> = records.into_iter().filter(|r| {
            let synthetic = format!("{}:{}", r.record_type, r.name);
            r.id.as_deref() != Some(record_id) && synthetic != record_id
        }).collect();
        self.set_all_hosts(domain, &filtered).await?;
        Ok(())
    }

    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error> {
        let (sld, tld) = Self::sld_tld(domain);
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.dns.getCustom")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("SLD".into(), sld));
        params.push(("TLD".into(), tld));
        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
        let doc = roxmltree::Document::parse(&xml)
            .map_err(|e| Error::Provider(format!("XML parse error: {}", e)))?;
        let ns: Vec<String> = doc.descendants()
            .filter(|n| n.has_tag_name("Nameserver"))
            .filter_map(|n| n.text().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        Ok(ns)
    }

    #[instrument(skip(self, nameservers))]
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error> {
        let (sld, tld) = Self::sld_tld(domain);
        let mut params: Vec<(String, String)> = self.base_params("namecheap.domains.dns.setCustom")
            .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        params.push(("SLD".into(), sld));
        params.push(("TLD".into(), tld));
        params.push(("Nameservers".into(), nameservers.join(",")));
        let xml = self.call(params).await?;
        Self::check_xml_errors(&xml)?;
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
    

    fn client(base_url: &str) -> NamecheapClient {
        NamecheapClient::with_base_url(
            "user".into(), "key".into(), "user".into(), "1.2.3.4".into(),
            RegistrantContact {
                first_name: "Jane".into(), last_name: "Doe".into(),
                email: "jane@example.com".into(), phone: "+1.5555555555".into(),
                address1: "123 Main".into(), address2: None,
                city: "Phoenix".into(), state: "AZ".into(),
                postal_code: "85001".into(), country: "US".into(),
                organization: None,
            },
            reqwest::Client::new(),
            base_url.to_string(),
        )
    }

    const CHECK_XML: &str = r#"<?xml version="1.0"?>
<ApiResponse Status="OK" xmlns="https://api.namecheap.com/xml.response">
  <Errors/>
  <CommandResponse Type="namecheap.domains.check">
    <DomainCheckResult Domain="example.com" Available="true" IsPremiumName="false" PremiumRegistrationPrice="0.00"/>
  </CommandResponse>
</ApiResponse>"#;

    const HOSTS_XML: &str = r#"<?xml version="1.0"?>
<ApiResponse Status="OK" xmlns="https://api.namecheap.com/xml.response">
  <Errors/>
  <CommandResponse>
    <DnsHostsResult>
      <host HostId="1" Name="@" Type="A" Address="1.2.3.4" TTL="1800"/>
      <host HostId="2" Name="www" Type="CNAME" Address="@" TTL="1800"/>
    </DnsHostsResult>
  </CommandResponse>
</ApiResponse>"#;

    const SET_OK_XML: &str = r#"<?xml version="1.0"?>
<ApiResponse Status="OK" xmlns="https://api.namecheap.com/xml.response">
  <Errors/>
  <CommandResponse><DnsSetHostsResult IsSuccess="true"/></CommandResponse>
</ApiResponse>"#;

    const DOMAIN_INFO_XML: &str = r#"<?xml version="1.0"?>
<ApiResponse Status="OK" xmlns="https://api.namecheap.com/xml.response">
  <Errors/>
  <CommandResponse>
    <DomainGetInfoResult Status="OK">
      <DomainDetails>
        <ExpiredDate>12/31/2025 00:00:00</ExpiredDate>
      </DomainDetails>
      <DnsDetails>
        <Nameserver>ns1.namecheap.com</Nameserver>
        <Nameserver>ns2.namecheap.com</Nameserver>
      </DnsDetails>
    </DomainGetInfoResult>
  </CommandResponse>
</ApiResponse>"#;

    #[tokio::test]
    async fn test_check_availability() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(CHECK_XML)
            .create_async().await;
        let c = client(&server.url());
        let av = c.check_availability("example.com").await.unwrap();
        assert!(av.available);
        assert!(!av.premium);
    }

    #[tokio::test]
    async fn test_list_dns_records() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(HOSTS_XML)
            .create_async().await;
        let c = client(&server.url());
        let recs = c.list_dns_records("example.com").await.unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].content, "1.2.3.4");
    }

    #[tokio::test]
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        // list call
        let _m1 = server.mock("GET", mockito::Matcher::Any)
            .with_body(HOSTS_XML)
            .expect(1)
            .create_async().await;
        // setHosts call
        let _m2 = server.mock("GET", mockito::Matcher::Any)
            .with_body(SET_OK_XML)
            .expect(1)
            .create_async().await;
        let c = client(&server.url());
        let rec = DnsRecord { id: None, record_type: DnsRecordType::Txt, name: "@".into(), content: "v=spf1 ~all".into(), ttl: Some(300), priority: None };
        let created = c.create_dns_record("example.com", &rec).await.unwrap();
        assert_eq!(created.id.unwrap(), "TXT:@");
    }

    #[tokio::test]
    async fn test_set_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(SET_OK_XML)
            .create_async().await;
        let c = client(&server.url());
        c.set_nameservers("example.com", &["ns1.test.com".into(), "ns2.test.com".into()]).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_domain_info() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", mockito::Matcher::Any)
            .with_body(DOMAIN_INFO_XML)
            .create_async().await;
        let c = client(&server.url());
        let info = c.get_domain_info("example.com").await.unwrap();
        assert_eq!(info.nameservers, vec!["ns1.namecheap.com", "ns2.namecheap.com"]);
        assert!(info.expiry_date.is_some());
    }
}
