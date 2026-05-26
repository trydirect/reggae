use reqwest::Client;
use async_trait::async_trait;
use crate::core::{error::Error, traits::DomainRegistrar, types::*};

const BASE_URL: &str = "https://api.godaddy.com";

pub struct GodaddyClient {
    api_key: String,
    api_secret: String,
    base_url: String,
    http: Client,
}

impl GodaddyClient {
    pub fn new(api_key: String, api_secret: String, http: Client) -> Self {
        Self { api_key, api_secret, base_url: BASE_URL.to_string(), http }
    }

    fn auth_header(&self) -> String {
        format!("sso-key {}:{}", self.api_key, self.api_secret)
    }
}

#[async_trait]
impl DomainRegistrar for GodaddyClient {
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
