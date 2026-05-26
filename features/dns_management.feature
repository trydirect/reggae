Feature: DNS record management
  Background:
    Given the Porkbun provider is configured

  Scenario: List DNS records for a domain
    When I list DNS records for "example.com"
    Then I should receive a list of DNS records

  Scenario: Add an A record
    When I add an A record "1.2.3.4" for "example.com"
    Then the record should be created successfully

  Scenario: Delete a DNS record
    Given a DNS record with id "123" exists for "example.com"
    When I delete record "123" from "example.com"
    Then the record should be deleted successfully
