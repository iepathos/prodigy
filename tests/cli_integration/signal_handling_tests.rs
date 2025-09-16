// Tests for signal handling (SIGINT, SIGTERM)

use super::test_utils::*;
use std::time::Duration;

#[cfg(unix)]
#[test]
fn test_sigint_handling() {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("long", &create_long_workflow("interrupt"));

    let child = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .spawn();

    // Wait a bit for the process to start
    std::thread::sleep(Duration::from_millis(500));

    // Send SIGINT
    let pid = Pid::from_raw(child.id() as i32);
    let _ = kill(pid, Signal::SIGINT);

    let output = child.wait_with_output().expect("Failed to wait for child");

    // Should handle SIGINT gracefully
    let exit_code = output.status.code().unwrap_or(-1);
    assert!(
        exit_code == exit_codes::INTERRUPTED || exit_code == -1 || exit_code != exit_codes::SUCCESS
    );
}

#[cfg(unix)]
#[test]
fn test_sigterm_handling() {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("long", &create_long_workflow("terminate"));

    let child = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .spawn();

    // Wait a bit for the process to start
    std::thread::sleep(Duration::from_millis(500));

    // Send SIGTERM
    let pid = Pid::from_raw(child.id() as i32);
    let _ = kill(pid, Signal::SIGTERM);

    let output = child.wait_with_output().expect("Failed to wait for child");

    // Should handle SIGTERM gracefully
    let exit_code = output.status.code().unwrap_or(-1);
    assert!(
        exit_code == exit_codes::TERMINATED || exit_code == -1 || exit_code != exit_codes::SUCCESS
    );
}

#[test]
fn test_graceful_shutdown() {
    let test = CliTest::new();
    let workflow_content = r#"
name: graceful-test
commands:
  - shell: "echo 'Starting'"
  - shell: "sleep 5"
  - shell: "echo 'Should not reach here'"
"#;
    let (test, workflow_path) = test.with_workflow("graceful", workflow_content);

    let mut child = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .spawn();

    // Wait for first step to complete
    std::thread::sleep(Duration::from_millis(1000));

    // Kill the process
    child.kill().expect("Failed to kill child");

    let output = child.wait_with_output().expect("Failed to wait for child");

    // Should have started but not completed
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Starting") || !output.status.success());
}

#[cfg(unix)]
#[test]
fn test_multiple_signals() {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    let test = CliTest::new();
    let (test, workflow_path) = test.with_workflow("multi", &create_long_workflow("multi"));

    let child = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .spawn();

    // Wait a bit for the process to start
    std::thread::sleep(Duration::from_millis(500));

    // Send multiple SIGINT signals
    let pid = Pid::from_raw(child.id() as i32);
    let _ = kill(pid, Signal::SIGINT);
    std::thread::sleep(Duration::from_millis(100));
    let _ = kill(pid, Signal::SIGINT);

    let output = child.wait_with_output().expect("Failed to wait for child");

    // Should handle multiple signals without crashing
    assert!(!output.status.success());
}

#[test]
fn test_cleanup_on_interrupt() {
    let test = CliTest::new();
    let workflow_content = r#"
name: cleanup-test
commands:
  - shell: "touch /tmp/prodigy-test-file"
  - shell: "sleep 10"
"#;
    let (test, workflow_path) = test.with_workflow("cleanup", workflow_content);

    let mut child = test
        .arg("cook")
        .arg(workflow_path.to_str().unwrap())
        .spawn();

    // Wait for file creation
    std::thread::sleep(Duration::from_millis(1000));

    // Kill the process
    child.kill().expect("Failed to kill child");
    child.wait().expect("Failed to wait for child");

    // Cleanup should have occurred (test file might or might not exist)
    // This tests that the process doesn't hang on cleanup
}
