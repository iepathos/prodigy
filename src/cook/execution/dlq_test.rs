#[cfg(test)]
mod tests {
    use super::super::dlq::*;
    use anyhow::Result;
    use chrono::{Duration, Utc};
    use serde_json::Value;
    use tempfile::tempdir;

    // Pure test helper functions
    fn create_test_failure_detail(
        attempt: u32,
        error_type: ErrorType,
        message: &str,
    ) -> FailureDetail {
        FailureDetail {
            attempt_number: attempt,
            timestamp: Utc::now(),
            error_type,
            error_message: message.to_string(),
            stack_trace: None,
            agent_id: format!("agent-{}", attempt),
            step_failed: "test-step".to_string(),
            duration_ms: 100,
        }
    }

    fn create_test_item(
        item_id: &str,
        data: Value,
        failure_count: u32,
        error_signature: &str,
        reprocess_eligible: bool,
    ) -> DeadLetteredItem {
        DeadLetteredItem {
            item_id: item_id.to_string(),
            item_data: data,
            first_attempt: Utc::now() - Duration::hours(1),
            last_attempt: Utc::now(),
            failure_count,
            failure_history: vec![create_test_failure_detail(
                1,
                ErrorType::Unknown,
                "Test error",
            )],
            error_signature: error_signature.to_string(),
            worktree_artifacts: None,
            reprocess_eligible,
            manual_review_required: false,
        }
    }

    async fn create_test_dlq(job_id: &str, max_items: usize) -> Result<DeadLetterQueue> {
        let temp_dir = tempdir()?;
        DeadLetterQueue::new(
            job_id.to_string(),
            temp_dir.path().to_path_buf(),
            max_items,
            30,
            None,
        )
        .await
    }

    fn assert_item_matches_expected(actual: &DeadLetteredItem, expected: &DeadLetteredItem) {
        assert_eq!(actual.item_id, expected.item_id);
        assert_eq!(actual.failure_count, expected.failure_count);
        assert_eq!(actual.error_signature, expected.error_signature);
        assert_eq!(actual.reprocess_eligible, expected.reprocess_eligible);
    }

