---
number: 65
title: MapReduce Setup Phase Implementation
category: foundation
priority: high
status: draft
dependencies: [58]
created: 2025-01-14
---

# Specification 65: MapReduce Setup Phase Implementation

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [58 - Command Output Input]

## Context

The whitepaper prominently features an optional setup phase for MapReduce workflows:
```yaml
setup:
  - shell: "analyze-codebase --output work-items.json"
  - shell: "generate-work-items.sh"
```

This setup phase runs before the map phase to prepare data, analyze the codebase, or generate work items dynamically. Currently, this critical feature is missing, forcing users to manually prepare data before running MapReduce workflows.

## Objective

Implement the setup phase for MapReduce workflows, enabling dynamic work item generation, codebase analysis, and environment preparation before parallel execution begins.

## Requirements

### Functional Requirements
- Execute setup commands sequentially before map phase
- Setup phase can generate the input file for map phase
- Access to setup phase outputs in map phase via variables
- Support multiple setup commands in sequence
- Setup failure prevents map phase execution
- Setup commands run in main worktree (not parallel)
- Variable capture from setup commands
- Support both shell and Claude commands in setup

### Non-Functional Requirements
- Clear separation between setup and map phases
- Setup phase timeout configuration
- Efficient handoff from setup to map phase
- Clear progress indication for setup phase

## Acceptance Criteria

- [ ] Setup phase executes before map phase
- [ ] Setup can generate `work-items.json` used by map phase
- [ ] Multiple setup commands execute in order
- [ ] Setup failure stops workflow execution
- [ ] Variables from setup available in map phase
- [ ] Progress shows "Setup phase" distinctly
- [ ] Setup runs in main worktree, not parallel worktrees
- [ ] Setup phase can modify workspace for map phase
- [ ] Claude commands work in setup phase
- [ ] Setup timeout configurable separately from map timeout

## Technical Details

### Implementation Approach

