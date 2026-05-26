Feature: Domain expiry check
  Background:
    Given the scheduler is configured with 30 days lead time

  Scenario: Domain expiring soon triggers notification
    Given a domain "example.com" expiring in 10 days
    When the expiry check runs
    Then a notification should be sent for "example.com"

  Scenario: Domain not expiring soon does not trigger notification
    Given a domain "safe.com" expiring in 60 days
    When the expiry check runs
    Then no notification should be sent for "safe.com"
