//! Main dry-run validator for MapReduce workflows
//!
//! Coordinates validation across all phases and components of a MapReduce workflow.

use super::command_validator::CommandValidator;
use super::input_validator::InputValidator;
use super::resource_estimator::ResourceEstimator;
use super::types::*;
use super::variable_processor::VariableProcessor;
use crate::cook::execution::mapreduce::{MapPhase, ReducePhase, SetupPhase};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info};

/// Main validator for dry-run mode
pub struct DryRunValidator {
    input_validator: InputValidator,
    command_validator: CommandValidator,
    resource_estimator: ResourceEstimator,
    variable_processor: VariableProcessor,
}

impl DryRunValidator {
    /// Create a new dry-run validator
    pub fn new() -> Self {
        Self {
            input_validator: InputValidator::new(),
            command_validator: CommandValidator::new(),
            resource_estimator: ResourceEstimator::new(),
            variable_processor: VariableProcessor::new(),
        }
    }

    /// Validate an entire MapReduce workflow with phases directly
    pub async fn validate_workflow_phases(
        &self,
        setup_phase: Option<SetupPhase>,
        map_phase: MapPhase,
        reduce_phase: Option<ReducePhase>,
    ) -> Result<DryRunReport, DryRunError> {
        info!("Starting dry-run validation");

        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Validate setup phase if present
        let setup_validation = if let Some(setup) = &setup_phase {
            Some(self.validate_setup_phase(setup, &mut warnings, &mut errors)?)
        } else {
            None
        };

        // Validate and load work items for map phase
        let (work_items, _input_validation) = self
            .validate_and_load_input(&map_phase, &mut warnings, &mut errors)
            .await?;

        // Validate map phase
        let map_validation =
            self.validate_map_phase(&map_phase, &work_items, &mut warnings, &mut errors)?;

        // Validate reduce phase if present
        let reduce_validation = if let Some(reduce) = &reduce_phase {
            Some(self.validate_reduce_phase(reduce, &mut warnings, &mut errors)?)
        } else {
            None
        };

        // Create work item preview
        let work_item_preview = self.create_work_item_preview(&map_phase, &work_items)?;

        // Estimate resources
        let resource_estimates = self.resource_estimator.estimate_resources(
            &map_phase,
            &work_items,
            setup_phase.as_ref(),
            reduce_phase.as_ref(),
        );

        // Process variables
        let variable_preview = self.variable_processor.create_preview(
            &map_phase,
            &work_items,
            setup_phase.as_ref(),
            reduce_phase.as_ref(),
        )?;

        // Calculate total estimated duration
        let estimated_duration = self.calculate_total_duration(
            setup_validation.as_ref(),
            &map_validation,
            reduce_validation.as_ref(),
        );

        // Determine overall validity
        let is_valid = errors.is_empty()
            && setup_validation.as_ref().map_or(true, |v| v.valid)
            && map_validation.valid
            && reduce_validation.as_ref().map_or(true, |v| v.valid);

        let validation_results = ValidationResults {
            setup_phase: setup_validation,
            map_phase: map_validation,
            reduce_phase: reduce_validation,
            is_valid,
        };

        Ok(DryRunReport {
            validation_results,
            work_item_preview,
            resource_estimates,
            variable_preview,
            warnings,
            errors,
            estimated_duration,
        })
    }