    #[tokio::test]
    async fn test_dlq_basic_operations() -> Result<()> {
        let dlq = create_test_dlq("test-job", 100).await?;
        let test_item = create_test_item(
            "item-1",
            serde_json::json!({"test": "data"}),
            3,
            "TestSignature",
            true,
        );

        // Test adding an item
        dlq.add(test_item.clone()).await?;

        // Test getting the item
        let retrieved = dlq.get_item("item-1").await?;
        assert!(retrieved.is_some());
        assert_item_matches_expected(&retrieved.unwrap(), &test_item);

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
        let dlq = create_test_dlq("test-pattern", 100).await?;
        let error_signature = "Timeout::Connection timeout";

        // Add multiple items with similar errors
        for i in 0..3 {
            let item = create_test_item(
                &format!("item-{}", i),
                serde_json::json!({"id": i}),
                3,
                error_signature,
                true,
            );
            dlq.add(item).await?;
        }

        // Analyze patterns
        let analysis = dlq.analyze_patterns().await?;
        assert_eq!(analysis.total_items, 3);
        assert_eq!(analysis.pattern_groups.len(), 1);
        assert_eq!(analysis.pattern_groups[0].count, 3);
        assert_eq!(analysis.pattern_groups[0].signature, error_signature);

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_filtering() -> Result<()> {
        let dlq = create_test_dlq("test-filter", 100).await?;

        // Add items with different characteristics
        let reprocessable = create_test_item(
            "reprocessable",
            serde_json::json!({"type": "reprocessable"}),
            2,
            "ReprocessableError",
            true,
        );
        let non_reprocessable = create_test_item(
            "non-reprocessable",
            serde_json::json!({"type": "non-reprocessable"}),
            5,
            "FatalError",
            false,
        );

        dlq.add(reprocessable).await?;
        dlq.add(non_reprocessable).await?;

        // Test filtering by reprocess eligibility
        let filter = DLQFilter {
            reprocess_eligible: Some(true),
            ..Default::default()
        };
        let reprocessable_items = dlq.list_items(filter).await?;
        assert_eq!(reprocessable_items.len(), 1);
        assert_eq!(reprocessable_items[0].item_id, "reprocessable");

        // Test filtering by error signature
        let filter = DLQFilter {
            error_signature: Some("Fatal".to_string()),
            ..Default::default()
        };
        let fatal_items = dlq.list_items(filter).await?;
        assert_eq!(fatal_items.len(), 1);
        assert_eq!(fatal_items[0].item_id, "non-reprocessable");

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_capacity_management() -> Result<()> {
        let max_items = 3;
        let dlq = create_test_dlq("test-capacity", max_items).await?;

        // Add items up to capacity
        for i in 0..max_items + 2 {
            let item = create_test_item(
                &format!("item-{}", i),
                serde_json::json!({"index": i}),
                1,
                "TestError",
                true,
            );
            dlq.add(item).await?;
        }

        // Should have evicted oldest items
        let stats = dlq.get_stats().await?;
        assert!(stats.total_items <= max_items);

        // Verify the oldest items were evicted
        let item_0 = dlq.get_item("item-0").await?;
        assert!(item_0.is_none(), "Oldest item should have been evicted");

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_reprocessing() -> Result<()> {
        let dlq = create_test_dlq("test-reprocess", 100).await?;

        // Add reprocessable items
        let item1 = create_test_item(
            "reprocess-1",
            serde_json::json!({"data": 1}),
            2,
            "RetryableError",
            true,
        );
        let item2 = create_test_item(
            "reprocess-2",
            serde_json::json!({"data": 2}),
            2,
            "RetryableError",
            false, // Not eligible
        );

        dlq.add(item1).await?;
        dlq.add(item2).await?;

        // Reprocess eligible items
        let reprocessed = dlq
            .reprocess(vec!["reprocess-1".to_string(), "reprocess-2".to_string()])
            .await?;

        assert_eq!(reprocessed.len(), 1);
        assert_eq!(reprocessed[0], serde_json::json!({"data": 1}));

        // Verify eligible item was removed
        let item1_after = dlq.get_item("reprocess-1").await?;
        assert!(item1_after.is_none());

        // Verify non-eligible item remains
        let item2_after = dlq.get_item("reprocess-2").await?;
        assert!(item2_after.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_purging() -> Result<()> {
        let dlq = create_test_dlq("test-purge", 100).await?;

        // Add old and new items
        let old_item = DeadLetteredItem {
            item_id: "old-item".to_string(),
            item_data: serde_json::json!({"age": "old"}),
            first_attempt: Utc::now() - Duration::days(10),
            last_attempt: Utc::now() - Duration::days(5),
            failure_count: 1,
            failure_history: vec![create_test_failure_detail(
                1,
                ErrorType::Unknown,
                "Old error",
            )],
            error_signature: "OldError".to_string(),
            worktree_artifacts: None,
            reprocess_eligible: true,
            manual_review_required: false,
        };

        let new_item = create_test_item(
            "new-item",
            serde_json::json!({"age": "new"}),
            1,
            "NewError",
            true,
        );

        dlq.add(old_item).await?;
        dlq.add(new_item).await?;

        // Purge items older than 3 days
        let cutoff = Utc::now() - Duration::days(3);
        let purged_count = dlq.purge_old_items(cutoff).await?;

        assert_eq!(purged_count, 1);

        // Verify old item was purged
        let old_item_after = dlq.get_item("old-item").await?;
        assert!(old_item_after.is_none());

        // Verify new item remains
        let new_item_after = dlq.get_item("new-item").await?;
        assert!(new_item_after.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_error_signature_creation() {
        let signature = DeadLetterQueue::create_error_signature(
            &ErrorType::Timeout,
            "Connection failed at /path/to/file:123 with error code 42",
        );

        // Should filter out paths and numbers
        assert_eq!(signature, "Timeout::Connection failed at with error code");
    }

    #[tokio::test]
    async fn test_should_move_to_dlq() {
        assert!(!DeadLetterQueue::should_move_to_dlq(2, 3));
        assert!(DeadLetterQueue::should_move_to_dlq(4, 3));
        assert!(DeadLetterQueue::should_move_to_dlq(3, 2));
    }

    #[tokio::test]
    async fn test_dlq_error_distribution_analysis() -> Result<()> {
        let dlq = create_test_dlq("test-error-dist", 100).await?;

        // Add items with different error types
        let error_types = vec![
            ErrorType::Timeout,
            ErrorType::Timeout,
            ErrorType::CommandFailed { exit_code: 1 },
            ErrorType::ValidationFailed,
        ];

        for (i, error_type) in error_types.into_iter().enumerate() {
            let mut item = create_test_item(
                &format!("item-{}", i),
                serde_json::json!({"id": i}),
                1,
                "TestError",
                true,
            );
            item.failure_history = vec![create_test_failure_detail(
                1,
                error_type.clone(),
                "Test message",
            )];
            dlq.add(item).await?;
        }

        let analysis = dlq.analyze_patterns().await?;

        // Verify error distribution
        assert_eq!(
            *analysis
                .error_distribution
                .get(&ErrorType::Timeout)
                .unwrap_or(&0),
            2
        );
        assert_eq!(
            *analysis
                .error_distribution
                .get(&ErrorType::CommandFailed { exit_code: 1 })
                .unwrap_or(&0),
            1
        );
        assert_eq!(
            *analysis
                .error_distribution
                .get(&ErrorType::ValidationFailed)
                .unwrap_or(&0),
            1
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_dlq_persistence() -> Result<()> {
        let temp_dir = tempdir()?;
        let job_id = "persistence-test";
        let base_path = temp_dir.path().to_path_buf();

        // Create DLQ and add an item
        {
            let dlq =
                DeadLetterQueue::new(job_id.to_string(), base_path.clone(), 100, 30, None).await?;

            let item = create_test_item(
                "persistent-item",
                serde_json::json!({"persisted": true}),
                1,
                "PersistenceTest",
                true,
            );
            dlq.add(item).await?;
        }

        // Create new DLQ instance and verify item persisted
        {
            let dlq = DeadLetterQueue::new(job_id.to_string(), base_path, 100, 30, None).await?;
            let retrieved = dlq.get_item("persistent-item").await?;

            assert!(retrieved.is_some());
            let item = retrieved.unwrap();
            assert_eq!(item.item_id, "persistent-item");
            assert_eq!(item.error_signature, "PersistenceTest");
        }

        Ok(())
    }
}
