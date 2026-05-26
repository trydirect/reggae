/// Provider integration tests use mockito to stand in for real APIs.
/// These test the Porkbun client via the trait interface.
#[cfg(test)]
mod porkbun_tests {
    use reggae::providers::porkbun::PorkbunClient;
    use reggae::core::traits::DomainRegistrar;
    use reggae::core::types::DnsRecordType;
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
    async fn test_create_dns_record() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/dns/create/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"SUCCESS","id":789}"#)
            .create_async().await;

        use reggae::core::types::DnsRecord;
        let client = make_client(&server.url());
        let record = DnsRecord {
            id: None,
            record_type: DnsRecordType::A,
            name: "@".to_string(),
            content: "1.2.3.4".to_string(),
            ttl: Some(300),
            priority: None,
        };
        let created = client.create_dns_record("example.com", &record).await.unwrap();
        assert_eq!(created.id.unwrap(), "789");
    }

    #[tokio::test]
    async fn test_get_nameservers() {
        let mut server = Server::new_async().await;
        let _m = server.mock("POST", "/domain/getNs/example.com")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"SUCCESS","ns":["ns1.porkbun.com","ns2.porkbun.com"]}"#)
            .create_async().await;

        let client = make_client(&server.url());
        let ns = client.get_nameservers("example.com").await.unwrap();
        assert_eq!(ns.len(), 2);
        assert_eq!(ns[0], "ns1.porkbun.com");
    }
}
