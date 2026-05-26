Feature: Domain registration
  Scenario: Check domain availability
    Given the Porkbun provider is configured
    When I check availability for "example123test.com"
    Then the response should indicate availability status

  Scenario: Check an unavailable domain
    Given the Porkbun provider is configured
    When I check availability for "google.com"
    Then the domain should be reported as unavailable
