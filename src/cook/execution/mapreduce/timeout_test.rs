//! Tests for MapReduce timeout enforcement

use super::timeout::*;
use crate::cook::workflow::WorkflowStep;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_timeout_config_defaults() {
    let config = TimeoutConfig::default();
    assert_eq!(config.agent_timeout_secs, Some(600));
    assert_eq!(config.cleanup_grace_period_secs, 30);
    assert!(config.enable_monitoring);
    assert!(matches!(config.timeout_policy, TimeoutPolicy::PerAgent));
    assert!(matches!(config.timeout_action, TimeoutAction::Dlq));
}

#[tokio::test]
async fn test_timeout_enforcer_creation() {
    let config = TimeoutConfig {
        agent_timeout_secs: Some(10),
        ..TimeoutConfig::default()
    };

    let enforcer = TimeoutEnforcer::new(config);
    assert!(enforcer.is_enabled());
}

#[tokio::test]
async fn test_register_agent_timeout() {
    let config = TimeoutConfig {
        agent_timeout_secs: Some(5),
        ..TimeoutConfig::default()
    };

    let enforcer = Arc::new(TimeoutEnforcer::new(config));

    let commands = vec![WorkflowStep {
        claude: Some("test command".to_string()),
        ..WorkflowStep::default()
    }];

    let handle = enforcer
        .register_agent_timeout("agent-1".to_string(), "item-1".to_string(), &commands)
        .await
        .unwrap();

    assert_eq!(handle.agent_id, "agent-1");
    assert_eq!(handle.work_item_id, "item-1");
    assert_eq!(handle.timeout_duration, Duration::from_secs(5));
}

#[tokio::test]
async fn test_command_timeout_calculation() {
    let mut config = TimeoutConfig::default();
    config.command_timeouts.insert("claude".to_string(), 120);
    config.command_timeouts.insert("shell".to_string(), 30);

    let enforcer = TimeoutEnforcer::new(config);

    let commands = vec![
        WorkflowStep {
            claude: Some("claude command".to_string()),
            ..WorkflowStep::default()
        },
        WorkflowStep {
            shell: Some("shell command".to_string()),
            ..WorkflowStep::default()
        },
    ];

    let handle = enforcer
        .register_agent_timeout("agent-1".to_string(), "item-1".to_string(), &commands)
        .await
        .unwrap();

    assert_eq!(handle.command_timeouts.len(), 2);
    assert_eq!(
        handle.command_timeouts[0].timeout_duration,
        Duration::from_secs(120)
    );
    assert_eq!(
        handle.command_timeouts[1].timeout_duration,
        Duration::from_secs(30)
    );
}

#[tokio::test]
async fn test_timeout_metrics() {
    let config = TimeoutConfig::default();
    let enforcer = Arc::new(TimeoutEnforcer::new(config));

    // Register and complete an agent
    let commands = vec![WorkflowStep::default()];
    let _handle = enforcer
        .register_agent_timeout("agent-1".to_string(), "item-1".to_string(), &commands)
        .await
        .unwrap();

    // Wait a bit
    sleep(Duration::from_millis(100)).await;

    // Unregister (complete)
    enforcer
        .unregister_agent_timeout(&"agent-1".to_string())
        .await
        .unwrap();

    // Check metrics
    let summary = enforcer.get_metrics().await;
    assert_eq!(summary.agents_started, 1);
    assert_eq!(summary.agents_completed, 1);
    assert_eq!(summary.timeouts_occurred, 0);
    assert_eq!(summary.timeout_rate_percent, 0.0);
}

#[tokio::test]
async fn test_timeout_cancellation() {
    let config = TimeoutConfig {
        agent_timeout_secs: Some(1),
        ..TimeoutConfig::default()
    };

    let enforcer = Arc::new(TimeoutEnforcer::new(config));

    let commands = vec![WorkflowStep::default()];
    let handle = enforcer
        .register_agent_timeout("agent-1".to_string(), "item-1".to_string(), &commands)
        .await
        .unwrap();

    // Cancel immediately (simulating successful completion)
    handle.cancel_notify.notify_one();

    // Wait longer than the timeout
    sleep(Duration::from_secs(2)).await;

    // Check metrics - should show no timeouts
    let summary = enforcer.get_metrics().await;
    assert_eq!(summary.timeouts_occurred, 0);
}

#[tokio::test]
async fn test_per_command_timeout_policy() {
    let config = TimeoutConfig {
        agent_timeout_secs: Some(100),
        timeout_policy: TimeoutPolicy::PerCommand,
        ..TimeoutConfig::default()
    };

    let enforcer = Arc::new(TimeoutEnforcer::new(config));

    let commands = vec![
        WorkflowStep {
            claude: Some("command 1".to_string()),
            ..WorkflowStep::default()
        },
        WorkflowStep {
            shell: Some("command 2".to_string()),
            ..WorkflowStep::default()
        },
    ];

    let _handle = enforcer
        .register_agent_timeout("agent-1".to_string(), "item-1".to_string(), &commands)
        .await
        .unwrap();

    // Register command start
    enforcer
        .register_command_start(&"agent-1".to_string(), 0)
        .await
        .unwrap();

    // Wait a bit
    sleep(Duration::from_millis(100)).await;

    // Register command completion
    enforcer
        .register_command_completion(&"agent-1".to_string(), 0, Duration::from_millis(100))
        .await
        .unwrap();

    // Clean up
    enforcer
        .unregister_agent_timeout(&"agent-1".to_string())
        .await
        .unwrap();
}

#[tokio::test]
async fn test_timeout_config_serialization() {
    let config = TimeoutConfig {
        agent_timeout_secs: Some(300),
        timeout_policy: TimeoutPolicy::Hybrid,
        timeout_action: TimeoutAction::GracefulTerminate,
        cleanup_grace_period_secs: 45,
        enable_monitoring: true,
        ..TimeoutConfig::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let parsed: TimeoutConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.agent_timeout_secs, Some(300));
    assert!(matches!(parsed.timeout_policy, TimeoutPolicy::Hybrid));
    assert!(matches!(
        parsed.timeout_action,
        TimeoutAction::GracefulTerminate
    ));
    assert_eq!(parsed.cleanup_grace_period_secs, 45);
}

#[tokio::test]
async fn test_yaml_config_parsing() {
    let yaml = r#"
agent_timeout_secs: 600
timeout_config:
  timeout_policy: hybrid
  cleanup_grace_period_secs: 30
  timeout_action: dlq
  enable_monitoring: true
  command_timeouts:
    claude: 300
    shell: 60
    claude_0: 600
"#;

    let parsed: serde_yaml::Value = serde_yaml::from_str(yaml).unwrap();
    assert!(parsed.get("agent_timeout_secs").is_some());
    assert!(parsed.get("timeout_config").is_some());
}

#[test]
fn test_timeout_error_conversion() {
    use crate::cook::execution::errors::MapReduceError;

    let error = TimeoutError::AgentTimeout {
        agent_id: "agent-1".to_string(),
        duration: Duration::from_secs(60),
    };

    let mr_error: MapReduceError = error.into();
    let error_msg = mr_error.to_string();
    assert!(error_msg.contains("agent-1"));
    assert!(error_msg.contains("60"));
}
