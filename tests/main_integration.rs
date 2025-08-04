use anyhow::Result;
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_main_help_command() -> Result<()> {
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("memento-mori management"));
    Ok(())
}

#[test]
fn test_main_version_command() -> Result<()> {
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
    Ok(())
}

#[test]
fn test_main_invalid_command() -> Result<()> {
    let mut cmd = Command::cargo_bin("mmm")?;
    cmd.arg("invalid-command");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("error"));
    Ok(())
}
