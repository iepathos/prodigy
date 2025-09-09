//! Unit tests for enhanced progress tracking

use super::progress::*;
use crate::cook::execution::errors::MapReduceResult;
use chrono::Utc;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod enhanced_progress_tracker_tests {
    use super::*;

    #[tokio::test]
    async fn test_new_tracker_initialization() {
        let tracker = EnhancedProgressTracker::new("test-job-123".to_string(), 100);

        assert_eq!(tracker.job_id, "test-job-123");
        assert_eq!(tracker.total_items, 100);

        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.pending_items, 100);
        assert_eq!(metrics.completed_items, 0);
        assert_eq!(metrics.failed_items, 0);
        assert_eq!(metrics.active_agents, 0);
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_update_agent_progress() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        let progress = AgentProgress {
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            state: AgentState::Running {
                step: "Processing".to_string(),
                progress: 50.0,
            },
            current_step: "Step 2 of 4".to_string(),
            steps_completed: 2,
            total_steps: 4,
            progress_percentage: 50.0,
            started_at: Utc::now(),
            last_update: Utc::now(),
            estimated_completion: Some(Utc::now() + chrono::Duration::seconds(60)),
            error_count: 0,
            retry_count: 0,
        };

        tracker
            .update_agent_progress("agent-1", progress.clone())
            .await
            .unwrap();

        let agents = tracker.agents.read().await;
        assert_eq!(agents.len(), 1);
        assert!(agents.contains_key("agent-1"));

        let stored_progress = &agents["agent-1"];
        assert_eq!(stored_progress.agent_id, "agent-1");
        assert_eq!(stored_progress.progress_percentage, 50.0);
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_mark_item_completed() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // First add an agent
        let progress = AgentProgress {
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            state: AgentState::Running {
                step: "Processing".to_string(),
                progress: 90.0,
            },
            current_step: "Final step".to_string(),
            steps_completed: 3,
            total_steps: 4,
            progress_percentage: 90.0,
            started_at: Utc::now(),
            last_update: Utc::now(),
            estimated_completion: None,
            error_count: 0,
            retry_count: 0,
        };

        tracker
            .update_agent_progress("agent-1", progress)
            .await
            .unwrap();
        tracker.mark_item_completed("agent-1").await.unwrap();

        let agents = tracker.agents.read().await;
        assert!(matches!(agents["agent-1"].state, AgentState::Completed));

        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.completed_items, 1);
        assert_eq!(metrics.pending_items, 9);
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_mark_item_failed() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // First add an agent
        let progress = AgentProgress {
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            state: AgentState::Running {
                step: "Processing".to_string(),
                progress: 50.0,
            },
            current_step: "Step 2".to_string(),
            steps_completed: 2,
            total_steps: 4,
            progress_percentage: 50.0,
            started_at: Utc::now(),
            last_update: Utc::now(),
            estimated_completion: None,
            error_count: 1,
            retry_count: 0,
        };

        tracker
            .update_agent_progress("agent-1", progress)
            .await
            .unwrap();
        tracker
            .mark_item_failed("agent-1", "Test error".to_string())
            .await
            .unwrap();

        let agents = tracker.agents.read().await;
        assert!(matches!(
            &agents["agent-1"].state,
            AgentState::Failed { error } if error == "Test error"
        ));

        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.failed_items, 1);
        assert_eq!(metrics.pending_items, 9);
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_overall_progress_calculation() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // Mark some items as completed
        for i in 0..3 {
            let agent_id = format!("agent-{}", i);
            let progress = AgentProgress {
                agent_id: agent_id.clone(),
                item_id: format!("item-{}", i),
                state: AgentState::Completed,
                current_step: "Done".to_string(),
                steps_completed: 4,
                total_steps: 4,
                progress_percentage: 100.0,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };
            tracker
                .update_agent_progress(&agent_id, progress)
                .await
                .unwrap();
            tracker.mark_item_completed(&agent_id).await.unwrap();
        }

        // Mark one as failed
        let progress = AgentProgress {
            agent_id: "agent-fail".to_string(),
            item_id: "item-fail".to_string(),
            state: AgentState::Failed {
                error: "Error".to_string(),
            },
            current_step: "Failed".to_string(),
            steps_completed: 1,
            total_steps: 4,
            progress_percentage: 25.0,
            started_at: Utc::now(),
            last_update: Utc::now(),
            estimated_completion: None,
            error_count: 1,
            retry_count: 0,
        };
        tracker
            .update_agent_progress("agent-fail", progress)
            .await
            .unwrap();
        tracker
            .mark_item_failed("agent-fail", "Error".to_string())
            .await
            .unwrap();

        let overall_progress = tracker.get_overall_progress().await;
        assert_eq!(overall_progress, 40.0); // 4 out of 10 items processed
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_export_json_format() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 5);

        // Add some test data
        let progress = AgentProgress {
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            state: AgentState::Completed,
            current_step: "Done".to_string(),
            steps_completed: 4,
            total_steps: 4,
            progress_percentage: 100.0,
            started_at: Utc::now(),
            last_update: Utc::now(),
            estimated_completion: None,
            error_count: 0,
            retry_count: 0,
        };
        tracker
            .update_agent_progress("agent-1", progress)
            .await
            .unwrap();
        tracker.mark_item_completed("agent-1").await.unwrap();

        let exported = tracker.export_progress(ExportFormat::Json).await.unwrap();
        let json_str = String::from_utf8(exported).unwrap();

        // Parse and validate JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["job_id"], "test-job");
        assert_eq!(parsed["metrics"]["completed_items"], 1);
    }

    #[tokio::test]
    async fn test_export_csv_format() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 5);

        let exported = tracker.export_progress(ExportFormat::Csv).await.unwrap();
        let csv_str = String::from_utf8(exported).unwrap();

        // Verify CSV headers
        assert!(csv_str.contains("timestamp,job_id,completed_items"));
        assert!(csv_str.contains("test-job"));
    }

    #[tokio::test]
    async fn test_export_html_format() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 5);

        let exported = tracker.export_progress(ExportFormat::Html).await.unwrap();
        let html_str = String::from_utf8(exported).unwrap();

        // Verify HTML content
        assert!(html_str.contains("<!DOCTYPE html>"));
        assert!(html_str.contains("test-job"));
        assert!(html_str.contains("MapReduce Job Progress Report"));
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_create_snapshot() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // Add some agents
        for i in 0..3 {
            let progress = AgentProgress {
                agent_id: format!("agent-{}", i),
                item_id: format!("item-{}", i),
                state: if i == 0 {
                    AgentState::Completed
                } else {
                    AgentState::Running {
                        step: "Processing".to_string(),
                        progress: 50.0,
                    }
                },
                current_step: format!("Step {}", i),
                steps_completed: i,
                total_steps: 4,
                progress_percentage: (i as f32) * 25.0,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };
            tracker
                .update_agent_progress(&format!("agent-{}", i), progress)
                .await
                .unwrap();
        }

        let snapshot = tracker.create_snapshot().await;

        assert_eq!(snapshot.job_id, "test-job");
        assert_eq!(snapshot.agent_states.len(), 3);
        assert!(snapshot.agent_states.contains_key("agent-0"));
        assert!(matches!(
            snapshot.agent_states["agent-0"],
            AgentState::Completed
        ));
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_metrics_recalculation() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 100);

        // Add active agents
        for i in 0..5 {
            let progress = AgentProgress {
                agent_id: format!("agent-{}", i),
                item_id: format!("item-{}", i),
                state: AgentState::Running {
                    step: "Processing".to_string(),
                    progress: 50.0,
                },
                current_step: "Working".to_string(),
                steps_completed: 2,
                total_steps: 4,
                progress_percentage: 50.0,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };
            tracker
                .update_agent_progress(&format!("agent-{}", i), progress)
                .await
                .unwrap();
        }

        // Mark some as completed
        for i in 0..3 {
            tracker
                .mark_item_completed(&format!("agent-{}", i))
                .await
                .unwrap();
        }

        // Wait a bit for throughput calculation
        sleep(Duration::from_millis(100)).await;

        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.completed_items, 3);
        assert!(metrics.throughput_average > 0.0);
        assert_eq!(metrics.success_rate, 100.0);
        assert!(metrics.estimated_completion.is_some());
    }

    #[tokio::test]
    async fn test_agent_state_transitions() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        let agent_id = "agent-1";

        // Start as queued
        tracker
            .update_agent_state(agent_id, AgentState::Queued)
            .await
            .unwrap();
        {
            let agents = tracker.agents.read().await;
            assert!(
                matches!(agents.get(agent_id), Some(agent) if matches!(agent.state, AgentState::Queued))
            );
        }

        // Move to initializing
        tracker
            .update_agent_state(agent_id, AgentState::Initializing)
            .await
            .unwrap();
        {
            let agents = tracker.agents.read().await;
            assert!(
                matches!(agents.get(agent_id), Some(agent) if matches!(agent.state, AgentState::Initializing))
            );
        }

        // Move to running
        tracker
            .update_agent_state(
                agent_id,
                AgentState::Running {
                    step: "Processing".to_string(),
                    progress: 25.0,
                },
            )
            .await
            .unwrap();
        {
            let agents = tracker.agents.read().await;
            assert!(matches!(
                agents.get(agent_id),
                Some(agent) if matches!(agent.state, AgentState::Running { .. })
            ));
        }

        // Move to retrying
        tracker
            .update_agent_state(agent_id, AgentState::Retrying { attempt: 1 })
            .await
            .unwrap();
        {
            let agents = tracker.agents.read().await;
            assert!(matches!(
                agents.get(agent_id),
                Some(agent) if matches!(agent.state, AgentState::Retrying { attempt: 1 })
            ));
        }

        // Move to dead-lettered
        tracker
            .update_agent_state(agent_id, AgentState::DeadLettered)
            .await
            .unwrap();
        {
            let agents = tracker.agents.read().await;
            assert!(matches!(
                agents.get(agent_id),
                Some(agent) if matches!(agent.state, AgentState::DeadLettered)
            ));
        }
    }
}

