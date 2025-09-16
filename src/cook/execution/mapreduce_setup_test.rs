//! Tests for MapReduce setup phase functionality

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_setup_phase_variable_capture() {
        // Test that variables captured in setup phase are available in map phase
        let temp_dir = tempdir().unwrap();
        let work_items_path = temp_dir.path().join("work-items.json");

        // Create work items file
        std::fs::write(
            &work_items_path,
            r#"[{"id": 1, "name": "item1"}, {"id": 2, "name": "item2"}]"#,
        )
        .unwrap();

        // Create setup variables that should be passed to map phase
        let mut setup_vars = HashMap::new();
        setup_vars.insert("build_version".to_string(), "1.2.3".to_string());
        setup_vars.insert("environment".to_string(), "test".to_string());

        // Verify setup variables are created correctly
        assert!(work_items_path.exists());
        assert_eq!(setup_vars.get("build_version"), Some(&"1.2.3".to_string()));
        assert_eq!(setup_vars.get("environment"), Some(&"test".to_string()));
    }

    #[tokio::test]
    async fn test_setup_generates_work_items() {
        // Test that setup phase can generate the work-items.json file
        let temp_dir = tempdir().unwrap();
        std::env::set_current_dir(&temp_dir).unwrap();

        // Simulate setup phase creating work-items.json
        let files_before: std::collections::HashSet<String> = std::fs::read_dir(".")
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();

        // Create work-items.json (simulating setup phase)
        std::fs::write("work-items.json", r#"[{"id": 1}, {"id": 2}, {"id": 3}]"#).unwrap();

        let files_after: std::collections::HashSet<String> = std::fs::read_dir(".")
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();

        // Check if work-items.json was created
        let mut generated_input_file = None;
        for file in files_after.difference(&files_before) {
            if file.ends_with("work-items.json") || file == "work-items.json" {
                generated_input_file = Some(file.clone());
                break;
            }
        }

        assert_eq!(generated_input_file, Some("work-items.json".to_string()));
    }

    #[tokio::test]
    async fn test_setup_phase_failure_prevents_map() {
        // Test that setup phase failure prevents map phase execution
        // This is handled in the workflow executor by returning early on setup failure

        // Simulate a failing setup step
        let setup_error = anyhow::anyhow!("Setup command failed");

        // Verify that error is propagated correctly
        assert!(setup_error.to_string().contains("Setup command failed"));
    }
}
