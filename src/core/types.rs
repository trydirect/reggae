use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub name: String,
    pub expiry_date: Option<DateTime<Utc>>,
    pub status: DomainStatus,
    pub nameservers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DomainStatus {
    Active,
    Pending,
    Expired,
    Suspended,
    Unknown,
}

impl fmt::Display for DomainStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DomainStatus::Active => write!(f, "active"),
            DomainStatus::Pending => write!(f, "pending"),
            DomainStatus::Expired => write!(f, "expired"),
            DomainStatus::Suspended => write!(f, "suspended"),
            DomainStatus::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsRecord {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub record_type: DnsRecordType,
    pub name: String,
    pub content: String,
    pub ttl: Option<u32>,
    pub priority: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, clap::ValueEnum, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum DnsRecordType {
    A,
    Aaaa,
    Cname,
    Mx,
    Txt,
    Ns,
    Srv,
    Caa,
    Alias,
}

impl fmt::Display for DnsRecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DnsRecordType::A => write!(f, "A"),
            DnsRecordType::Aaaa => write!(f, "AAAA"),
            DnsRecordType::Cname => write!(f, "CNAME"),
            DnsRecordType::Mx => write!(f, "MX"),
            DnsRecordType::Txt => write!(f, "TXT"),
            DnsRecordType::Ns => write!(f, "NS"),
            DnsRecordType::Srv => write!(f, "SRV"),
            DnsRecordType::Caa => write!(f, "CAA"),
            DnsRecordType::Alias => write!(f, "ALIAS"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Availability {
    pub available: bool,
    pub premium: bool,
    pub price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pricing {
    pub registration_price: f64,
    pub renewal_price: f64,
    pub transfer_price: f64,
    pub currency: String,
}
