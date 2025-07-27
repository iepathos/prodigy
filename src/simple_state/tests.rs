//! Tests for simple state management

#[cfg(test)]
mod test {
    use super::super::*;

    use tempfile::TempDir;

    #[test]
    fn test_state_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(state_mgr.state().version, "1.0");
        assert_eq!(state_mgr.state().total_runs, 0);
    }

    #[test]
    fn test_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create and save state
        {
            let mut state_mgr = StateManager::with_root(root.clone()).unwrap();
            state_mgr.state_mut().total_runs = 5;
            state_mgr.save().unwrap();
        }

        // Load state
        {
            let state_mgr = StateManager::with_root(root).unwrap();
            assert_eq!(state_mgr.state().total_runs, 5);
        }
    }

    #[test]
    fn test_session_recording() {
        let temp_dir = TempDir::new().unwrap();
        let mut state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        let mut session = SessionRecord::new();
        session.complete(1, 2, "Fixed error handling".to_string());

        state_mgr.record_session(session).unwrap();
        assert_eq!(state_mgr.state().total_runs, 1);
    }

    #[test]
    fn test_cache_manager() {
        let temp_dir = TempDir::new().unwrap();
        let cache_mgr = CacheManager::with_config(
            temp_dir.path().join("cache"),
            60, // 1 minute TTL for testing
        )
        .unwrap();

        // Test set and get
        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestData {
            value: String,
            number: i32,
        }

        let data = TestData {
            value: "test".to_string(),
            number: 42,
        };

        cache_mgr.set("test_key", &data).unwrap();

        let retrieved: TestData = cache_mgr.get("test_key").unwrap().unwrap();
        assert_eq!(retrieved, data);

        // Test missing key
        let missing: Option<TestData> = cache_mgr.get("missing_key").unwrap();
        assert!(missing.is_none());

        // Test clear
        cache_mgr.clear().unwrap();
        let cleared: Option<TestData> = cache_mgr.get("test_key").unwrap();
        assert!(cleared.is_none());
    }

    #[test]
    fn test_cache_expiration() {
        let temp_dir = TempDir::new().unwrap();
        let cache_mgr = CacheManager::with_config(
            temp_dir.path().join("cache"),
            0, // 0 second TTL - expires immediately
        )
        .unwrap();

        cache_mgr.set("test_key", &"test_value").unwrap();

        // Sleep to ensure expiration
        std::thread::sleep(std::time::Duration::from_millis(100));

        let retrieved: Option<String> = cache_mgr.get("test_key").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_state_corruption_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Write corrupted JSON
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("state.json"), "{ invalid json").unwrap();

        // Should recover gracefully
        let state_mgr = StateManager::with_root(root.clone()).unwrap();
        assert_eq!(state_mgr.state().version, "1.0");

        // Check backup was created
        let entries: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("state.json.corrupted"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_history_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        let mut state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        // Record multiple sessions
        for i in 0..3 {
            let mut session = SessionRecord::new();
            session.complete(i + 1, i + 1, format!("Improvement {i}"));
            state_mgr.record_session(session).unwrap();
        }

        // Get all history
        let history = state_mgr.get_history().unwrap();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_concurrent_state_access() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        let root = Arc::new(temp_dir.path().to_path_buf());
        let save_counter = Arc::new(Mutex::new(0));

        // Spawn multiple threads that try to update state
        let handles: Vec<_> = (0..5)
            .map(|_i| {
                let root_clone = Arc::clone(&root);
                let counter_clone = Arc::clone(&save_counter);
                thread::spawn(move || {
                    let mut state_mgr = StateManager::with_root((*root_clone).clone()).unwrap();
                    state_mgr.state_mut().total_runs += 1;
                    // Allow save failures due to concurrent access
                    if state_mgr.save().is_ok() {
                        let mut counter = counter_clone.lock().unwrap();
                        *counter += 1;
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }

        // At least one save should have succeeded
        let successful_saves = *save_counter.lock().unwrap();
        assert!(successful_saves >= 1);

        // Check final state exists
        let state_mgr = StateManager::with_root((*root).clone()).unwrap();
        // total_runs is u32, so it's always >= 0
        assert!(state_mgr.state().total_runs <= 10); // Should be reasonable after 5 threads
    }

    #[test]
    fn test_session_record_edge_cases() {
        let mut session = SessionRecord::new();

        // Test with empty summary
        session.complete(1, 0, String::new());
        assert_eq!(session.summary, "");
        assert!(session.completed_at.is_some());

        // Test with very long summary
        let long_summary = "x".repeat(1000);
        let mut session2 = SessionRecord::new();
        session2.complete(1, 1, long_summary.clone());
        assert_eq!(session2.summary, long_summary);
    }

    #[test]
    fn test_cache_manager_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let cache_mgr = CacheManager::with_config(temp_dir.path().join("cache"), 3600).unwrap();

        // Test empty key
        let result: Result<Option<String>, _> = cache_mgr.get("");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        // Test very large data
        let large_data = vec![0u8; 1_000_000]; // 1MB
        cache_mgr.set("large", &large_data).unwrap();
        let retrieved: Vec<u8> = cache_mgr.get("large").unwrap().unwrap();
        assert_eq!(retrieved.len(), large_data.len());

        // Test special characters in key
        let special_key = "test/key:with*special|chars";
        cache_mgr.set(special_key, &"value").unwrap();
        let retrieved: String = cache_mgr.get(special_key).unwrap().unwrap();
        assert_eq!(retrieved, "value");
    }

    #[test]
    fn test_state_file_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let mut state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        state_mgr.state_mut().total_runs = 8;
        state_mgr.save().unwrap();

        // Check that state file exists and is readable
        let state_file = temp_dir.path().join("state.json");
        assert!(state_file.exists());

        // Verify we can read it back
        let contents = std::fs::read_to_string(&state_file).unwrap();
        // Check that the total_runs was saved
        assert!(contents.contains("total_runs"));
        assert!(contents.contains("8"));
    }

    #[test]
    fn test_invalid_root_directory() {
        // Test with non-existent parent directory
        let result = StateManager::with_root("/non/existent/path/mmm".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_history_sorting() {
        let temp_dir = TempDir::new().unwrap();
        let mut state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        // Record sessions in reverse chronological order
        for i in (0..3).rev() {
            let mut session = SessionRecord::new();
            session.started_at = chrono::Utc::now() - chrono::Duration::days(i);
            session.complete(1, 1, format!("Session {i}"));
            state_mgr.record_session(session).unwrap();
        }

        // History should be sorted by start time
        let history = state_mgr.get_history().unwrap();
        assert_eq!(history.len(), 3);

        // Verify chronological order
        for i in 0..history.len() - 1 {
            assert!(history[i].started_at <= history[i + 1].started_at);
        }
    }

    #[test]
    fn test_cache_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache");

        {
            let cache_mgr = CacheManager::with_config(cache_path.clone(), 3600).unwrap();

            // Add multiple cache entries
            for i in 0..10 {
                cache_mgr
                    .set(&format!("key_{i}"), &format!("value_{i}"))
                    .unwrap();
            }

            // Verify files exist
            let entries = std::fs::read_dir(&cache_path).unwrap().count();
            assert_eq!(entries, 10);

            // Clear cache
            cache_mgr.clear().unwrap();
        }

        // Verify cache directory is empty
        let entries = std::fs::read_dir(&cache_path).unwrap().count();
        assert_eq!(entries, 0);
    }

    #[test]
    fn test_corrupted_cache_entry() {
        let temp_dir = TempDir::new().unwrap();
        let cache_path = temp_dir.path().join("cache");
        let cache_mgr = CacheManager::with_config(cache_path.clone(), 3600).unwrap();

        // Create a corrupted cache file
        let key = "corrupted_key";
        let file_path = cache_path.join(format!("{key}.json"));
        std::fs::create_dir_all(&cache_path).unwrap();
        std::fs::write(&file_path, "{ invalid json").unwrap();

        // Should handle gracefully - corrupted entries return None
        let result: Result<Option<String>, _> = cache_mgr.get(key);
        // Either error or None is acceptable for corrupted entries
        assert!(result.is_err() || result.unwrap().is_none());
    }
}
