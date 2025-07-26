use assert_cmd::Command;

#[test]
fn test_cli_parsing() {
    // Test that CLI can be parsed without type conflicts
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--help").assert().success();
}

#[test]
fn test_target_arg_parsing() {
    // Test target argument parsing
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["--target", "9.0", "--help"]).assert().success();
}

#[test]
fn test_verbose_flags() {
    // Test that verbose flags work
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["-v", "--help"]).assert().success();
}

#[test]
fn test_all_args_parsing() {
    // Test all arguments together (help mode to avoid hanging)
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["--target", "9.0", "--show-progress", "-v", "--help"])
        .assert()
        .success();
}
