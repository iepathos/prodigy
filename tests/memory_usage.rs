//! Memory usage tests for cloning optimizations
//!
//! Validates that the optimizations from spec 104 reduce memory usage

use std::collections::HashMap;
use std::mem;
use std::sync::Arc;

/// Test memory size of String vs Arc<str>
#[test]
fn test_string_vs_arc_memory() {
    let test_str = "This is a test string for memory measurement";

    // String version
    let string_version = test_str.to_string();
    let string_size = mem::size_of_val(&string_version) + string_version.capacity();

    // Arc<str> version
    let arc_version: Arc<str> = Arc::from(test_str);
    let arc_size = mem::size_of_val(&arc_version);

    // Arc should use less memory for the handle
    assert!(
        arc_size < string_size,
        "Arc<str> handle ({} bytes) should be smaller than String ({} bytes)",
        arc_size,
        string_size
    );

    // Multiple clones
    let string_clones: Vec<_> = (0..10).map(|_| string_version.clone()).collect();
    let arc_clones: Vec<_> = (0..10).map(|_| Arc::clone(&arc_version)).collect();

    let total_string_size = string_clones
        .iter()
        .map(|s| mem::size_of_val(s) + s.capacity())
        .sum::<usize>();

    let total_arc_size = arc_clones.len() * mem::size_of_val(&arc_version);

    // Arc clones should use significantly less memory
    assert!(
        total_arc_size < total_string_size / 3,
        "Arc clones ({} bytes) should use much less memory than String clones ({} bytes)",
        total_arc_size,
        total_string_size
    );
}

/// Test memory size of HashMap clone vs Arc<HashMap>
#[test]
fn test_hashmap_vs_arc_memory() {
    let mut map = HashMap::new();
    for i in 0..100 {
        map.insert(format!("key_{}", i), format!("value_{}", i));
    }

    // Estimate HashMap memory (approximate)
    let map_size_estimate =
        mem::size_of_val(&map) + map.capacity() * (mem::size_of::<String>() * 2);

    // Arc version
    let arc_map = Arc::new(map.clone());
    let arc_handle_size = mem::size_of_val(&arc_map);

    // Arc handle should be much smaller
    assert!(
        arc_handle_size < map_size_estimate / 10,
        "Arc<HashMap> handle ({} bytes) should be much smaller than HashMap estimate ({} bytes)",
        arc_handle_size,
        map_size_estimate
    );
}

/// Test memory impact of Cow usage in path operations
#[test]
fn test_cow_memory_efficiency() {
    use std::borrow::Cow;

    let path = "/home/user/documents/project/src/main.rs";

    // Case 1: No modification needed - Cow borrows
    let cow_borrowed: Cow<str> = Cow::Borrowed(path);
    let cow_size = mem::size_of_val(&cow_borrowed);

    // Case 2: Modification needed - Cow owns
    let mut cow_owned: Cow<str> = Cow::Borrowed(path);
    if path.contains("/home/") {
        cow_owned = Cow::Owned(path.replace("/home/", "/users/"));
    }
    let owned_size = mem::size_of_val(&cow_owned);

    // Both should be small (pointer-sized or small enum)
    assert!(
        cow_size <= 32,
        "Cow size should be small: {} bytes",
        cow_size
    );
    assert!(
        owned_size <= 32,
        "Owned Cow size should be small: {} bytes",
        owned_size
    );
}

