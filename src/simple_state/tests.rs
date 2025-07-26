//! Tests for simple state management

#[cfg(test)]
mod tests {
    use super::super::*;
    use chrono::Utc;
    use tempfile::TempDir;

    #[test]
    fn test_state_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(state_mgr.state().version, "1.0");
        assert_eq!(state_mgr.state().current_score, 0.0);
        assert_eq!(state_mgr.state().stats.total_runs, 0);
    }

    #[test]
    fn test_state_persistence() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create and save state
        {
            let mut state_mgr = StateManager::with_root(root.clone()).unwrap();
            state_mgr.state_mut().current_score = 7.5;
            state_mgr.state_mut().stats.total_runs = 5;
            state_mgr.save().unwrap();
        }

        // Load state
        {
            let state_mgr = StateManager::with_root(root).unwrap();
            assert_eq!(state_mgr.state().current_score, 7.5);
            assert_eq!(state_mgr.state().stats.total_runs, 5);
        }
    }

    #[test]
    fn test_session_recording() {
        let temp_dir = TempDir::new().unwrap();
        let mut state_mgr = StateManager::with_root(temp_dir.path().to_path_buf()).unwrap();

        let mut session = SessionRecord::new(7.0);
        session.improvements.push(Improvement {
            improvement_type: "error_handling".to_string(),
            file: "src/main.rs".to_string(),
            line: Some(42),
            description: "Replaced unwrap with ?".to_string(),
            impact: 0.2,
        });
        session.complete(7.2);

        state_mgr.record_session(session).unwrap();

        assert_eq!(state_mgr.state().current_score, 7.2);
        assert_eq!(state_mgr.state().stats.total_runs, 1);
        assert_eq!(state_mgr.state().stats.total_improvements, 1);
        assert!(state_mgr.state().stats.average_improvement > 0.0);
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
    fn test_learning_manager() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("learning.json");
        let mut learning_mgr = LearningManager::load_from(path).unwrap();

        // Record some improvements
        let improvement1 = Improvement {
            improvement_type: "error_handling".to_string(),
            file: "src/main.rs".to_string(),
            line: Some(10),
            description: "Added error handling".to_string(),
            impact: 0.3,
        };

        let improvement2 = Improvement {
            improvement_type: "error_handling".to_string(),
            file: "src/lib.rs".to_string(),
            line: Some(20),
            description: "Improved error messages".to_string(),
            impact: 0.2,
        };

        learning_mgr.record_improvement(&improvement1).unwrap();
        learning_mgr.record_improvement(&improvement2).unwrap();

        // Check pattern stats
        let stats = learning_mgr.get_pattern_stats("error_handling").unwrap();
        assert_eq!(stats.total_attempts, 2);
        assert_eq!(stats.successful, 2);
        assert_eq!(stats.success_rate, 1.0);
        assert_eq!(stats.average_impact, 0.25);

        // Test suggestions
        let suggestions = learning_mgr.suggest_improvements(5);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].0, "error_handling");
    }

    #[test]
    fn test_learning_failure_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("learning.json");
        let mut learning_mgr = LearningManager::load_from(path).unwrap();

        // Record multiple failures
        for _ in 0..5 {
            learning_mgr.record_failure("bad_pattern").unwrap();
        }

        // Check it's marked as failed
        let patterns_to_avoid = learning_mgr.patterns_to_avoid();
        assert!(patterns_to_avoid.contains(&"bad_pattern".to_string()));
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
            let mut session = SessionRecord::new(7.0 + i as f32 * 0.1);
            session.improvements.push(Improvement {
                improvement_type: "test".to_string(),
                file: format!("file{}.rs", i),
                line: Some(i),
                description: format!("Improvement {}", i),
                impact: 0.1,
            });
            session.complete(7.1 + i as f32 * 0.1);
            state_mgr.record_session(session).unwrap();
        }

        // Get all history
        let history = state_mgr.get_history(None).unwrap();
        assert_eq!(history.len(), 3);

        // Get today's history
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let today_history = state_mgr.get_history(Some(&today)).unwrap();
        assert_eq!(today_history.len(), 3);
    }

    #[test]
    fn test_cache_stats() {
        let temp_dir = TempDir::new().unwrap();
        let cache_mgr = CacheManager::with_config(temp_dir.path().join("cache"), 3600).unwrap();

        // Add some cache entries
        cache_mgr.set("key1", &"value1").unwrap();
        cache_mgr.set("key2", &"value2").unwrap();
        cache_mgr.set("key3", &"value3").unwrap();

        let stats = cache_mgr.stats().unwrap();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.valid_entries, 3);
        assert_eq!(stats.expired_entries, 0);
        assert!(stats.total_size_bytes > 0);
    }
}