1. **Enhanced MapReduce Configuration**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct MapReduceWorkflow {
       pub name: String,

       /// Optional setup phase
       #[serde(skip_serializing_if = "Option::is_none")]
       pub setup: Option<SetupPhase>,

       /// Map phase configuration
       pub map: MapPhase,

       /// Optional reduce phase
       #[serde(skip_serializing_if = "Option::is_none")]
       pub reduce: Option<ReducePhase>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct SetupPhase {
       /// Commands to execute in setup
       pub commands: Vec<WorkflowStep>,

       /// Timeout for entire setup phase
       #[serde(with = "duration_serde", default = "default_setup_timeout")]
       pub timeout: Duration,

       /// Variables to capture from setup
       #[serde(default)]
       pub capture_outputs: Vec<String>,

       /// Working directory for setup
       #[serde(skip_serializing_if = "Option::is_none")]
       pub working_dir: Option<PathBuf>,
   }
   ```

2. **Setup Phase Executor**:
   ```rust
   pub struct SetupPhaseExecutor {
       command_executor: CommandExecutor,
       variable_manager: VariableManager,
   }

   impl SetupPhaseExecutor {
       pub async fn execute(
           &self,
           setup: &SetupPhase,
           context: &mut ExecutionContext,
       ) -> Result<SetupResult> {
           info!("Starting setup phase with {} commands", setup.commands.len());

           let start_time = Instant::now();
           let timeout_future = tokio::time::sleep(setup.timeout);
           tokio::pin!(timeout_future);

           for (idx, command) in setup.commands.iter().enumerate() {
               // Check timeout
               if timeout_future.is_elapsed() {
                   return Err(Error::SetupTimeout);
               }

               // Execute command in main worktree
               info!("Setup step {}/{}: {}", idx + 1, setup.commands.len(), command.name());

               let result = tokio::select! {
                   res = self.execute_command(command, context) => res?,
                   _ = &mut timeout_future => {
                       return Err(Error::SetupTimeout);
                   }
               };

               // Capture outputs if requested
               if let Some(capture_name) = command.capture_as() {
                   context.variables.insert(
                       capture_name.clone(),
                       result.output.clone(),
                   );
               }

               // Check for required output files
               if let Some(output_file) = command.creates_file() {
                   if !output_file.exists() {
                       return Err(Error::SetupFileMissing(output_file));
                   }
               }
           }

           Ok(SetupResult {
               duration: start_time.elapsed(),
               variables: context.variables.clone(),
               created_files: self.detect_created_files()?,
           })
       }

       async fn execute_command(
           &self,
           command: &WorkflowStep,
           context: &mut ExecutionContext,
       ) -> Result<CommandResult> {
           match &command.command_type {
               CommandType::Shell(cmd) => {
                   self.command_executor.execute_shell(cmd, context).await
               }
               CommandType::Claude(cmd) => {
                   self.command_executor.execute_claude(cmd, context).await
               }
               _ => Err(Error::UnsupportedSetupCommand),
           }
       }
   }
   ```

3. **Integration with MapReduce Executor**:
   ```rust
   impl MapReduceExecutor {
       pub async fn execute(
           &self,
           workflow: &MapReduceWorkflow,
           context: &mut ExecutionContext,
       ) -> Result<MapReduceResult> {
           // Execute setup phase if present
           let setup_result = if let Some(setup) = &workflow.setup {
               let executor = SetupPhaseExecutor::new();
               Some(executor.execute(setup, context).await?)
           } else {
               None
           };

           // Check if setup generated the input file
           if let Some(result) = &setup_result {
               if let Some(generated_input) = result.created_files
                   .iter()
                   .find(|f| f.ends_with("work-items.json"))
               {
                   // Update map phase to use generated input
                   let mut map_phase = workflow.map.clone();
                   map_phase.input = generated_input.clone();
               }
           }

           // Continue with map phase
           self.execute_map_phase(&workflow.map, context).await?;

           // Execute reduce phase if present
           if let Some(reduce) = &workflow.reduce {
               self.execute_reduce_phase(reduce, context).await?;
           }

           Ok(MapReduceResult {
               setup_result,
               map_results: self.map_results,
               reduce_result: self.reduce_result,
           })
       }
   }
   ```

### Architecture Changes
- Add `SetupPhase` to MapReduce configuration
- Create `SetupPhaseExecutor` component
- Modify `MapReduceExecutor` to handle setup phase
- Update progress tracking to show setup phase
- Enhance variable passing between phases

### Data Structures
```yaml
# Example with setup phase
name: analyze-and-fix-tech-debt
mode: mapreduce

setup:
  - shell: "npm run build:analyze"
    capture_as: "build_metrics"
  - shell: "debtmap analyze src --output debt-analysis.json"
  - claude: "/generate-fix-strategy ${build_metrics}"
    creates_file: "fix-strategy.json"

map:
  input: "debt-analysis.json"  # Generated by setup
  json_path: "$.high_priority_items[*]"
  agent_template:
    commands:
      - claude: "/fix-tech-debt --strategy fix-strategy.json --item '${item}'"
  max_parallel: 10

reduce:
  commands:
    - shell: "debtmap analyze src --output debt-after.json"
    - claude: "/generate-improvement-report debt-analysis.json debt-after.json"
```

## Dependencies

- **Prerequisites**: [58 - Command Output Input] for dynamic input generation
- **Affected Components**:
  - `src/config/mapreduce.rs` - Setup phase configuration
  - `src/cook/execution/mapreduce.rs` - Setup phase execution
  - `src/cook/workflow/` - Integration with workflow system
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Setup phase configuration parsing
  - Variable capture from setup commands
  - File creation detection
  - Timeout handling
- **Integration Tests**:
  - Full MapReduce with setup phase
  - Setup generating input for map phase
  - Setup failure preventing map execution
  - Variable passing between phases
- **End-to-End Tests**:
  - Real codebase analysis in setup
  - Dynamic work item generation
  - Multi-command setup sequences

## Documentation Requirements

- **Code Documentation**: Document setup phase execution flow
- **User Documentation**:
  - Setup phase usage guide
  - Common setup patterns
  - Dynamic work item generation examples
- **Architecture Updates**: Add setup phase to MapReduce flow diagram

## Implementation Notes

- Setup phase always runs in main worktree for consistency
- Consider caching setup results for resume scenarios
- Support dry-run to preview setup commands
- Clear logging of files created during setup
- Future: Support parallel setup commands where safe

## Migration and Compatibility

- MapReduce workflows without setup continue to work
- No breaking changes to existing workflows
- Setup phase is purely additive
- Clear examples for migrating manual setup to automated