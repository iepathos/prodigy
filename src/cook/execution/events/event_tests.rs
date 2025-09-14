//! Tests for event logging system

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::cook::execution::mapreduce::MapReduceConfig;
    use chrono::Utc;
    use std::collections::HashMap;
    
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::Mutex;
    use uuid::Uuid;

    /// Mock event writer for testing
    struct MockEventWriter {
        events: Arc<Mutex<Vec<EventRecord>>>,
    }

    impl MockEventWriter {
        fn new() -> Self {
            Self {
                events: Arc::new(Mutex::new(Vec::new())),
            }
        }

        #[allow(dead_code)]
        async fn get_events(&self) -> Vec<EventRecord> {
            self.events.lock().await.clone()
        }
    }

    #[async_trait::async_trait]
    impl EventWriter for MockEventWriter {
        async fn write(&self, events: &[EventRecord]) -> anyhow::Result<()> {
            let mut stored = self.events.lock().await;
            stored.extend_from_slice(events);
            Ok(())
        }

        async fn flush(&self) -> anyhow::Result<()> {
            Ok(())
        }

        fn clone(&self) -> Box<dyn EventWriter> {
            Box::new(Self {
                events: Arc::clone(&self.events),
            })
        }
    }

    #[tokio::test]
    async fn test_event_logger_logs_events() {
        let mock_writer = MockEventWriter::new();
        let events_ref = Arc::clone(&mock_writer.events);

        let logger = EventLogger::new(vec![Box::new(mock_writer)]);

        // Log a job started event
        let job_id = "test-job-123";
        let event = MapReduceEvent::JobStarted {
            job_id: job_id.to_string(),
            config: MapReduceConfig {
                input: "test.json".to_string(),
                json_path: "$.items".to_string(),
                max_parallel: 5,
                timeout_per_agent: 300,
                retry_on_failure: 2,
                max_items: None,
                offset: None,
            },
            total_items: 10,
            timestamp: Utc::now(),
        };

        logger.log(event.clone()).await.unwrap();
        logger.flush().await.unwrap();

        // Verify event was written
        let stored_events = events_ref.lock().await;
        assert_eq!(stored_events.len(), 1);
        assert_eq!(stored_events[0].event.job_id(), job_id);
    }

    #[tokio::test]
    async fn test_event_logger_with_metadata() {
        let mock_writer = MockEventWriter::new();
        let events_ref = Arc::clone(&mock_writer.events);

        let logger = EventLogger::new(vec![Box::new(mock_writer)]);

        // Create event with metadata
        let event = MapReduceEvent::AgentStarted {
            job_id: "test-job".to_string(),
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            worktree: "worktree-1".to_string(),
            attempt: 1,
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "custom_field".to_string(),
            serde_json::Value::String("custom_value".to_string()),
        );

        logger
            .log_with_metadata(event, metadata.clone())
            .await
            .unwrap();
        logger.flush().await.unwrap();

        // Verify metadata was included
        let stored_events = events_ref.lock().await;
        assert_eq!(stored_events.len(), 1);
        assert!(stored_events[0].metadata.contains_key("custom_field"));
    }

    #[tokio::test]
    async fn test_event_buffering() {
        let mock_writer = MockEventWriter::new();
        let events_ref = Arc::clone(&mock_writer.events);

        // Create logger with small buffer size
        let logger =
            EventLogger::with_config(vec![Box::new(mock_writer)], chrono::Duration::seconds(5), 2);

        // Log multiple events
        for i in 0..3 {
            let event = MapReduceEvent::AgentProgress {
                job_id: "test-job".to_string(),
                agent_id: format!("agent-{}", i),
                step: "processing".to_string(),
                progress_pct: (i as f32) * 33.3,
            };
            logger.log(event).await.unwrap();
        }

        // Events should be auto-flushed after buffer limit
        let stored_events = events_ref.lock().await;
        assert!(stored_events.len() >= 2); // At least 2 should be flushed
    }

    #[tokio::test]
    async fn test_event_correlation_id() {
        let logger = EventLogger::new(vec![]);

        // Check default correlation ID is set
        let initial_id = logger.current_correlation_id().await;
        assert!(!initial_id.is_empty());

        // Set custom correlation ID
        let custom_id = "custom-correlation-123";
        logger.set_correlation_id(custom_id.to_string()).await;

        let updated_id = logger.current_correlation_id().await;
        assert_eq!(updated_id, custom_id);
    }

    #[tokio::test]
    async fn test_event_severity_categorization() {
        // Test error severity
        let error_event = MapReduceEvent::JobFailed {
            job_id: "job-1".to_string(),
            error: "Test error".to_string(),
            partial_results: 5,
        };
        assert_eq!(error_event.severity(), EventSeverity::Error);

        // Test warning severity
        let warning_event = MapReduceEvent::MemoryPressure {
            job_id: "job-1".to_string(),
            used_mb: 900,
            limit_mb: 1000,
        };
        assert_eq!(warning_event.severity(), EventSeverity::Warning);

        // Test info severity
        let info_event = MapReduceEvent::JobCompleted {
            job_id: "job-1".to_string(),
            duration: chrono::Duration::seconds(60),
            success_count: 10,
            failure_count: 0,
        };
        assert_eq!(info_event.severity(), EventSeverity::Info);
    }

    #[tokio::test]
    async fn test_event_category_classification() {
        // Test job lifecycle category
        let job_event = MapReduceEvent::JobStarted {
            job_id: "job-1".to_string(),
            config: MapReduceConfig {
                input: "test.json".to_string(),
                json_path: "$.items".to_string(),
                max_parallel: 5,
                timeout_per_agent: 300,
                retry_on_failure: 2,
                max_items: None,
                offset: None,
            },
            total_items: 10,
            timestamp: Utc::now(),
        };
        assert_eq!(job_event.category(), EventCategory::JobLifecycle);

        // Test agent lifecycle category
        let agent_event = MapReduceEvent::AgentCompleted {
            job_id: "job-1".to_string(),
            agent_id: "agent-1".to_string(),
            duration: chrono::Duration::seconds(30),
            commits: vec!["abc123".to_string()],
        };
        assert_eq!(agent_event.category(), EventCategory::AgentLifecycle);

        // Test checkpoint category
        let checkpoint_event = MapReduceEvent::CheckpointCreated {
            job_id: "job-1".to_string(),
            version: 1,
            agents_completed: 5,
        };
        assert_eq!(checkpoint_event.category(), EventCategory::Checkpoint);
    }

    #[tokio::test]
    async fn test_jsonl_file_writer() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_events.jsonl");

        let writer = JsonlEventWriter::new(file_path.clone()).await.unwrap();

        // Create test events
        let events = vec![
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                correlation_id: "test-correlation".to_string(),
                event: MapReduceEvent::JobStarted {
                    job_id: "job-1".to_string(),
                    config: MapReduceConfig {
                        input: "test.json".to_string(),
                        json_path: "$.items".to_string(),
                        max_parallel: 5,
                        timeout_per_agent: 300,
                        retry_on_failure: 2,
                        max_items: None,
                        offset: None,
                    },
                    total_items: 10,
                    timestamp: Utc::now(),
                },
                metadata: HashMap::new(),
            },
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: Utc::now(),
                correlation_id: "test-correlation".to_string(),
                event: MapReduceEvent::JobCompleted {
                    job_id: "job-1".to_string(),
                    duration: chrono::Duration::seconds(60),
                    success_count: 10,
                    failure_count: 0,
                },
                metadata: HashMap::new(),
            },
        ];

        writer.write(&events).await.unwrap();
        writer.flush().await.unwrap();

        // Verify file was written
        assert!(file_path.exists());

        // Read and verify content
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        // Verify JSON structure
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed["id"].is_string());
            assert!(parsed["timestamp"].is_string());
            assert!(parsed["correlation_id"].is_string());
            assert!(parsed["event"].is_object());
        }
    }

    #[tokio::test]
    async fn test_file_event_writer_creates_directory() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join(".prodigy").join("mapreduce");
        let job_id = "test-job-456";

        let writer = FileEventWriter::new(base_path.clone(), job_id.to_string())
            .await
            .unwrap();

        let event = EventRecord {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            correlation_id: "test".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: job_id.to_string(),
                config: MapReduceConfig {
                    input: "test.json".to_string(),
                    json_path: "$.items".to_string(),
                    max_parallel: 5,
                    timeout_per_agent: 300,
                    retry_on_failure: 2,
                    max_items: None,
                    offset: None,
                },
                total_items: 5,
                timestamp: Utc::now(),
            },
            metadata: HashMap::new(),
        };

        writer.write(&[event]).await.unwrap();
        writer.flush().await.unwrap();

        // Verify directory structure was created
        let events_dir = base_path.join("events").join(job_id);
        assert!(events_dir.exists());
        assert!(events_dir.is_dir());
    }
}
