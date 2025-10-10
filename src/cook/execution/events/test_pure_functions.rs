// Test module for pure functions - to be included in event_store tests
// Note: This file is included via include! macro, so imports are already available

#[test]
fn test_calculate_time_range_empty() {
    let events = vec![];
    assert_eq!(calculate_time_range(&events), None);
}

#[test]
fn test_calculate_time_range_single_event() {
    let timestamp = Utc::now();
    let event = EventRecord {
        id: Uuid::new_v4(),
        timestamp,
        correlation_id: "test".to_string(),
        event: MapReduceEvent::JobStarted {
            job_id: "job-1".to_string(),
            config: MapReduceConfig {
                agent_timeout_secs: None,
                continue_on_failure: false,
                batch_size: None,
                enable_checkpoints: true,
                input: "test.json".to_string(),
                json_path: "$.items".to_string(),
                max_parallel: 5,
                max_items: None,
                offset: None,
            },
            total_items: 10,
            timestamp,
        },
        metadata: HashMap::new(),
    };

    let events = vec![event];
    let result = calculate_time_range(&events);
    assert!(result.is_some());
    let (start, end) = result.unwrap();
    assert_eq!(start, timestamp);
    assert_eq!(end, timestamp);
}

#[test]
fn test_calculate_time_range_multiple_events() {
    let t1 = Utc::now();
    let t2 = t1 + chrono::Duration::seconds(10);
    let t3 = t1 + chrono::Duration::seconds(20);

    let events = vec![
        EventRecord {
            id: Uuid::new_v4(),
            timestamp: t2,
            correlation_id: "test".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: "job-1".to_string(),
                config: MapReduceConfig {
                    agent_timeout_secs: None,
                    continue_on_failure: false,
                    batch_size: None,
                    enable_checkpoints: true,
                    input: "test.json".to_string(),
                    json_path: "$.items".to_string(),
                    max_parallel: 5,
                    max_items: None,
                    offset: None,
                },
                total_items: 10,
                timestamp: t2,
            },
            metadata: HashMap::new(),
        },
        EventRecord {
            id: Uuid::new_v4(),
            timestamp: t1,
            correlation_id: "test".to_string(),
            event: MapReduceEvent::AgentStarted {
                job_id: "job-1".to_string(),
                agent_id: "agent-1".to_string(),
                item_id: "item-1".to_string(),
                worktree: "worktree-1".to_string(),
                attempt: 1,
            },
            metadata: HashMap::new(),
        },
        EventRecord {
            id: Uuid::new_v4(),
            timestamp: t3,
            correlation_id: "test".to_string(),
            event: MapReduceEvent::JobCompleted {
                job_id: "job-1".to_string(),
                success_count: 1,
                failure_count: 0,
                duration: chrono::Duration::seconds(20),
            },
            metadata: HashMap::new(),
        },
    ];

    let result = calculate_time_range(&events);
    assert!(result.is_some());
    let (start, end) = result.unwrap();
    assert_eq!(start, t1, "Should find earliest timestamp");
    assert_eq!(end, t3, "Should find latest timestamp");
}

#[test]
fn test_build_index_from_events_empty() {
    let job_id = "test-job";
    let file_events = vec![];

    let index = build_index_from_events(job_id, file_events);

    assert_eq!(index.job_id, job_id);
    assert_eq!(index.total_events, 0);
    assert!(index.event_counts.is_empty());
    assert!(index.file_offsets.is_empty());
}

#[test]
fn test_build_index_from_events_single_file() {
    let job_id = "test-job";
    let timestamp = Utc::now();
    let event1 = EventRecord {
        id: Uuid::new_v4(),
        timestamp,
        correlation_id: "test".to_string(),
        event: MapReduceEvent::JobStarted {
            job_id: job_id.to_string(),
            config: MapReduceConfig {
                agent_timeout_secs: None,
                continue_on_failure: false,
                batch_size: None,
                enable_checkpoints: true,
                input: "test.json".to_string(),
                json_path: "$.items".to_string(),
                max_parallel: 5,
                max_items: None,
                offset: None,
            },
            total_items: 10,
            timestamp,
        },
        metadata: HashMap::new(),
    };

    let event2 = EventRecord {
        id: Uuid::new_v4(),
        timestamp: timestamp + chrono::Duration::seconds(1),
        correlation_id: "test".to_string(),
        event: MapReduceEvent::AgentStarted {
            job_id: job_id.to_string(),
            agent_id: "agent-1".to_string(),
            item_id: "item-1".to_string(),
            worktree: "worktree-1".to_string(),
            attempt: 1,
        },
        metadata: HashMap::new(),
    };

    let file_events = vec![(
        PathBuf::from("test-file.jsonl"),
        vec![(event1.clone(), 0, 1), (event2.clone(), 100, 2)],
    )];

    let index = build_index_from_events(job_id, file_events);

    assert_eq!(index.job_id, job_id);
    assert_eq!(index.total_events, 2);
    assert_eq!(index.event_counts.get("job_started"), Some(&1));
    assert_eq!(index.event_counts.get("agent_started"), Some(&1));
    assert_eq!(index.file_offsets.len(), 2);

    // Check time range
    assert_eq!(index.time_range.0, timestamp);
    assert_eq!(index.time_range.1, timestamp + chrono::Duration::seconds(1));

    // Check file offsets
    assert_eq!(index.file_offsets[0].event_id, event1.id);
    assert_eq!(index.file_offsets[0].byte_offset, 0);
    assert_eq!(index.file_offsets[0].line_number, 1);
    assert_eq!(index.file_offsets[1].event_id, event2.id);
    assert_eq!(index.file_offsets[1].byte_offset, 100);
    assert_eq!(index.file_offsets[1].line_number, 2);
}

