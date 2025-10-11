#[cfg(test)]
mod large_env_tests {
    use super::super::*;

    /// Test that demonstrates E2BIG error when passing large env vars
    /// This reproduces the bug from book-docs-drift workflow where
    /// execute_reduce_phase serialized 486KB of map.results as env var
    #[tokio::test]
    async fn test_large_env_var_causes_e2big() {
        let runner = runner::TokioProcessRunner;

        // Create 1.1MB+ JSON to exceed ARG_MAX limit (1MB on macOS)
        // ARG_MAX includes: command + args + all env vars (keys + values)
        let map_results: Vec<serde_json::Value> = (0..13)
            .map(|i| {
                serde_json::json!({
                    "item_id": format!("chapter_{}", i),
                    "status": "Success",
                    "output": "x".repeat(90000),  // 90KB per chapter = 1.17MB total
                    "commits": vec!["abc123", "def456"],
                    "files_modified": vec!["file1.md", "file2.md"],
                })
            })
            .collect();

        let huge_json = serde_json::to_string(&map_results).unwrap();
        let size_kb = huge_json.len() / 1024;
        println!("Test data size: {} KB (ARG_MAX is {} KB)", size_kb, 1024);

        // Verify we exceed ARG_MAX to trigger E2BIG
        assert!(
            size_kb > 1024,
            "Need to exceed 1MB ARG_MAX to reproduce bug"
        );

        // Try to pass this huge JSON as an environment variable
        // This is what execute_reduce_phase() was doing
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo 'Command executed'"])
            .env("MAP_RESULTS", &huge_json)
            .build();

        let result = runner.run(command).await;

        // On macOS with ARG_MAX=1MB, this should fail with E2BIG
        match result {
            Err(e) => {
                let err_msg = e.to_string();
                println!("Got expected error: {}", err_msg);
                // E2BIG shows as "Argument list too long" or "os error 7"
                assert!(
                    err_msg.contains("Argument list too long") || err_msg.contains("os error 7"),
                    "Expected E2BIG error, got: {}",
                    err_msg
                );
            }
            Ok(_) => {
                println!("WARNING: System has large ARG_MAX, test cannot reproduce E2BIG");
                println!("This is expected on some systems with high limits");
                // Don't fail - some systems have larger ARG_MAX
            }
        }
    }

    /// Test that small env vars work fine
    /// This shows the fix: only pass scalar values, not large JSON
    #[tokio::test]
    async fn test_small_env_vars_succeed() {
        let runner = runner::TokioProcessRunner;

        // The fix: only pass scalar summary values, not the huge JSON
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo \"Processed $MAP_TOTAL items: $MAP_SUCCESSFUL succeeded, $MAP_FAILED failed\""])
            .env("MAP_SUCCESSFUL", "13")
            .env("MAP_FAILED", "0")
            .env("MAP_TOTAL", "13")
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());
        assert!(output.stdout.contains("Processed 13 items"));
        assert!(output.stdout.contains("13 succeeded"));
    }
}
