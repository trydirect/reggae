use async_trait::async_trait;
use crate::core::{error::Error, types::*};

#[async_trait]
pub trait DomainRegistrar: Send + Sync {
    async fn check_availability(&self, domain: &str) -> Result<Availability, Error>;

    /// Register a domain. `contact` is required by most registrars.
    async fn register_domain(
        &self,
        domain: &str,
        years: u32,
        contact: &RegistrantContact,
        records: Option<Vec<DnsRecord>>,
    ) -> Result<Domain, Error>;

    async fn renew_domain(&self, domain: &str, years: u32) -> Result<(), Error>;
    async fn transfer_domain(&self, domain: &str, auth_code: &str) -> Result<(), Error>;
    async fn get_domain_info(&self, domain: &str) -> Result<Domain, Error>;
    async fn list_dns_records(&self, domain: &str) -> Result<Vec<DnsRecord>, Error>;
    async fn create_dns_record(&self, domain: &str, record: &DnsRecord) -> Result<DnsRecord, Error>;
    async fn update_dns_record(&self, domain: &str, record_id: &str, record: &DnsRecord) -> Result<DnsRecord, Error>;
    async fn delete_dns_record(&self, domain: &str, record_id: &str) -> Result<(), Error>;
    async fn set_nameservers(&self, domain: &str, nameservers: &[String]) -> Result<(), Error>;
    async fn get_nameservers(&self, domain: &str) -> Result<Vec<String>, Error>;
    async fn get_pricing(&self, tld: &str) -> Result<Pricing, Error>;
}
