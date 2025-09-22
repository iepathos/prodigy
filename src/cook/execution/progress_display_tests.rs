#[cfg(test)]
mod tests {
    use crate::cook::execution::progress_display::*;
    use crate::cook::execution::progress_tracker::*;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::RwLock;

    struct MockWriter {
        buffer: Arc<RwLock<Vec<u8>>>,
    }

    impl MockWriter {
        fn new() -> (Self, Arc<RwLock<Vec<u8>>>) {
            let buffer = Arc::new(RwLock::new(Vec::new()));
            (
                Self {
                    buffer: buffer.clone(),
                },
                buffer,
            )
        }
    }

    impl std::io::Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let mut buffer = futures::executor::block_on(self.buffer.write());
            buffer.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_json_progress_renderer_workflow_update() {
        let (writer, buffer) = MockWriter::new();
        let renderer = JsonProgressRenderer::new(Box::new(writer));

        let workflow = WorkflowProgress {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            status: WorkflowStatus::Running,
            start_time: Instant::now(),
            eta: None,
            total_steps: 10,
            completed_steps: 5,
            failed_steps: 1,
            current_phase: Some("processing".to_string()),
            resource_usage: ResourceUsage {
                cpu_percent: 45.5,
                memory_bytes: 1024 * 1024 * 100, // 100MB
                disk_bytes_written: 0,
                disk_bytes_read: 0,
                network_bytes_sent: 0,
                network_bytes_received: 0,
            },
        };

        let phases = std::collections::HashMap::new();

        renderer.update_display(&workflow, &phases).await.unwrap();

        let buffer_content = buffer.read().await;
        let content = String::from_utf8_lossy(&buffer_content);

        // Check that JSON was written
        assert!(content.contains("workflow_updated"));
        assert!(content.contains("\"id\":\"test-workflow\""));
        assert!(content.contains("\"completed_steps\":5"));
        assert!(content.contains("\"failed_steps\":1"));
        assert!(content.contains("\"current_phase\":\"processing\""));
        assert!(content.contains("\"cpu_percent\":45.5"));
    }

    #[tokio::test]
    async fn test_json_progress_renderer_phase_progress() {
        let (writer, buffer) = MockWriter::new();
        let renderer = JsonProgressRenderer::new(Box::new(writer));

        let workflow = WorkflowProgress {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            status: WorkflowStatus::Running,
            start_time: Instant::now(),
            eta: None,
            total_steps: 10,
            completed_steps: 5,
            failed_steps: 0,
            current_phase: Some("map".to_string()),
            resource_usage: ResourceUsage {
                cpu_percent: 0.0,
                memory_bytes: 0,
                disk_bytes_written: 0,
                disk_bytes_read: 0,
                network_bytes_sent: 0,
                network_bytes_received: 0,
            },
        };

        let mut phases = std::collections::HashMap::new();
        phases.insert(
            "map".to_string(),
            PhaseProgress {
                name: "map".to_string(),
                phase_type: PhaseType::Map,
                status: PhaseStatus::Running,
                start_time: Instant::now(),
                total_items: 100,
                processed_items: 50,
                successful_items: 48,
                failed_items: 2,
                active_agents: vec![],
                throughput: 10.5,
                avg_item_time: Duration::from_secs(2),
            },
        );

        renderer.update_display(&workflow, &phases).await.unwrap();

        let buffer_content = buffer.read().await;
        let content = String::from_utf8_lossy(&buffer_content);

        // Check phase progress event
        assert!(content.contains("phase_progress"));
        assert!(content.contains("\"name\":\"map\""));
        assert!(content.contains("\"processed_items\":50"));
        assert!(content.contains("\"successful_items\":48"));
        assert!(content.contains("\"failed_items\":2"));
        assert!(content.contains("\"throughput\":10.5"));
    }

    #[tokio::test]
    async fn test_json_progress_renderer_agent_progress() {
        let (writer, buffer) = MockWriter::new();
        let renderer = JsonProgressRenderer::new(Box::new(writer));

        let workflow = WorkflowProgress {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            status: WorkflowStatus::Running,
            start_time: Instant::now(),
            eta: None,
            total_steps: 10,
            completed_steps: 5,
            failed_steps: 0,
            current_phase: Some("map".to_string()),
            resource_usage: ResourceUsage {
                cpu_percent: 0.0,
                memory_bytes: 0,
                disk_bytes_written: 0,
                disk_bytes_read: 0,
                network_bytes_sent: 0,
                network_bytes_received: 0,
            },
        };

        let mut phases = std::collections::HashMap::new();
        phases.insert(
            "map".to_string(),
            PhaseProgress {
                name: "map".to_string(),
                phase_type: PhaseType::Map,
                status: PhaseStatus::Running,
                start_time: Instant::now(),
                total_items: 100,
                processed_items: 50,
                successful_items: 48,
                failed_items: 2,
                active_agents: vec![AgentProgress {
                    id: "agent-1".to_string(),
                    worktree: "worktree-1".to_string(),
                    current_item: Some("item-25".to_string()),
                    status: AgentStatus::Working,
                    items_processed: 24,
                    start_time: Instant::now(),
                    last_update: Instant::now(),
                    current_step: Some("processing".to_string()),
                    memory_usage: 0,
                    cpu_usage: 0.0,
                }],
                throughput: 10.5,
                avg_item_time: Duration::from_secs(2),
            },
        );

        renderer.update_display(&workflow, &phases).await.unwrap();

        let buffer_content = buffer.read().await;
        let content = String::from_utf8_lossy(&buffer_content);

        // Check agent progress event
        assert!(content.contains("agent_progress"));
        assert!(content.contains("\"id\":\"agent-1\""));
        assert!(content.contains("\"current_item\":\"item-25\""));
        assert!(content.contains("\"current_step\":\"processing\""));
        assert!(content.contains("\"items_processed\":24"));
    }

    #[tokio::test]
    async fn test_json_progress_renderer_multiple_events() {
        let (writer, buffer) = MockWriter::new();
        let renderer = JsonProgressRenderer::new(Box::new(writer));

        // Emit multiple events
        renderer
            .emit_event(ProgressEvent::WorkflowStarted {
                id: "workflow-1".to_string(),
                name: "Test".to_string(),
                total_steps: 10,
            })
            .await
            .unwrap();

        renderer
            .emit_event(ProgressEvent::PhaseStarted {
                name: "setup".to_string(),
                phase_type: PhaseType::Setup,
                total_items: 5,
            })
            .await
            .unwrap();

        renderer
            .emit_event(ProgressEvent::LogMessage {
                level: "info".to_string(),
                message: "Processing started".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
            })
            .await
            .unwrap();

        let buffer_content = buffer.read().await;
        let content = String::from_utf8_lossy(&buffer_content);

        // Should have 3 separate JSON lines
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 3);

        // Check each event
        assert!(lines[0].contains("workflow_started"));
        assert!(lines[1].contains("phase_started"));
        assert!(lines[2].contains("log_message"));

        // Verify each line is valid JSON
        for line in lines {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }
}