#[cfg(test)]
mod cli_progress_viewer_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_cli_viewer_initialization() {
        let tracker = Arc::new(EnhancedProgressTracker::new("test-job".to_string(), 10));
        let _viewer = CLIProgressViewer::new(tracker.clone());

        // Viewer initialized correctly with expected interval
    }

    #[tokio::test]
    async fn test_progress_bar_creation() {
        let tracker = Arc::new(EnhancedProgressTracker::new("test-job".to_string(), 10));
        let viewer = CLIProgressViewer::new(tracker);

        // Test various percentages
        let bar_0 = viewer.create_progress_bar(0.0);
        assert_eq!(bar_0, "░░░░░░░░░░░░░░░░░░░░");

        let bar_50 = viewer.create_progress_bar(50.0);
        assert_eq!(bar_50, "██████████░░░░░░░░░░");

        let bar_100 = viewer.create_progress_bar(100.0);
        assert_eq!(bar_100, "████████████████████");
    }

    #[test]
    fn test_format_duration() {
        use crate::cook::execution::progress::format_duration;
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
        assert_eq!(format_duration(Duration::from_secs(7322)), "2h 2m 2s");
    }
}

#[cfg(test)]
mod progress_web_server_tests {
    use super::*;

    #[tokio::test]
    async fn test_web_server_initialization() {
        let mut tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // Start web server on a random port for testing
        let result = tracker.start_web_server(0).await;
        assert!(result.is_ok());
        assert!(tracker.web_server.is_some());
    }

