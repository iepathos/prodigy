//! Pure checkpoint trigger predicates
//!
//! This module contains pure functions for determining when a checkpoint
//! should be created. No I/O operations - just logic.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Configuration for checkpoint triggers
#[derive(Debug, Clone)]
pub struct CheckpointTriggerConfig {
    /// Create checkpoint after every N agent completions
    pub agent_completion_interval: Option<usize>,
    /// Create checkpoint every N seconds
    pub time_interval: Option<Duration>,
    /// Create checkpoint on signal (SIGINT/SIGTERM)
    pub on_signal: bool,
    /// Create checkpoint after each phase completes
    pub on_phase_completion: bool,
}

impl Default for CheckpointTriggerConfig {
    fn default() -> Self {
        Self {
            agent_completion_interval: Some(5),           // Every 5 agents
            time_interval: Some(Duration::from_secs(30)), // Every 30s
            on_signal: true,
            on_phase_completion: true,
        }
    }
}

impl CheckpointTriggerConfig {
    /// Create a configuration with no automatic triggers
    pub fn none() -> Self {
        Self {
            agent_completion_interval: None,
            time_interval: None,
            on_signal: true, // Always keep signal handling
            on_phase_completion: false,
        }
    }

    /// Create with only item-based triggers
    pub fn item_interval(interval: usize) -> Self {
        Self {
            agent_completion_interval: Some(interval),
            time_interval: None,
            on_signal: true,
            on_phase_completion: true,
        }
    }

    /// Create with only time-based triggers
    pub fn time_interval(interval: Duration) -> Self {
        Self {
            agent_completion_interval: None,
            time_interval: Some(interval),
            on_signal: true,
            on_phase_completion: true,
        }
    }
}

/// Pure: Determine if a checkpoint should be created
///
/// Checks both item-based and time-based triggers.
///
/// # Arguments
/// * `items_since_last_checkpoint` - Number of items processed since last checkpoint
/// * `last_checkpoint_time` - When the last checkpoint was created
/// * `current_time` - Current time
/// * `config` - Checkpoint trigger configuration
///
/// # Returns
/// True if a checkpoint should be created
pub fn should_checkpoint(
    items_since_last_checkpoint: usize,
    last_checkpoint_time: DateTime<Utc>,
    current_time: DateTime<Utc>,
    config: &CheckpointTriggerConfig,
) -> bool {
    should_checkpoint_by_items(items_since_last_checkpoint, config)
        || should_checkpoint_by_time(last_checkpoint_time, current_time, config)
}

/// Pure: Check if item-based trigger is met
fn should_checkpoint_by_items(
    items_since_last_checkpoint: usize,
    config: &CheckpointTriggerConfig,
) -> bool {
    config
        .agent_completion_interval
        .map(|interval| items_since_last_checkpoint >= interval)
        .unwrap_or(false)
}

/// Pure: Check if time-based trigger is met
fn should_checkpoint_by_time(
    last_checkpoint_time: DateTime<Utc>,
    current_time: DateTime<Utc>,
    config: &CheckpointTriggerConfig,
) -> bool {
    config.time_interval.is_some_and(|interval| {
        let elapsed = current_time.signed_duration_since(last_checkpoint_time);
        elapsed >= chrono::Duration::from_std(interval).unwrap_or(chrono::TimeDelta::MAX)
    })
}

/// Pure: Calculate how many items until next checkpoint
pub fn items_until_checkpoint(
    items_since_last_checkpoint: usize,
    config: &CheckpointTriggerConfig,
) -> Option<usize> {
    config
        .agent_completion_interval
        .map(|interval| interval.saturating_sub(items_since_last_checkpoint))
}

/// Pure: Calculate time until next checkpoint
pub fn time_until_checkpoint(
    last_checkpoint_time: DateTime<Utc>,
    current_time: DateTime<Utc>,
    config: &CheckpointTriggerConfig,
) -> Option<Duration> {
    config.time_interval.and_then(|interval| {
        let elapsed = current_time.signed_duration_since(last_checkpoint_time);
        let interval_chrono = chrono::Duration::from_std(interval).ok()?;
        if elapsed >= interval_chrono {
            Some(Duration::ZERO)
        } else {
            (interval_chrono - elapsed).to_std().ok()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_checkpoint_item_interval() {
        let config = CheckpointTriggerConfig {
            agent_completion_interval: Some(5),
            time_interval: None,
            on_signal: true,
            on_phase_completion: true,
        };
        let now = Utc::now();
        let last_checkpoint = now - chrono::Duration::seconds(10);

        // Below threshold
        assert!(!should_checkpoint(3, last_checkpoint, now, &config));

        // At threshold
        assert!(should_checkpoint(5, last_checkpoint, now, &config));

        // Above threshold
        assert!(should_checkpoint(10, last_checkpoint, now, &config));
    }

    #[test]
    fn test_should_checkpoint_time_interval() {
        let config = CheckpointTriggerConfig {
            agent_completion_interval: None,
            time_interval: Some(Duration::from_secs(30)),
            on_signal: true,
            on_phase_completion: true,
        };
        let now = Utc::now();

        // Within time interval
        let recent = now - chrono::Duration::seconds(20);
        assert!(!should_checkpoint(100, recent, now, &config));

        // Past time interval
        let old = now - chrono::Duration::seconds(35);
        assert!(should_checkpoint(0, old, now, &config));
    }

    #[test]
    fn test_should_checkpoint_either_trigger() {
        let config = CheckpointTriggerConfig::default();
        let now = Utc::now();
        let recent = now - chrono::Duration::seconds(5);

        // Item trigger met, time not met
        assert!(should_checkpoint(5, recent, now, &config));

        // Time trigger met, item not met
        let old = now - chrono::Duration::seconds(35);
        assert!(should_checkpoint(1, old, now, &config));

        // Neither trigger met
        assert!(!should_checkpoint(2, recent, now, &config));
    }

    #[test]
    fn test_items_until_checkpoint() {
        let config = CheckpointTriggerConfig::item_interval(5);

        assert_eq!(items_until_checkpoint(0, &config), Some(5));
        assert_eq!(items_until_checkpoint(3, &config), Some(2));
        assert_eq!(items_until_checkpoint(5, &config), Some(0));
        assert_eq!(items_until_checkpoint(7, &config), Some(0));
    }

    #[test]
    fn test_time_until_checkpoint() {
        let config = CheckpointTriggerConfig::time_interval(Duration::from_secs(30));
        let now = Utc::now();
        let past = now - chrono::Duration::seconds(10);

        let remaining = time_until_checkpoint(past, now, &config);
        assert!(remaining.is_some());
        let remaining = remaining.unwrap();
        assert!(remaining.as_secs() >= 19 && remaining.as_secs() <= 21);

        // Already past threshold
        let old = now - chrono::Duration::seconds(40);
        assert_eq!(
            time_until_checkpoint(old, now, &config),
            Some(Duration::ZERO)
        );
    }

    #[test]
    fn test_config_none_disables_triggers() {
        let config = CheckpointTriggerConfig::none();
        let now = Utc::now();
        let old = now - chrono::Duration::hours(1);

        // Neither item nor time trigger should fire
        assert!(!should_checkpoint(1000, old, now, &config));
    }
}
