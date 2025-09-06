#[cfg(test)]
mod tests {
    use super::super::dlq::*;
    use anyhow::Result;
    use chrono::Utc;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_dlq_basic_operations() -> Result<()> {
        let temp_dir = tempdir()?;
        let dlq = DeadLetterQueue::new(
            "test-job".to_string(),
            temp_dir.path().to_path_buf(),
            100,
            30,
            None,
        )
        .await?;

        // Test adding an item
        let item = DeadLetteredItem {
            item_id: "item-1".to_string(),
            item_data: serde_json::json!({"test": "data"}),
            first_attempt: Utc::now(),
            last_attempt: Utc::now(),
            failure_count: 3,
            failure_history: vec![FailureDetail {
                attempt_number: 1,
                timestamp: Utc::now(),
                error_type: ErrorType::Unknown,
                error_message: "Test error".to_string(),
                stack_trace: None,
                agent_id: "agent-1".to_string(),
                step_failed: "test-step".to_string(),
                duration_ms: 100,
            }],
            error_signature: "TestSignature".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };

        dlq.add(item.clone()).await?;

        // Test getting the item
        let retrieved = dlq.get_item("item-1").await?;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().item_id, "item-1");

        // Test listing items
        let items = dlq.list_items(DLQFilter::default()).await?;
        assert_eq!(items.len(), 1);

        // Test stats
        let stats = dlq.get_stats().await?;
        assert_eq!(stats.total_items, 1);
        assert_eq!(stats.eligible_for_reprocess, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_pattern_analysis() -> Result<()> {
        let temp_dir = tempdir()?;
        let dlq = DeadLetterQueue::new(
            "test-job".to_string(),
            temp_dir.path().to_path_buf(),
            100,
            30,
            None,
        )
        .await?;

        // Add multiple items with similar errors
        for i in 0..3 {
            let item = DeadLetteredItem {
                item_id: format!("item-{}", i),
                item_data: serde_json::json!({"id": i}),
                first_attempt: Utc::now(),
                last_attempt: Utc::now(),
                failure_count: 3,
                failure_history: vec![FailureDetail {
                    attempt_number: 1,
                    timestamp: Utc::now(),
                    error_type: ErrorType::Timeout,
                    error_message: "Connection timeout".to_string(),
                    stack_trace: None,
                    agent_id: format!("agent-{}", i),
                    step_failed: "connect".to_string(),
                    duration_ms: 5000,
                }],
                error_signature: "Timeout::Connection timeout".to_string(),
                worktree_artifacts: None,
                reprocess_eligible: true,
                manual_review_required: false,
            };

            dlq.add(item).await?;
        }

        // Analyze patterns
        let analysis = dlq.analyze_patterns().await?;
        assert_eq!(analysis.total_items, 3);
        assert_eq!(analysis.pattern_groups.len(), 1);
        assert_eq!(analysis.pattern_groups[0].count, 3);

        Ok(())
    }
}
