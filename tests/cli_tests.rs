use assert_cmd::Command;

#[test]
fn test_cli_parsing() {
    // Test that all commands can be parsed without type conflicts
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--help").assert().success();
}

#[test]
fn test_improve_command_parsing() {
    // Test improve command specifically
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["improve", "--help"]).assert().success();
}

#[test]
fn test_verbose_flags_dont_conflict() {
    // Test that global verbose and improve show-progress don't conflict (help mode)
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["-v", "improve", "--help"]).assert().success();
}

#[test]
fn test_improve_args_parsing() {
    // Test improve command argument parsing (help mode to avoid hanging)
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["improve", "--target", "9.0", "--show-progress", "--help"])
        .assert()
        .success();
}