#[test]
fn test_build_index_from_events_multiple_files() {
    let job_id = "test-job";
    let timestamp = Utc::now();

    let events_file1 = vec![(
        EventRecord {
            id: Uuid::new_v4(),
            timestamp,
            correlation_id: "test".to_string(),
            event: MapReduceEvent::JobStarted {
                job_id: job_id.to_string(),
                config: MapReduceConfig {
                    agent_timeout_secs: None,
                    continue_on_failure: false,
                    batch_size: None,
                    enable_checkpoints: true,
                    input: "test.json".to_string(),
                    json_path: "$.items".to_string(),
                    max_parallel: 5,
                    max_items: None,
                    offset: None,
                },
                total_items: 10,
                timestamp,
            },
            metadata: HashMap::new(),
        },
        0,
        1,
    )];

    let events_file2 = vec![
        (
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp + chrono::Duration::seconds(1),
                correlation_id: "test".to_string(),
                event: MapReduceEvent::AgentStarted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-1".to_string(),
                    item_id: "item-1".to_string(),
                    worktree: "worktree-1".to_string(),
                    attempt: 1,
                },
                metadata: HashMap::new(),
            },
            0,
            1,
        ),
        (
            EventRecord {
                id: Uuid::new_v4(),
                timestamp: timestamp + chrono::Duration::seconds(2),
                correlation_id: "test".to_string(),
                event: MapReduceEvent::AgentCompleted {
                    job_id: job_id.to_string(),
                    agent_id: "agent-1".to_string(),
                    commits: vec![],
                    duration: chrono::Duration::seconds(1),
                    json_log_location: None,
                },
                metadata: HashMap::new(),
            },
            150,
            2,
        ),
    ];

    let file_events = vec![
        (PathBuf::from("file1.jsonl"), events_file1),
        (PathBuf::from("file2.jsonl"), events_file2),
    ];

    let index = build_index_from_events(job_id, file_events);

    assert_eq!(index.job_id, job_id);
    assert_eq!(index.total_events, 3);
    assert_eq!(index.event_counts.get("job_started"), Some(&1));
    assert_eq!(index.event_counts.get("agent_started"), Some(&1));
    assert_eq!(index.event_counts.get("agent_completed"), Some(&1));
    assert_eq!(index.file_offsets.len(), 3);

    // Check time range spans all events
    assert_eq!(index.time_range.0, timestamp);
    assert_eq!(index.time_range.1, timestamp + chrono::Duration::seconds(2));
}

#[test]
fn test_update_time_range() {
    let t1 = Utc::now();
    let t2 = t1 + chrono::Duration::seconds(10);
    let t3 = t1 + chrono::Duration::seconds(20);

    // Test with empty range
    let (start, end) = update_time_range(None, None, t1);
    assert_eq!(start, Some(t1));
    assert_eq!(end, Some(t1));

    // Test updating start
    let (start, end) = update_time_range(Some(t2), Some(t2), t1);
    assert_eq!(start, Some(t1));
    assert_eq!(end, Some(t2));

    // Test updating end
    let (start, end) = update_time_range(Some(t1), Some(t2), t3);
    assert_eq!(start, Some(t1));
    assert_eq!(end, Some(t3));

    // Test no update needed
    let (start, end) = update_time_range(Some(t1), Some(t3), t2);
    assert_eq!(start, Some(t1));
    assert_eq!(end, Some(t3));
}

#[test]
fn test_increment_event_count() {
    let mut counts = HashMap::new();

    // First increment
    increment_event_count(&mut counts, "job_started".to_string());
    assert_eq!(counts.get("job_started"), Some(&1));

    // Second increment
    increment_event_count(&mut counts, "job_started".to_string());
    assert_eq!(counts.get("job_started"), Some(&2));

    // Different event type
    increment_event_count(&mut counts, "agent_started".to_string());
    assert_eq!(counts.get("agent_started"), Some(&1));
    assert_eq!(counts.get("job_started"), Some(&2));
}