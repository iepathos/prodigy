//! Variable checkpoint state management for workflow resume
//!
//! This module handles persisting and restoring variable state across workflow interruptions,
//! ensuring that all variable types maintain their values correctly during resume operations.

use crate::cook::workflow::variables::VariableStore;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Complete variable state for checkpoint persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableCheckpointState {
    /// Global workflow variables
    pub global_variables: HashMap<String, Value>,

    /// Phase-specific variables (setup, map, reduce)
    pub phase_variables: HashMap<String, HashMap<String, Value>>,

    /// Cached computed values
    pub computed_cache: HashMap<String, CachedValue>,

    /// Environment snapshot at checkpoint time
    pub environment_snapshot: EnvironmentSnapshot,

    /// History of variable interpolations for validation
    pub interpolation_history: Vec<InterpolationRecord>,

    /// Metadata about variable state
    pub variable_metadata: VariableMetadata,

    /// Captured outputs from commands
    pub captured_outputs: HashMap<String, String>,

    /// Iteration variables for loops
    pub iteration_vars: HashMap<String, String>,
}

/// Cached computed value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedValue {
    /// The computed value
    pub value: Value,

    /// When this value was computed
    pub computed_at: DateTime<Utc>,

    /// Cache key for invalidation
    pub cache_key: String,

    /// Variable dependencies
    pub dependencies: Vec<String>,

    /// Whether this is an expensive computation
    pub is_expensive: bool,
}

/// Environment snapshot for compatibility checking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    /// Environment variables at checkpoint time
    pub variables: HashMap<String, String>,

    /// When the snapshot was taken
    pub captured_at: DateTime<Utc>,

    /// Hostname for validation
    pub hostname: String,

    /// Working directory
    pub working_directory: PathBuf,
}

/// Record of a variable interpolation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationRecord {
    /// Template that was interpolated
    pub template: String,

    /// Result of interpolation
    pub result: String,

    /// When interpolation occurred
    pub interpolated_at: DateTime<Utc>,

    /// Variables that were referenced
    pub variable_dependencies: Vec<String>,

    /// Phase context (if any)
    pub phase: Option<String>,
}

/// Metadata about variable state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableMetadata {
    /// Total number of variables
    pub total_variables: usize,

    /// Number of computed variables
    pub computed_variables: usize,

    /// Number of interpolations performed
    pub total_interpolations: usize,

    /// Checkpoint version for compatibility
    pub checkpoint_version: String,
}

/// Environment compatibility check results
#[derive(Debug, Clone)]
pub struct EnvironmentCompatibility {
    /// Variables missing in current environment
    pub missing_variables: HashMap<String, String>,

    /// Variables that changed values
    pub changed_variables: HashMap<String, (String, String)>, // (old, new)

    /// New variables in current environment
    pub new_variables: HashMap<String, String>,

    /// Whether resume is safe
    pub is_compatible: bool,
}

impl EnvironmentCompatibility {
    /// Create new compatibility check
    pub fn new() -> Self {
        Self {
            missing_variables: HashMap::new(),
            changed_variables: HashMap::new(),
            new_variables: HashMap::new(),
            is_compatible: true,
        }
    }

    /// Add a missing variable
    pub fn add_missing_variable(&mut self, key: String, value: String) {
        self.missing_variables.insert(key, value);
    }

    /// Add a changed variable
    pub fn add_changed_variable(&mut self, key: String, old_value: String, new_value: String) {
        self.changed_variables.insert(key, (old_value, new_value));
    }

    /// Add a new variable
    pub fn add_new_variable(&mut self, key: String, value: String) {
        self.new_variables.insert(key, value);
    }

    /// Check if there are critical changes
    pub fn has_critical_changes(&self) -> bool {
        // Critical if required variables are missing or changed
        !self.missing_variables.is_empty() || !self.changed_variables.is_empty()
    }
}

