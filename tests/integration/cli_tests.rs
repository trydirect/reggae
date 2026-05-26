use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_output() {
    let mut cmd = Command::cargo_bin("reggae").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Manage domains and DNS records"));
}

#[test]
fn test_check_help() {
    let mut cmd = Command::cargo_bin("reggae").unwrap();
    cmd.args(["check", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("domain"));
}

#[test]
fn test_dns_help() {
    let mut cmd = Command::cargo_bin("reggae").unwrap();
    cmd.args(["dns", "--help"])
        .assert()
        .success();
}

#[test]
fn test_version() {
    let mut cmd = Command::cargo_bin("reggae").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}
