---
number: 74
title: Variable Substitution During Resume
category: foundation
priority: high
status: draft
dependencies: [61, 73]
created: 2025-01-16
---

# Specification 74: Variable Substitution During Resume

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [61 - Enhanced Variable Interpolation System, 73 - MapReduce Resume Functionality]

## Context

Variable substitution during workflow resume is currently unclear and untested. Tests exist but don't actually verify if variables are preserved and correctly used during resumed execution. This creates uncertainty about whether variables like `${map.successful}`, `${shell.output}`, and custom variables maintain their values across resume operations, potentially leading to incorrect workflow behavior after interruption.

Key issues with current variable handling during resume:
1. Variable state persistence in checkpoints is incomplete
2. Variable interpolation during resume is not validated
3. Cross-phase variable passing after resume is untested
4. MapReduce aggregate variables may be recalculated incorrectly
5. Environment variable changes between runs are not handled

## Objective

Implement comprehensive variable substitution support during resume operations, ensuring all variable types are properly preserved, restored, and interpolated during resumed workflow execution with full test coverage and validation.

## Requirements

### Functional Requirements

1. **Variable State Persistence**
   - Save all variable contexts to checkpoints
   - Persist computed variable results and cache state
   - Store variable interpolation history
   - Maintain variable scope boundaries
   - Preserve variable correlation IDs

2. **Variable Restoration During Resume**
   - Restore variable contexts from checkpoints
   - Rebuild variable interpolation engines
   - Validate variable consistency
   - Handle environment variable changes
   - Restore computed variable cache state

3. **Cross-Phase Variable Continuity**
   - Maintain variables across resume boundaries
   - Preserve setup phase variables for map/reduce
   - Restore MapReduce aggregate variables correctly
   - Handle partial phase completion variables
   - Ensure variable scope integrity after resume

4. **MapReduce Variable Handling**
   - Correctly restore `${map.total}`, `${map.successful}`, etc.
   - Recalculate aggregate variables based on checkpoint state
   - Handle partially completed map phases
   - Maintain item-level variable contexts
   - Preserve agent-specific variables

5. **Environment Variable Management**
   - Detect environment changes between runs
   - Handle missing environment variables gracefully
   - Provide environment variable migration strategies
   - Validate environment variable consistency
   - Support environment variable overrides during resume

6. **Variable Validation and Testing**
   - Comprehensive test coverage for variable preservation
   - Validation of variable interpolation results
   - Test all variable types during resume scenarios
   - Performance testing for large variable contexts
   - Edge case handling for variable scenarios

### Non-Functional Requirements

1. **Reliability**
   - Variables maintain consistent values across resume
   - No variable data loss during interruption/resume cycles
   - Deterministic variable interpolation results
   - Robust handling of variable reference chains

2. **Performance**
   - Fast variable context restoration (< 5 seconds)
   - Efficient variable cache reconstruction
   - Minimal memory overhead for variable persistence
   - Scalable to workflows with hundreds of variables

3. **Debuggability**
   - Clear variable interpolation tracing across resume
   - Variable state diff between original and resumed execution
   - Detailed logging of variable restoration operations
   - Variable interpolation debugging tools

## Acceptance Criteria

- [ ] All variable types are preserved accurately across resume operations
- [ ] Variable interpolation produces identical results before and after resume
- [ ] MapReduce aggregate variables are correctly recalculated from checkpoint state
- [ ] Environment variable changes are detected and handled appropriately
- [ ] Cross-phase variable passing works correctly after resume
- [ ] Computed variables are restored with proper cache state
- [ ] Variable scope boundaries are maintained after resume
- [ ] Large variable contexts (100+ variables) restore within 5 seconds
- [ ] Comprehensive test coverage validates all variable scenarios
- [ ] Variable interpolation debugging tools work correctly

## Technical Details

