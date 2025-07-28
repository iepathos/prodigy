use assert_cmd::Command;

#[test]
fn test_cli_parsing() {
    // Test that CLI can be parsed without type conflicts
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.arg("--help").assert().success();
}

#[test]
fn test_max_iterations_arg_parsing() {
    // Test max iterations argument parsing for cook subcommand
    let mut cmd = Command::cargo_bin("mmm").unwrap();
    cmd.args(["cook", "-n", "5", "--help"]).assert().success();
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
    cmd.args(["cook", "-n", "5", "--show-progress", "-v", "--help"])
        .assert()
        .success();
}