/// Test results for variable interpolation validation
#[derive(Debug, Clone)]
pub struct InterpolationTestResults {
    /// Individual test results
    pub tests: Vec<InterpolationTest>,

    /// Total tests run
    pub total_tests: usize,

    /// Tests that passed
    pub passed_tests: usize,

    /// Tests that failed
    pub failed_tests: usize,

    /// Duration of testing
    pub test_duration: std::time::Duration,
}

impl InterpolationTestResults {
    /// Create new test results
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            test_duration: std::time::Duration::default(),
        }
    }

    /// Add a test result
    pub fn add_test(&mut self, test: InterpolationTest) {
        self.total_tests += 1;
        if test.matches {
            self.passed_tests += 1;
        } else {
            self.failed_tests += 1;
        }
        self.tests.push(test);
    }

    /// Check if all tests passed
    pub fn all_passed(&self) -> bool {
        self.failed_tests == 0
    }
}

/// Individual interpolation test result
#[derive(Debug, Clone)]
pub struct InterpolationTest {
    /// Template that was tested
    pub template: String,

    /// Original interpolation result
    pub original_result: String,

    /// Current interpolation result
    pub current_result: String,

    /// Whether results match
    pub matches: bool,

    /// When the test was performed
    pub interpolated_at: DateTime<Utc>,
}

/// Manager for variable resume operations
pub struct VariableResumeManager {
    /// Variable store for restoration
    variable_store: Option<VariableStore>,
}

impl VariableResumeManager {
    /// Create new variable resume manager
    pub fn new() -> Self {
        Self {
            variable_store: None,
        }
    }

    /// Create variable checkpoint state from current context
    pub fn create_checkpoint(
        &self,
        variables: &HashMap<String, String>,
        captured_outputs: &HashMap<String, String>,
        iteration_vars: &HashMap<String, String>,
        _variable_store: &VariableStore,
    ) -> Result<VariableCheckpointState> {
        // Convert string variables to Value
        let mut global_variables = HashMap::new();
        for (key, value) in variables {
            global_variables.insert(key.clone(), Value::String(value.clone()));
        }

        // Add captured outputs
        for (key, value) in captured_outputs {
            global_variables.insert(key.clone(), Value::String(value.clone()));
        }

        // Create environment snapshot
        let environment_snapshot = EnvironmentSnapshot {
            variables: std::env::vars().collect(),
            captured_at: Utc::now(),
            hostname: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            working_directory: std::env::current_dir()?,
        };

        // Create metadata
        let variable_metadata = VariableMetadata {
            total_variables: global_variables.len(),
            computed_variables: 0,
            total_interpolations: 0,
            checkpoint_version: "1.0.0".to_string(),
        };

        Ok(VariableCheckpointState {
            global_variables,
            phase_variables: HashMap::new(),
            computed_cache: HashMap::new(),
            environment_snapshot,
            interpolation_history: Vec::new(),
            variable_metadata,
            captured_outputs: captured_outputs.clone(),
            iteration_vars: iteration_vars.clone(),
        })
    }

    /// Restore variables from checkpoint state
    pub fn restore_from_checkpoint(
        &self,
        state: &VariableCheckpointState,
    ) -> Result<(HashMap<String, String>, HashMap<String, String>, HashMap<String, String>)> {
        let mut variables = HashMap::new();
        let mut captured_outputs = HashMap::new();
        let iteration_vars = state.iteration_vars.clone();

        // Restore global variables
        for (key, value) in &state.global_variables {
            if let Value::String(s) = value {
                variables.insert(key.clone(), s.clone());
            } else {
                variables.insert(key.clone(), value.to_string());
            }
        }

        // Restore captured outputs
        for (key, value) in &state.captured_outputs {
            captured_outputs.insert(key.clone(), value.clone());
        }

        Ok((variables, captured_outputs, iteration_vars))
    }