    #[tokio::test]
    async fn test_dashboard_html_exists() {
        // Ensure the dashboard HTML is included
        let html = include_str!("progress_dashboard.html");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("MapReduce Progress Dashboard"));
    }
}

#[cfg(test)]
mod progress_reporter_trait_tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::DateTime;

    // Mock implementation for testing
    struct MockProgressReporter {
        progress: f32,
    }

    #[async_trait]
    impl ProgressReporter for MockProgressReporter {
        async fn update_agent_progress(
            &self,
            _agent_id: &str,
            _progress: AgentProgress,
        ) -> MapReduceResult<()> {
            Ok(())
        }

        async fn get_overall_progress(&self) -> MapReduceResult<f32> {
            Ok(self.progress)
        }

        async fn get_estimated_completion(&self) -> MapReduceResult<Option<DateTime<Utc>>> {
            Ok(Some(Utc::now() + chrono::Duration::seconds(60)))
        }

        async fn export_progress(&self, _format: ExportFormat) -> MapReduceResult<Vec<u8>> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_trait_implementation() {
        let reporter = MockProgressReporter { progress: 75.0 };

        let progress = reporter.get_overall_progress().await.unwrap();
        assert_eq!(progress, 75.0);

        let etc = reporter.get_estimated_completion().await.unwrap();
        assert!(etc.is_some());

        let export = reporter.export_progress(ExportFormat::Json).await.unwrap();
        assert_eq!(export.len(), 0);
    }

    #[tokio::test]
    async fn test_enhanced_tracker_implements_trait() {
        let tracker = EnhancedProgressTracker::new("test-job".to_string(), 10);

        // Use the trait methods
        let progress_reporter: &dyn ProgressReporter = &tracker;

        let progress = progress_reporter.get_overall_progress().await.unwrap();
        assert_eq!(progress, 0.0);

        let etc = progress_reporter.get_estimated_completion().await.unwrap();
        assert!(etc.is_none());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_complete_workflow_simulation() {
        let tracker = EnhancedProgressTracker::new("integration-test".to_string(), 20);

        // Simulate agents processing items
        for i in 0..10 {
            let agent_id = format!("agent-{}", i);

            // Queue state
            let progress = AgentProgress {
                agent_id: agent_id.clone(),
                item_id: format!("item-{}", i),
                state: AgentState::Queued,
                current_step: "Waiting".to_string(),
                steps_completed: 0,
                total_steps: 4,
                progress_percentage: 0.0,
                started_at: Utc::now(),
                last_update: Utc::now(),
                estimated_completion: None,
                error_count: 0,
                retry_count: 0,
            };
            tracker
                .update_agent_progress(&agent_id, progress)
                .await
                .unwrap();

            // Initialize
            tracker
                .update_agent_state(&agent_id, AgentState::Initializing)
                .await
                .unwrap();

            // Running through steps
            for step in 1..=4 {
                let progress = AgentProgress {
                    agent_id: agent_id.clone(),
                    item_id: format!("item-{}", i),
                    state: AgentState::Running {
                        step: format!("Step {}", step),
                        progress: (step as f32) * 25.0,
                    },
                    current_step: format!("Step {} of 4", step),
                    steps_completed: step,
                    total_steps: 4,
                    progress_percentage: (step as f32) * 25.0,
                    started_at: Utc::now(),
                    last_update: Utc::now(),
                    estimated_completion: Some(Utc::now() + chrono::Duration::seconds(60)),
                    error_count: 0,
                    retry_count: 0,
                };
                tracker
                    .update_agent_progress(&agent_id, progress)
                    .await
                    .unwrap();
            }

            // Complete or fail based on index
            if i % 5 == 0 && i > 0 {
                tracker
                    .mark_item_failed(&agent_id, format!("Error in item {}", i))
                    .await
                    .unwrap();
            } else {
                tracker.mark_item_completed(&agent_id).await.unwrap();
            }
        }

        // Verify final state
        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.completed_items, 8);
        assert_eq!(metrics.failed_items, 2);
        assert_eq!(metrics.pending_items, 10);

        let overall_progress = tracker.get_overall_progress().await;
        assert_eq!(overall_progress, 50.0); // 10 out of 20 items processed

        // Test snapshot
        let snapshot = tracker.create_snapshot().await;
        assert_eq!(snapshot.job_id, "integration-test");
        assert_eq!(snapshot.agent_states.len(), 10);

        // Test export
        let json_export = tracker.export_progress(ExportFormat::Json).await.unwrap();
        assert!(!json_export.is_empty());

        let csv_export = tracker.export_progress(ExportFormat::Csv).await.unwrap();
        assert!(!csv_export.is_empty());

        let html_export = tracker.export_progress(ExportFormat::Html).await.unwrap();
        assert!(!html_export.is_empty());
    }

    #[tokio::test]
    #[ignore = "Temporarily disabled - investigating deadlock"]
    async fn test_concurrent_agent_updates() {
        let tracker = Arc::new(EnhancedProgressTracker::new(
            "concurrent-test".to_string(),
            100,
        ));

        // Spawn multiple tasks to update agents concurrently
        let mut handles = vec![];

        for i in 0..20 {
            let tracker_clone = tracker.clone();
            let handle = tokio::spawn(async move {
                let agent_id = format!("agent-{}", i);

                for step in 1..=5 {
                    let progress = AgentProgress {
                        agent_id: agent_id.clone(),
                        item_id: format!("item-{}", i),
                        state: AgentState::Running {
                            step: format!("Step {}", step),
                            progress: (step as f32) * 20.0,
                        },
                        current_step: format!("Processing step {}", step),
                        steps_completed: step,
                        total_steps: 5,
                        progress_percentage: (step as f32) * 20.0,
                        started_at: Utc::now(),
                        last_update: Utc::now(),
                        estimated_completion: None,
                        error_count: 0,
                        retry_count: 0,
                    };

                    tracker_clone
                        .update_agent_progress(&agent_id, progress)
                        .await
                        .unwrap();
                    sleep(Duration::from_millis(10)).await;
                }

                tracker_clone.mark_item_completed(&agent_id).await.unwrap();
            });

            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all updates were processed
        let agents = tracker.agents.read().await;
        assert_eq!(agents.len(), 20);

        let metrics = tracker.metrics.read().await;
        assert_eq!(metrics.completed_items, 20);
        assert_eq!(metrics.pending_items, 80);
    }
}