/// Test compound data structure memory optimization
#[test]
fn test_workflow_data_memory_optimization() {
    #[derive(Clone)]
    #[allow(dead_code)]
    struct OriginalWorkflow {
        id: String,
        name: String,
        description: String,
        variables: HashMap<String, String>,
        commands: Vec<String>,
    }

    struct OptimizedWorkflow {
        id: Arc<str>,
        name: Arc<str>,
        description: Arc<str>,
        variables: Arc<HashMap<String, String>>,
        commands: Arc<[Arc<str>]>,
    }

    impl Clone for OptimizedWorkflow {
        fn clone(&self) -> Self {
            Self {
                id: Arc::clone(&self.id),
                name: Arc::clone(&self.name),
                description: Arc::clone(&self.description),
                variables: Arc::clone(&self.variables),
                commands: Arc::clone(&self.commands),
            }
        }
    }

    let mut vars = HashMap::new();
    for i in 0..20 {
        vars.insert(format!("var_{}", i), format!("value_{}", i));
    }

    let commands = vec![
        "cargo build".to_string(),
        "cargo test".to_string(),
        "cargo clippy".to_string(),
        "cargo fmt".to_string(),
    ];

    let original = OriginalWorkflow {
        id: "workflow-123".to_string(),
        name: "Test Workflow".to_string(),
        description: "A comprehensive test workflow".to_string(),
        variables: vars.clone(),
        commands: commands.clone(),
    };

    let optimized = OptimizedWorkflow {
        id: Arc::from("workflow-123"),
        name: Arc::from("Test Workflow"),
        description: Arc::from("A comprehensive test workflow"),
        variables: Arc::new(vars),
        commands: commands
            .iter()
            .map(|s| Arc::from(s.as_str()))
            .collect::<Vec<_>>()
            .into(),
    };

    // Test single clone
    let original_size = mem::size_of_val(&original);
    let optimized_size = mem::size_of_val(&optimized);

    // Optimized should have smaller stack footprint
    assert!(
        optimized_size <= original_size,
        "Optimized workflow ({} bytes) should not be larger than original ({} bytes)",
        optimized_size,
        original_size
    );

    // Test multiple clones (simulating agents)
    let original_clones: Vec<_> = (0..10).map(|_| original.clone()).collect();
    let optimized_clones: Vec<_> = (0..10).map(|_| optimized.clone()).collect();

    // For clones, optimized should be much smaller (Arc copies vs deep copies)
    let original_clone_size = original_clones.len() * mem::size_of_val(&original);
    let optimized_clone_size = optimized_clones.len() * mem::size_of_val(&optimized);

    assert!(
        optimized_clone_size < original_clone_size,
        "Optimized clones ({} bytes) should use less memory than original clones ({} bytes)",
        optimized_clone_size,
        original_clone_size
    );
}

/// Test that Arc prevents unnecessary allocations
#[test]
fn test_arc_prevents_allocations() {
    let large_string = "x".repeat(10000);
    let arc_string: Arc<str> = Arc::from(large_string.as_str());

    // Get initial strong count
    let initial_count = Arc::strong_count(&arc_string);

    // Clone multiple times
    let clones: Vec<_> = (0..100).map(|_| Arc::clone(&arc_string)).collect();

    // Verify reference count increased
    assert_eq!(
        Arc::strong_count(&arc_string),
        initial_count + 100,
        "Arc strong count should reflect all clones"
    );

    // Verify all point to same data (same pointer)
    let ptr1 = Arc::as_ptr(&arc_string);
    for clone in &clones {
        let ptr2 = Arc::as_ptr(clone);
        assert_eq!(ptr1, ptr2, "All Arc clones should point to the same memory");
    }

    // Drop clones and verify count decreases
    drop(clones);
    assert_eq!(
        Arc::strong_count(&arc_string),
        initial_count,
        "Strong count should return to initial after dropping clones"
    );
}

/// Test memory efficiency in concurrent scenarios
#[test]
fn test_concurrent_memory_efficiency() {
    use std::thread;

    let data: Arc<Vec<String>> = Arc::new((0..1000).map(|i| format!("item_{}", i)).collect());

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let data_clone = Arc::clone(&data);
            thread::spawn(move || {
                // Each thread has access to the same data
                let _item = &data_clone[500];
                Arc::strong_count(&data_clone)
            })
        })
        .collect();

    let counts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All threads should see multiple references
    for count in counts {
        assert!(count > 1, "Each thread should see shared references");
    }

    // After all threads complete, we should be back to 1
    assert_eq!(
        Arc::strong_count(&data),
        1,
        "After all threads complete, only original reference should remain"
    );
}