    /// Validate environment compatibility
    pub fn validate_environment(
        &self,
        saved_snapshot: &EnvironmentSnapshot,
    ) -> Result<EnvironmentCompatibility> {
        let mut compatibility = EnvironmentCompatibility::new();
        let current_env: HashMap<String, String> = std::env::vars().collect();

        // Check for missing or changed variables
        for (key, saved_value) in &saved_snapshot.variables {
            // Skip internal/system variables
            if key.starts_with("PRODIGY_") || key.starts_with("_") {
                continue;
            }

            match current_env.get(key) {
                Some(current_value) if current_value != saved_value => {
                    compatibility.add_changed_variable(
                        key.clone(),
                        saved_value.clone(),
                        current_value.clone(),
                    );
                }
                None => {
                    // Only warn about missing variables that seem important
                    if !key.starts_with("RUST_") && !key.contains("TEMP") && !key.contains("TMP") {
                        compatibility.add_missing_variable(key.clone(), saved_value.clone());
                    }
                }
                _ => {} // Variable matches
            }
        }

        // Check for new variables
        for (key, current_value) in &current_env {
            if !saved_snapshot.variables.contains_key(key) && !key.starts_with("PRODIGY_") {
                compatibility.add_new_variable(key.clone(), current_value.clone());
            }
        }

        // Determine compatibility
        compatibility.is_compatible = !compatibility.has_critical_changes();

        Ok(compatibility)
    }

    /// Recalculate MapReduce variables from job state
    pub fn recalculate_mapreduce_variables(
        &self,
        total_items: usize,
        successful_items: usize,
        failed_items: usize,
    ) -> HashMap<String, String> {
        let mut variables = HashMap::new();

        // Set aggregate variables
        variables.insert("map.total".to_string(), total_items.to_string());
        variables.insert("map.successful".to_string(), successful_items.to_string());
        variables.insert("map.failed".to_string(), failed_items.to_string());
        variables.insert("map.completed".to_string(), (successful_items + failed_items).to_string());

        // Calculate success rate
        let success_rate = if total_items > 0 {
            (successful_items as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };
        variables.insert("map.success_rate".to_string(), format!("{:.2}", success_rate));

        variables
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_compatibility() {
        let mut compatibility = EnvironmentCompatibility::new();

        // Add some changes
        compatibility.add_missing_variable("API_KEY".to_string(), "secret".to_string());
        compatibility.add_changed_variable(
            "ENV".to_string(),
            "production".to_string(),
            "development".to_string(),
        );

        // Should have critical changes
        assert!(compatibility.has_critical_changes());
        assert!(!compatibility.is_compatible);
    }

    #[test]
    fn test_mapreduce_variable_recalculation() {
        let manager = VariableResumeManager::new();
        let vars = manager.recalculate_mapreduce_variables(10, 7, 3);

        assert_eq!(vars.get("map.total").unwrap(), "10");
        assert_eq!(vars.get("map.successful").unwrap(), "7");
        assert_eq!(vars.get("map.failed").unwrap(), "3");
        assert_eq!(vars.get("map.completed").unwrap(), "10");
        assert_eq!(vars.get("map.success_rate").unwrap(), "70.00");
    }

    #[test]
    fn test_interpolation_test_results() {
        let mut results = InterpolationTestResults::new();

        results.add_test(InterpolationTest {
            template: "${item}".to_string(),
            original_result: "test.txt".to_string(),
            current_result: "test.txt".to_string(),
            matches: true,
            interpolated_at: Utc::now(),
        });

        results.add_test(InterpolationTest {
            template: "${map.total}".to_string(),
            original_result: "10".to_string(),
            current_result: "0".to_string(),
            matches: false,
            interpolated_at: Utc::now(),
        });

        assert_eq!(results.total_tests, 2);
        assert_eq!(results.passed_tests, 1);
        assert_eq!(results.failed_tests, 1);
        assert!(!results.all_passed());
    }
}