    /// Validate setup phase
    fn validate_setup_phase(
        &self,
        setup: &SetupPhase,
        warnings: &mut Vec<ValidationWarning>,
        errors: &mut Vec<ValidationError>,
    ) -> Result<PhaseValidation, DryRunError> {
        debug!("Validating setup phase");

        let mut issues = Vec::new();
        let mut all_valid = true;

        // Validate each command in setup
        for (idx, command) in setup.commands.iter().enumerate() {
            let validation = self.command_validator.validate_command(command);

            if !validation.valid {
                all_valid = false;
                errors.push(ValidationError {
                    phase: "setup".to_string(),
                    message: format!("Command {} is invalid: {:?}", idx + 1, validation.issues),
                });
            }

            // Add issues from command validation
            issues.extend(validation.issues.clone());

            // Check for warnings
            for issue in &validation.issues {
                match issue {
                    ValidationIssue::Warning(msg) => warnings.push(ValidationWarning {
                        phase: "setup".to_string(),
                        message: msg.clone(),
                    }),
                    ValidationIssue::Error(msg) => {
                        if validation.valid {
                            // Only add as error if not already marked invalid
                            warnings.push(ValidationWarning {
                                phase: "setup".to_string(),
                                message: msg.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(PhaseValidation {
            valid: all_valid,
            command_count: setup.commands.len(),
            estimated_duration: Duration::from_secs(setup.timeout),
            dependencies_met: true, // Would need actual dependency checking
            issues,
        })
    }

    /// Validate and load input data
    async fn validate_and_load_input(
        &self,
        map_phase: &MapPhase,
        warnings: &mut Vec<ValidationWarning>,
        errors: &mut Vec<ValidationError>,
    ) -> Result<(Vec<Value>, InputValidation), DryRunError> {
        debug!("Validating input source");

        let input_validation = self
            .input_validator
            .validate_input_source(&map_phase.config.input)
            .await?;

        if !input_validation.valid {
            errors.push(ValidationError {
                phase: "map".to_string(),
                message: format!("Invalid input source: {}", map_phase.config.input),
            });
            return Err(DryRunError::InputError("Invalid input source".to_string()));
        }

        // Load work items
        let work_items = self
            .input_validator
            .load_work_items(&map_phase.config.input, map_phase.json_path.as_deref())
            .await?;

        if work_items.is_empty() {
            warnings.push(ValidationWarning {
                phase: "map".to_string(),
                message: "No work items found in input source".to_string(),
            });
        }

        Ok((work_items, input_validation))
    }

    /// Validate map phase
    fn validate_map_phase(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
        warnings: &mut Vec<ValidationWarning>,
        errors: &mut Vec<ValidationError>,
    ) -> Result<PhaseValidation, DryRunError> {
        debug!("Validating map phase");

        let mut issues = Vec::new();
        let mut all_valid = true;

        // Validate agent template commands
        for (idx, command) in map_phase.agent_template.iter().enumerate() {
            let validation = self.command_validator.validate_command(command);

            if !validation.valid {
                all_valid = false;
                errors.push(ValidationError {
                    phase: "map".to_string(),
                    message: format!(
                        "Agent template command {} is invalid: {:?}",
                        idx + 1,
                        validation.issues
                    ),
                });
            }

            issues.extend(validation.issues);
        }

        // Validate max_parallel setting
        if map_phase.config.max_parallel == 0 {
            errors.push(ValidationError {
                phase: "map".to_string(),
                message: "max_parallel must be greater than 0".to_string(),
            });
            all_valid = false;
        }

        // Warn if max_parallel is very high
        if map_phase.config.max_parallel > 50 {
            warnings.push(ValidationWarning {
                phase: "map".to_string(),
                message: format!(
                    "max_parallel is set to {}, which may consume significant resources",
                    map_phase.config.max_parallel
                ),
            });
        }

        // Estimate duration based on work items and parallelism
        let avg_item_duration = Duration::from_secs(60); // Estimated 1 minute per item
        let map_duration = Duration::from_secs(
            (work_items.len() as u64 * avg_item_duration.as_secs())
                / map_phase.config.max_parallel.max(1) as u64,
        );

        Ok(PhaseValidation {
            valid: all_valid,
            command_count: map_phase.agent_template.len(),
            estimated_duration: map_duration,
            dependencies_met: true,
            issues,
        })
    }

    /// Validate reduce phase
    fn validate_reduce_phase(
        &self,
        reduce: &ReducePhase,
        _warnings: &mut Vec<ValidationWarning>,
        errors: &mut Vec<ValidationError>,
    ) -> Result<PhaseValidation, DryRunError> {
        debug!("Validating reduce phase");

        let mut issues = Vec::new();
        let mut all_valid = true;

        // Validate each command in reduce
        for (idx, command) in reduce.commands.iter().enumerate() {
            let validation = self.command_validator.validate_command(command);

            if !validation.valid {
                all_valid = false;
                errors.push(ValidationError {
                    phase: "reduce".to_string(),
                    message: format!("Command {} is invalid: {:?}", idx + 1, validation.issues),
                });
            }

            issues.extend(validation.issues);
        }

        let estimated_duration = reduce
            .timeout_secs
            .map(Duration::from_secs)
            .unwrap_or_else(|| Duration::from_secs(reduce.commands.len() as u64 * 10));

        Ok(PhaseValidation {
            valid: all_valid,
            command_count: reduce.commands.len(),
            estimated_duration,
            dependencies_met: true,
            issues,
        })
    }

    /// Create work item preview
    fn create_work_item_preview(
        &self,
        map_phase: &MapPhase,
        work_items: &[Value],
    ) -> Result<WorkItemPreview, DryRunError> {
        let sample_size = map_phase.max_items.unwrap_or(10);
        let sample_items: Vec<Value> = work_items.iter().take(sample_size).cloned().collect();

        // Calculate distribution across agents
        let mut distribution = HashMap::new();
        let agents = map_phase.config.max_parallel;
        let items_per_agent = (work_items.len() + agents - 1) / agents;

        for agent_id in 0..agents {
            let start = agent_id * items_per_agent;
            let end = ((agent_id + 1) * items_per_agent).min(work_items.len());
            if start < work_items.len() {
                distribution.insert(agent_id, end - start);
            }
        }

        Ok(WorkItemPreview {
            total_count: work_items.len(),
            sample_items,
            distribution,
            filtered_count: None, // Would need actual filtering logic
            sort_description: map_phase.sort_by.clone(),
        })
    }

    /// Calculate total duration from phase validations
    fn calculate_total_duration(
        &self,
        setup: Option<&PhaseValidation>,
        map: &PhaseValidation,
        reduce: Option<&PhaseValidation>,
    ) -> Duration {
        let setup_duration = setup
            .map(|v| v.estimated_duration)
            .unwrap_or(Duration::ZERO);
        let reduce_duration = reduce
            .map(|v| v.estimated_duration)
            .unwrap_or(Duration::ZERO);

        setup_duration + map.estimated_duration + reduce_duration
    }
}

impl Default for DryRunValidator {
    fn default() -> Self {
        Self::new()
    }
}
