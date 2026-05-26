use reqwest::Client;
use async_trait::async_trait;
use crate::core::{error::Error, traits::DomainRegistrar, types::*};

/// Namecheap uses an XML-based API, which is structurally different.
/// This skeleton stub returns Unsupported for all methods.
pub struct NamecheapClient {
    _api_user: String,
    _api_key: String,
    _username: String,
    _http: Client,
}

impl NamecheapClient {
    pub fn new(api_user: String, api_key: String, username: String, http: Client) -> Self {
        Self { _api_user: api_user, _api_key: api_key, _username: username, _http: http }
    }
}

#[async_trait]
impl DomainRegistrar for NamecheapClient {
    async fn check_availability(&self, _domain: &str) -> Result<Availability, Error> {
        Err(Error::Unsupported)
    }
    async fn register_domain(&self, _domain: &str, _records: Option<Vec<DnsRecord>>) -> Result<Domain, Error> {
        Err(Error::Unsupported)
    }
    async fn renew_domain(&self, _domain: &str, _years: u32) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    async fn transfer_domain(&self, _domain: &str, _auth_code: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    async fn get_domain_info(&self, _domain: &str) -> Result<Domain, Error> {
        Err(Error::Unsupported)
    }
    async fn list_dns_records(&self, _domain: &str) -> Result<Vec<DnsRecord>, Error> {
        Err(Error::Unsupported)
    }
    async fn create_dns_record(&self, _domain: &str, _record: &DnsRecord) -> Result<DnsRecord, Error> {
        Err(Error::Unsupported)
    }
    async fn update_dns_record(&self, _domain: &str, _record_id: &str, _record: &DnsRecord) -> Result<DnsRecord, Error> {
        Err(Error::Unsupported)
    }
    async fn delete_dns_record(&self, _domain: &str, _record_id: &str) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    async fn set_nameservers(&self, _domain: &str, _nameservers: &[String]) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
    async fn get_nameservers(&self, _domain: &str) -> Result<Vec<String>, Error> {
        Err(Error::Unsupported)
    }
    async fn get_pricing(&self, _tld: &str) -> Result<Pricing, Error> {
        Err(Error::Unsupported)
    }
}
