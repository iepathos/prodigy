#[cfg(test)]
mod tests {
    use super::super::errors::*;
    use std::path::PathBuf;

    #[test]
    fn test_error_display_messages() {
        let error = MapReduceError::JobInitializationFailed {
            job_id: "job123".to_string(),
            reason: "config invalid".to_string(),
            source: None,
        };
        assert_eq!(
            error.to_string(),
            "Job job123 initialization failed: config invalid"
        );

        let error = MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
            job_id: "job456".to_string(),
            agent_id: "agent1".to_string(),
            item_id: "item1".to_string(),
            duration_secs: 30,
            last_operation: "processing".to_string(),
        }));
        assert_eq!(
            error.to_string(),
            "Agent timeout: Agent agent1 timeout after 30s"
        );
    }

    #[test]
    fn test_retryable_errors() {
        let timeout = MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            item_id: "item1".to_string(),
            duration_secs: 60,
            last_operation: "processing".to_string(),
        }));
        assert!(timeout.is_retryable());

        let resource = MapReduceError::ResourceExhausted(Box::new(ResourceExhaustedError {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            resource: ResourceType::Memory,
            limit: "1GB".to_string(),
            usage: "1.2GB".to_string(),
        }));
        assert!(resource.is_retryable());

        let worktree = MapReduceError::WorktreeCreationFailed {
            agent_id: "agent1".to_string(),
            reason: "disk full".to_string(),
            source: std::io::Error::other("disk full"),
        };
        assert!(worktree.is_retryable());

        let checkpoint = MapReduceError::CheckpointPersistFailed {
            job_id: "job1".to_string(),
            path: PathBuf::from("/tmp/checkpoint"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
        };
        assert!(checkpoint.is_retryable());

        let config = MapReduceError::InvalidConfiguration {
            reason: "bad value".to_string(),
            field: "timeout".to_string(),
            value: "-1".to_string(),
        };
        assert!(!config.is_retryable());

        let not_found = MapReduceError::JobNotFound {
            job_id: "missing".to_string(),
        };
        assert!(!not_found.is_retryable());
    }

    #[test]
    fn test_recovery_hints() {
        let resource = MapReduceError::ResourceExhausted(Box::new(ResourceExhaustedError {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            resource: ResourceType::Memory,
            limit: "1GB".to_string(),
            usage: "1.2GB".to_string(),
        }));
        let hint = resource.recovery_hint();
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("Memory"));

        let merge = MapReduceError::WorktreeMergeConflict {
            agent_id: "agent1".to_string(),
            branch: "feature".to_string(),
            conflicts: vec!["file1.txt".to_string()],
        };
        let hint = merge.recovery_hint();
        assert_eq!(
            hint,
            Some("Manual conflict resolution required".to_string())
        );

        let subst = MapReduceError::ShellSubstitutionFailed {
            variable: "VAR1".to_string(),
            command: "echo $VAR1".to_string(),
            available_vars: vec!["VAR2".to_string(), "VAR3".to_string()],
        };
        let hint = subst.recovery_hint();
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("VAR1"));

        let timeout = MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            item_id: "item1".to_string(),
            duration_secs: 60,
            last_operation: "processing".to_string(),
        }));
        let hint = timeout.recovery_hint();
        assert!(hint.is_some());
        assert!(hint.unwrap().contains("timeout"));
    }

    #[test]
    fn test_variant_names() {
        let errors = vec![
            (
                MapReduceError::JobInitializationFailed {
                    job_id: "test".to_string(),
                    reason: "test".to_string(),
                    source: None,
                },
                "JobInitializationFailed",
            ),
            (
                MapReduceError::JobAlreadyExists {
                    job_id: "test".to_string(),
                },
                "JobAlreadyExists",
            ),
            (
                MapReduceError::JobNotFound {
                    job_id: "test".to_string(),
                },
                "JobNotFound",
            ),
            (
                MapReduceError::CheckpointCorrupted {
                    job_id: "test".to_string(),
                    version: 1,
                    details: "test".to_string(),
                },
                "CheckpointCorrupted",
            ),
            (
                MapReduceError::AgentFailed(Box::new(AgentFailedError {
                    job_id: "test".to_string(),
                    agent_id: "test".to_string(),
                    item_id: "test".to_string(),
                    reason: "test".to_string(),
                    worktree: None,
                    duration_ms: 0,
                    source: None,
                })),
                "AgentFailed",
            ),
            (
                MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
                    job_id: "test".to_string(),
                    agent_id: "test".to_string(),
                    item_id: "test".to_string(),
                    duration_secs: 0,
                    last_operation: "test".to_string(),
                })),
                "AgentTimeout",
            ),
        ];

        for (error, expected_name) in errors {
            assert_eq!(error.variant_name(), expected_name);
        }
    }

    #[test]
    fn test_aggregated_error() {
        let errors = vec![
            MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
                job_id: "job1".to_string(),
                agent_id: "agent1".to_string(),
                item_id: "item1".to_string(),
                duration_secs: 60,
                last_operation: "processing".to_string(),
            })),
            MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
                job_id: "job1".to_string(),
                agent_id: "agent2".to_string(),
                item_id: "item2".to_string(),
                duration_secs: 60,
                last_operation: "processing".to_string(),
            })),
            MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
                job_id: "job1".to_string(),
                agent_id: "agent3".to_string(),
                item_id: "item3".to_string(),
                duration_secs: 60,
                last_operation: "processing".to_string(),
            })),
            MapReduceError::JobNotFound {
                job_id: "job2".to_string(),
            },
        ];

        let aggregated = AggregatedError::new(errors);
        assert_eq!(aggregated.total_count, 4);
        assert_eq!(aggregated.most_common_error(), Some("AgentTimeout"));
        assert_eq!(*aggregated.by_type.get("AgentTimeout").unwrap(), 3);
        assert_eq!(*aggregated.by_type.get("JobNotFound").unwrap(), 1);

        let summary = aggregated.summary();
        assert!(summary.contains("Total errors: 4"));
        assert!(summary.contains("AgentTimeout: 3"));
        assert!(summary.contains("JobNotFound: 1"));
    }

    #[test]
    fn test_error_handler_trait() {
        let handler = DefaultErrorHandler;

        let timeout = MapReduceError::AgentTimeout(Box::new(AgentTimeoutError {
            job_id: "job1".to_string(),
            agent_id: "agent1".to_string(),
            item_id: "item1".to_string(),
            duration_secs: 60,
            last_operation: "processing".to_string(),
        }));
        assert!(handler.should_retry(&timeout));

        let action = handler.handle_error(&timeout);
        match action {
            ErrorAction::Retry { delay } => {
                assert_eq!(delay.as_secs(), 10);
            }
            _ => panic!("Expected Retry action"),
        }

        let config = MapReduceError::InvalidConfiguration {
            reason: "bad".to_string(),
            field: "field".to_string(),
            value: "value".to_string(),
        };
        assert!(!handler.should_retry(&config));

        let action = handler.handle_error(&config);
        match action {
            ErrorAction::Propagate => {}
            _ => panic!("Expected Propagate action"),
        }
    }

    #[test]
    fn test_from_anyhow_conversion() {
        let anyhow_err = anyhow::anyhow!("Test error");
        let mapreduce_err: MapReduceError = anyhow_err.into();

        match mapreduce_err {
            MapReduceError::General { message, source } => {
                assert_eq!(message, "Test error");
                assert!(source.is_none());
            }
            _ => panic!("Expected General error variant"),
        }
    }

    #[test]
    fn test_contextual_error() {
        use chrono::Utc;

        let error = MapReduceError::JobNotFound {
            job_id: "test".to_string(),
        };

        let context = ErrorContext {
            correlation_id: "corr123".to_string(),
            timestamp: Utc::now(),
            hostname: "host1".to_string(),
            thread_id: "thread1".to_string(),
            span_trace: vec![],
        };

        let contextual = error.with_context(context.clone());
        let display = contextual.to_string();
        assert!(display.contains("Job test not found"));
        assert!(display.contains("corr123"));
    }
}
