use prodigy::cook::execution::mapreduce::agent::types::AgentResult;
use std::time::Duration;

#[test]
fn test_agent_result_json_serialization() {
    let result = AgentResult::success(
        "test_item".to_string(),
        Some("test output".to_string()),
        Duration::from_secs(120),
    );

    // Try to serialize to JSON string
    let json_str =
        serde_json::to_string(&result).expect("Failed to serialize AgentResult to JSON string");
    println!("Serialized JSON string: {}", json_str);

    // Try to serialize to JSON Value
    let json_value =
        serde_json::to_value(&result).expect("Failed to serialize AgentResult to JSON value");
    println!("Serialized JSON value: {:?}", json_value);

    // Try to serialize the value back to string (this is what interpolation does)
    let reserialize =
        serde_json::to_string(&json_value).expect("Failed to re-serialize JSON value");
    println!("Re-serialized: {}", reserialize);

    // Verify it's valid JSON by parsing it
    serde_json::from_str::<serde_json::Value>(&reserialize)
        .expect("Re-serialized JSON is not valid");
}

#[test]
fn test_agent_result_array_serialization() {
    let results = vec![
        AgentResult::success(
            "item_0".to_string(),
            Some("output 0".to_string()),
            Duration::from_secs(120),
        ),
        AgentResult::success(
            "item_1".to_string(),
            Some("output 1".to_string()),
            Duration::from_secs(150),
        ),
    ];

    // Serialize array to Value (like reduce phase does)
    let json_value =
        serde_json::to_value(&results).expect("Failed to serialize array to JSON value");

    // Serialize value to string (like interpolation does)
    let json_str = serde_json::to_string(&json_value).expect("Failed to serialize value to string");
    println!("Array serialized: {}", json_str);

    // Verify it can be parsed (like write_file does)
    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).expect("Failed to parse serialized JSON");
    println!("Parsed successfully: {:?}", parsed);
}