### Implementation Approach

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableCheckpointState {
    pub global_variables: HashMap<String, Variable>,
    pub phase_variables: HashMap<String, HashMap<String, Variable>>,
    pub computed_cache: HashMap<String, CachedValue>,
    pub environment_snapshot: EnvironmentSnapshot,
    pub interpolation_history: Vec<InterpolationRecord>,
    pub variable_metadata: VariableMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedValue {
    pub value: Value,
    pub computed_at: DateTime<Utc>,
    pub cache_key: String,
    pub dependencies: Vec<String>,
    pub is_expensive: bool,
}

#[derive(Debug, Clone)]
pub struct VariableResumeManager {
    checkpoint_state: VariableCheckpointState,
    context_builder: Arc<VariableContextBuilder>,
    interpolator: Arc<VariableInterpolator>,
    validator: Arc<VariableValidator>,
}

impl VariableResumeManager {
    pub async fn restore_variable_context(
        &self,
        checkpoint: &WorkflowCheckpoint,
        current_environment: &Environment,
    ) -> Result<VariableContext> {
        // Load variable state from checkpoint
        let saved_state = checkpoint.variable_checkpoint_state
            .as_ref()
            .ok_or(ProdigyError::MissingVariableState)?;

        // Validate environment consistency
        self.validate_environment_compatibility(
            &saved_state.environment_snapshot,
            current_environment,
        ).await?;

        // Rebuild variable contexts
        let mut context = VariableContext::new();

        // Restore global variables
        for (name, variable) in &saved_state.global_variables {
            context.set_global(name.clone(), variable.clone())?;
        }

        // Restore phase-specific variables
        for (phase, variables) in &saved_state.phase_variables {
            for (name, variable) in variables {
                context.set_phase_variable(phase, name.clone(), variable.clone())?;
            }
        }

        // Restore computed variable cache
        self.restore_computed_cache(&mut context, &saved_state.computed_cache).await?;

        // Validate variable consistency
        self.validator.validate_restored_context(&context).await?;

        Ok(context)
    }

    pub async fn restore_mapreduce_variables(
        &self,
        job_state: &MapReduceResumeState,
        context: &mut VariableContext,
    ) -> Result<()> {
        // Recalculate aggregate variables based on actual completion state
        let total_items = job_state.original_total_items;
        let completed_items = job_state.completed_items.len();
        let failed_items = job_state.failed_items.len();
        let successful_items = completed_items - failed_items;

        // Set accurate MapReduce aggregate variables
        context.set_global("map.total".to_string(), Variable::Static(total_items.into()))?;
        context.set_global("map.successful".to_string(), Variable::Static(successful_items.into()))?;
        context.set_global("map.failed".to_string(), Variable::Static(failed_items.into()))?;
        context.set_global("map.completed".to_string(), Variable::Static(completed_items.into()))?;

        // Calculate success rate
        let success_rate = if total_items > 0 {
            (successful_items as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };
        context.set_global("map.success_rate".to_string(), Variable::Static(success_rate.into()))?;

        // Restore map results if available
        if let Some(results) = &job_state.phase_results.get(&MapReducePhase::Map) {
            context.set_global("map.results".to_string(), Variable::Static(results.output.clone().unwrap_or_default()))?;
        }

        Ok(())
    }

    async fn validate_environment_compatibility(
        &self,
        saved_snapshot: &EnvironmentSnapshot,
        current_env: &Environment,
    ) -> Result<EnvironmentCompatibility> {
        let mut compatibility = EnvironmentCompatibility::new();

        // Check for missing environment variables
        for (key, saved_value) in &saved_snapshot.variables {
            match current_env.get(key) {
                Some(current_value) if current_value == saved_value => {
                    // Variable matches
                }
                Some(current_value) => {
                    compatibility.add_changed_variable(key.clone(), saved_value.clone(), current_value.clone());
                }
                None => {
                    compatibility.add_missing_variable(key.clone(), saved_value.clone());
                }
            }
        }

        // Check for new environment variables
        for (key, current_value) in current_env.iter() {
            if !saved_snapshot.variables.contains_key(key) {
                compatibility.add_new_variable(key.clone(), current_value.clone());
            }
        }

        // Determine if resume is safe
        if compatibility.has_critical_changes() {
            return Err(ProdigyError::EnvironmentMismatch(compatibility));
        }

        Ok(compatibility)
    }

    pub async fn test_variable_interpolation(
        &self,
        context: &VariableContext,
        original_interpolations: &[InterpolationRecord],
    ) -> Result<InterpolationTestResults> {
        let mut results = InterpolationTestResults::new();

        for record in original_interpolations {
            // Re-interpolate the same template
            let current_result = self.interpolator.interpolate(&record.template, context)?;

            // Compare with original result
            let test_result = InterpolationTest {
                template: record.template.clone(),
                original_result: record.result.clone(),
                current_result: current_result.clone(),
                matches: record.result == current_result,
                interpolated_at: Utc::now(),
            };

            results.add_test(test_result);
        }

        Ok(results)
    }
}
```

### Architecture Changes

1. **Variable Checkpoint System**
   - Enhanced `VariableCheckpointState` for comprehensive variable persistence
   - Variable interpolation history tracking
   - Environment variable snapshot management

2. **Resume-Aware Variable Context**
   - Variable context restoration from checkpoints
   - Environment compatibility validation
   - Variable consistency checking

3. **MapReduce Variable Recalculation**
   - Aggregate variable recalculation based on actual state
   - Partial completion handling
   - Cross-agent variable synchronization

4. **Variable Testing Framework**
   - Comprehensive variable interpolation testing
   - Variable state validation tools
   - Performance testing for variable operations

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationRecord {
    pub template: String,
    pub result: String,
    pub interpolated_at: DateTime<Utc>,
    pub variable_dependencies: Vec<String>,
    pub phase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSnapshot {
    pub variables: HashMap<String, String>,
    pub captured_at: DateTime<Utc>,
    pub hostname: String,
    pub working_directory: PathBuf,
}

#[derive(Debug, Clone)]
pub struct EnvironmentCompatibility {
    pub missing_variables: HashMap<String, String>,
    pub changed_variables: HashMap<String, (String, String)>, // (old, new)
    pub new_variables: HashMap<String, String>,
    pub is_compatible: bool,
}

#[derive(Debug, Clone)]
pub struct InterpolationTestResults {
    pub tests: Vec<InterpolationTest>,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub test_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct InterpolationTest {
    pub template: String,
    pub original_result: String,
    pub current_result: String,
    pub matches: bool,
    pub interpolated_at: DateTime<Utc>,
}
```

### Integration Points

1. **Checkpoint Manager Integration**
   - Save/load variable checkpoint state
   - Variable state validation
   - Cross-phase variable management

2. **Workflow Executor Integration**
   - Variable context restoration during resume
   - Variable interpolation testing
   - Environment compatibility checking

3. **MapReduce Executor Integration**
   - Aggregate variable recalculation
   - Cross-agent variable synchronization
   - Item-level variable context restoration

## Dependencies

- **Prerequisites**: [61 - Enhanced Variable Interpolation System, 73 - MapReduce Resume Functionality]
- **Affected Components**:
  - `src/cook/workflow/variables.rs`
  - `src/cook/execution/interpolation.rs`
  - `src/cook/workflow/checkpoint.rs`
  - `src/cook/execution/mapreduce.rs`
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Variable state persistence and restoration
  - Environment compatibility validation
  - Variable interpolation consistency
  - MapReduce variable recalculation

- **Integration Tests**:
  - End-to-end variable preservation across resume
  - Cross-phase variable passing after resume
  - Complex variable interpolation scenarios
  - Environment change handling

- **Property Tests**:
  - Variable interpolation determinism
  - Variable state consistency
  - Environment snapshot accuracy

- **Performance Tests**:
  - Variable context restoration speed
  - Large variable set handling
  - Memory usage during variable operations

## Documentation Requirements

- **Code Documentation**:
  - Variable resume architecture
  - Environment compatibility checking
  - Variable testing framework

- **User Documentation**:
  - Variable behavior during resume
  - Environment variable best practices
  - Debugging variable issues

- **Architecture Updates**:
  - Variable persistence mechanisms
  - Resume variable flow
  - Testing strategy documentation

## Implementation Notes

1. **Deterministic Behavior**: Ensure variable interpolation is deterministic across resume
2. **Environment Flexibility**: Handle environment changes gracefully with clear user feedback
3. **Performance**: Optimize variable context restoration for large workflows
4. **Testing**: Comprehensive test coverage for all variable scenarios
5. **Debugging**: Rich debugging tools for variable-related issues

## Migration and Compatibility

- Backward compatible with existing variable implementations
- Automatic migration of legacy variable state
- Graceful handling of missing variable data in old checkpoints
- Progressive rollout with comprehensive testing