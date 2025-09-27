---
number: 110
title: MapReduce Dry-Run Mode Support
category: testing
priority: critical
status: draft
dependencies: []
created: 2025-09-27
---

# Specification 110: MapReduce Dry-Run Mode Support

## Context

The current MapReduce implementation lacks dry-run mode support, making it impossible to validate workflows, test configurations, or preview operations without executing actual commands. This creates significant risks when testing new workflows or validating changes to existing ones.

Current gaps:
- No dry-run mode in MapReduceCoordinator
- Agent template validation only at execution time
- No preview of work item distribution and processing
- Cannot validate JSONPath expressions without execution
- No way to test resource requirements or limits
- Missing validation of variable interpolation

Users need to be able to run MapReduce workflows in simulation mode to validate configuration, test data processing logic, and estimate resource requirements before committing to full execution.

## Objective

Implement comprehensive dry-run mode support for MapReduce workflows that allows users to validate configurations, preview work distribution, test data transformations, and estimate resource requirements without executing actual commands.

## Requirements

### Functional Requirements

#### Configuration Validation
- Validate all MapReduce configuration parameters
- Verify agent template command syntax and structure
- Test JSONPath expressions against input data
- Validate variable interpolation patterns
- Check resource limit compatibility

#### Work Item Preview
- Show work items that would be processed
- Display work item distribution across agents
- Preview variable interpolation results for sample items
- Show filtered and sorted work item order
- Estimate processing time based on configuration

#### Resource Estimation
- Calculate estimated memory and disk requirements
- Show number of worktrees that would be created
- Estimate network and I/O requirements
- Preview checkpoint storage requirements
- Show parallel execution plan

#### Integration Testing
- Test setup phase commands without execution
- Validate reduce phase configuration
- Test merge workflow if configured
- Verify error handling configuration
- Test DLQ configuration and retry logic

### Non-Functional Requirements
- Dry-run mode should complete within 30 seconds for most workflows
- Memory usage should be minimal compared to actual execution
- Output should be human-readable and actionable
- Support JSON output format for programmatic use
- Compatible with existing CLI flags and options

## Acceptance Criteria

- [ ] `prodigy run --dry-run workflow.yaml` validates entire MapReduce workflow
- [ ] Dry-run mode detects and reports configuration errors
- [ ] Work item preview shows accurate distribution and filtering
- [ ] Variable interpolation preview works for all variable types
- [ ] Resource estimation provides useful metrics
- [ ] JSONPath expressions are validated against actual input data
- [ ] Setup/reduce phase commands are validated without execution
- [ ] Error conditions are properly simulated and reported
- [ ] Performance benchmarks show sub-30-second completion for typical workflows

## Technical Details

### Implementation Approach

#### 1. Dry-Run Execution Mode

Extend the MapReduce coordinator to support dry-run mode:

```rust
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    Normal,
    DryRun {
        show_work_items: bool,
        show_variables: bool,
        show_resources: bool,
        sample_size: Option<usize>,
    },
}

pub struct MapReduceCoordinator {
    // ... existing fields
    execution_mode: ExecutionMode,
    dry_run_validator: DryRunValidator,
}

impl MapReduceCoordinator {
    pub fn with_dry_run_mode(mut self, config: DryRunConfig) -> Self {
        self.execution_mode = ExecutionMode::DryRun {
            show_work_items: config.show_work_items,
            show_variables: config.show_variables,
            show_resources: config.show_resources,
            sample_size: config.sample_size,
        };
        self
    }

    pub async fn execute_dry_run(&self) -> Result<DryRunReport, DryRunError> {
        self.dry_run_validator.validate_workflow(&self.workflow).await
    }
}
```

#### 2. Dry-Run Validator

Create a comprehensive validation service:

```rust
pub struct DryRunValidator {
    input_validator: InputValidator,
    command_validator: CommandValidator,
    resource_estimator: ResourceEstimator,
    variable_processor: VariableProcessor,
}

#[derive(Debug, Serialize)]
pub struct DryRunReport {
    pub validation_results: ValidationResults,
    pub work_item_preview: WorkItemPreview,
    pub resource_estimates: ResourceEstimates,
    pub variable_preview: VariablePreview,
    pub warnings: Vec<ValidationWarning>,
    pub errors: Vec<ValidationError>,
}

#[derive(Debug, Serialize)]
pub struct ValidationResults {
    pub setup_phase: PhaseValidation,
    pub map_phase: PhaseValidation,
    pub reduce_phase: Option<PhaseValidation>,
    pub merge_workflow: Option<PhaseValidation>,
}

#[derive(Debug, Serialize)]
pub struct PhaseValidation {
    pub valid: bool,
    pub command_count: usize,
    pub estimated_duration: Duration,
    pub dependencies_met: bool,
    pub issues: Vec<ValidationIssue>,
}
```

#### 3. Input Data Validation

Validate input sources and JSONPath expressions:

```rust
pub struct InputValidator;

impl InputValidator {
    pub async fn validate_input_source(&self, input: &str) -> Result<InputValidation, ValidationError> {
        match input {
            path if Path::new(path).exists() => self.validate_file_input(path).await,
            cmd if cmd.starts_with("shell:") => self.validate_command_input(cmd).await,
            _ => Err(ValidationError::InvalidInputSource(input.to_string())),
        }
    }

    pub async fn validate_jsonpath(&self, path: &str, sample_data: &Value) -> Result<JsonPathValidation, ValidationError> {
        let selector = JsonPathSelector::compile(path)?;
        let matches = selector.select(sample_data)?;

        Ok(JsonPathValidation {
            path: path.to_string(),
            valid: true,
            match_count: matches.len(),
            sample_matches: matches.into_iter().take(5).collect(),
            data_types: self.analyze_data_types(&matches),
        })
    }

    async fn validate_file_input(&self, path: &str) -> Result<InputValidation, ValidationError> {
        let content = fs::read_to_string(path).await?;
        let data: Value = serde_json::from_str(&content)?;

        Ok(InputValidation {
            source: path.to_string(),
            valid: true,
            size_bytes: content.len(),
            item_count_estimate: self.estimate_item_count(&data),
            data_structure: self.analyze_structure(&data),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct JsonPathValidation {
    pub path: String,
    pub valid: bool,
    pub match_count: usize,
    pub sample_matches: Vec<Value>,
    pub data_types: HashMap<String, usize>,
}
```

#### 4. Command Validation

Validate agent template commands without execution:

```rust
pub struct CommandValidator;

impl CommandValidator {
    pub fn validate_commands(&self, commands: &[WorkflowStep]) -> Vec<CommandValidation> {
        commands.iter().map(|cmd| self.validate_command(cmd)).collect()
    }

    fn validate_command(&self, command: &WorkflowStep) -> CommandValidation {
        let mut validation = CommandValidation {
            command_type: self.get_command_type(command),
            valid: true,
            issues: Vec::new(),
            variable_references: self.extract_variables(command),
            estimated_duration: self.estimate_duration(command),
        };

        // Validate command structure
        self.validate_command_structure(command, &mut validation);

        // Validate variable references
        self.validate_variable_references(command, &mut validation);

        // Validate command syntax
        self.validate_command_syntax(command, &mut validation);

        validation
    }

    fn validate_command_structure(&self, command: &WorkflowStep, validation: &mut CommandValidation) {
        match command {
            WorkflowStep { claude: Some(cmd), .. } => {
                if !cmd.starts_with('/') {
                    validation.issues.push(ValidationIssue::Warning(
                        "Claude command should start with '/'".to_string()
                    ));
                }
            }
            WorkflowStep { shell: Some(cmd), .. } => {
                if cmd.trim().is_empty() {
                    validation.issues.push(ValidationIssue::Error(
                        "Shell command cannot be empty".to_string()
                    ));
                    validation.valid = false;
                }
            }
            _ => {
                validation.issues.push(ValidationIssue::Error(
                    "Command must specify either 'claude' or 'shell'".to_string()
                ));
                validation.valid = false;
            }
        }
    }

    fn extract_variables(&self, command: &WorkflowStep) -> Vec<VariableReference> {
        let mut variables = Vec::new();

        if let Some(cmd) = &command.claude {
            variables.extend(self.extract_from_string(cmd));
        }

        if let Some(cmd) = &command.shell {
            variables.extend(self.extract_from_string(cmd));
        }

        variables
    }

    fn extract_from_string(&self, text: &str) -> Vec<VariableReference> {
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
        re.captures_iter(text)
            .map(|cap| VariableReference {
                name: cap[1].to_string(),
                context: self.determine_context(&cap[1]),
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
pub struct CommandValidation {
    pub command_type: CommandType,
    pub valid: bool,
    pub issues: Vec<ValidationIssue>,
    pub variable_references: Vec<VariableReference>,
    pub estimated_duration: Duration,
}

#[derive(Debug, Serialize)]
pub enum CommandType {
    Claude,
    Shell,
    GoalSeek,
    Foreach,
}

#[derive(Debug, Serialize)]
pub struct VariableReference {
    pub name: String,
    pub context: VariableContext,
}

#[derive(Debug, Serialize)]
pub enum VariableContext {
    Item,
    Map,
    Setup,
    Shell,
    Merge,
    Unknown,
}
```

#### 5. Resource Estimation

Estimate resource requirements for the workflow:

```rust
pub struct ResourceEstimator;

impl ResourceEstimator {
    pub fn estimate_resources(&self, workflow: &MapReduceWorkflow, work_items: &[Value]) -> ResourceEstimates {
        ResourceEstimates {
            memory_usage: self.estimate_memory(workflow, work_items),
            disk_usage: self.estimate_disk(workflow, work_items),
            network_usage: self.estimate_network(workflow),
            worktree_count: workflow.map.max_parallel.min(work_items.len()),
            estimated_duration: self.estimate_duration(workflow, work_items),
            checkpoint_storage: self.estimate_checkpoint_storage(work_items),
        }
    }

    fn estimate_memory(&self, workflow: &MapReduceWorkflow, work_items: &[Value]) -> MemoryEstimate {
        let base_per_agent = 50; // MB base overhead per agent
        let data_per_item = work_items.iter()
            .map(|item| serde_json::to_string(item).unwrap().len())
            .max()
            .unwrap_or(1024); // bytes

        let total_agents = workflow.map.max_parallel;
        let memory_per_agent = base_per_agent + (data_per_item / 1024 / 1024); // Convert to MB

        MemoryEstimate {
            total_mb: total_agents * memory_per_agent,
            per_agent_mb: memory_per_agent,
            peak_concurrent_agents: total_agents,
        }
    }

    fn estimate_duration(&self, workflow: &MapReduceWorkflow, work_items: &[Value]) -> DurationEstimate {
        let setup_duration = workflow.setup.as_ref()
            .map(|s| Duration::from_secs(s.commands.len() as u64 * 30))
            .unwrap_or(Duration::ZERO);

        let avg_item_duration = Duration::from_secs(60); // Estimated 1 minute per item
        let parallel_agents = workflow.map.max_parallel;
        let map_duration = Duration::from_secs(
            (work_items.len() as u64 * avg_item_duration.as_secs()) / parallel_agents as u64
        );

        let reduce_duration = workflow.reduce.as_ref()
            .map(|r| Duration::from_secs(r.commands.len() as u64 * 10))
            .unwrap_or(Duration::ZERO);

        DurationEstimate {
            total: setup_duration + map_duration + reduce_duration,
            setup_phase: setup_duration,
            map_phase: map_duration,
            reduce_phase: reduce_duration,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ResourceEstimates {
    pub memory_usage: MemoryEstimate,
    pub disk_usage: DiskEstimate,
    pub network_usage: NetworkEstimate,
    pub worktree_count: usize,
    pub estimated_duration: DurationEstimate,
    pub checkpoint_storage: StorageEstimate,
}
```

#### 6. CLI Integration

Extend CLI to support dry-run mode:

```rust
#[derive(Parser)]
pub struct RunCommand {
    /// Workflow file to execute
    pub workflow: PathBuf,

    /// Run in dry-run mode without executing commands
    #[clap(long)]
    pub dry_run: bool,

    /// Show work item preview in dry-run mode
    #[clap(long, requires = "dry_run")]
    pub show_work_items: bool,

    /// Show variable interpolation preview
    #[clap(long, requires = "dry_run")]
    pub show_variables: bool,

    /// Show resource estimates
    #[clap(long, requires = "dry_run")]
    pub show_resources: bool,

    /// Limit work item preview to N items
    #[clap(long, requires = "dry_run")]
    pub sample_size: Option<usize>,

    /// Output format for dry-run results
    #[clap(long, requires = "dry_run", default_value = "human")]
    pub output_format: OutputFormat,
}

#[derive(Clone, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Yaml,
}

pub async fn handle_run_command(cmd: RunCommand) -> anyhow::Result<()> {
    if cmd.dry_run {
        handle_dry_run(cmd).await
    } else {
        handle_normal_run(cmd).await
    }
}

async fn handle_dry_run(cmd: RunCommand) -> anyhow::Result<()> {
    let workflow = load_workflow(&cmd.workflow)?;
    let coordinator = MapReduceCoordinator::new(workflow)
        .with_dry_run_mode(DryRunConfig {
            show_work_items: cmd.show_work_items,
            show_variables: cmd.show_variables,
            show_resources: cmd.show_resources,
            sample_size: cmd.sample_size,
        });

    let report = coordinator.execute_dry_run().await?;

    match cmd.output_format {
        OutputFormat::Human => print_human_report(&report),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
        OutputFormat::Yaml => println!("{}", serde_yaml::to_string(&report)?),
    }

    if !report.errors.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}
```

### Output Formatting

```rust
fn print_human_report(report: &DryRunReport) {
    println!("🔍 MapReduce Workflow Dry-Run Report");
    println!("=====================================\n");

    // Validation Results
    println!("📋 Validation Results:");
    print_validation_results(&report.validation_results);

    // Work Item Preview
    if !report.work_item_preview.items.is_empty() {
        println!("\n📊 Work Item Preview:");
        print_work_item_preview(&report.work_item_preview);
    }

    // Resource Estimates
    println!("\n💻 Resource Estimates:");
    print_resource_estimates(&report.resource_estimates);

    // Warnings and Errors
    if !report.warnings.is_empty() {
        println!("\n⚠️  Warnings:");
        for warning in &report.warnings {
            println!("  • {}", warning);
        }
    }

    if !report.errors.is_empty() {
        println!("\n❌ Errors:");
        for error in &report.errors {
            println!("  • {}", error);
        }
    }

    // Summary
    let status = if report.errors.is_empty() { "✅ READY" } else { "❌ NEEDS FIXES" };
    println!("\nStatus: {}", status);
}
```

## Testing Strategy

### Unit Tests
- Test dry-run validator with various workflow configurations
- Test input validation with different data sources and formats
- Test command validation with valid and invalid commands
- Test resource estimation accuracy
- Test variable interpolation validation

### Integration Tests
- Test end-to-end dry-run mode with complete workflows
- Test CLI integration with various flags and options
- Test output formatting for different output modes
- Test error detection and reporting
- Test performance with large input datasets

### Performance Tests
- Benchmark dry-run execution time vs. workflow complexity
- Test memory usage during large work item preview
- Test resource estimation accuracy vs. actual execution
- Test concurrent dry-run operations

### Validation Tests
- Test dry-run accuracy against actual execution results
- Test edge cases with malformed YAML or data
- Test validation coverage for all supported command types
- Test variable interpolation edge cases

## Migration Strategy

### Phase 1: Core Validation Infrastructure
1. Implement `DryRunValidator` and basic validation framework
2. Add input validation and JSONPath testing
3. Implement command structure validation

### Phase 2: Resource Estimation
1. Add resource estimation algorithms
2. Implement work item preview functionality
3. Add variable interpolation preview

### Phase 3: CLI Integration
1. Extend CLI with dry-run flags
2. Implement output formatting
3. Add comprehensive error reporting

### Phase 4: Advanced Features
1. Add performance optimization for large datasets
2. Implement validation caching
3. Add detailed recommendations and suggestions

## Documentation Requirements

- Update CLI documentation with dry-run mode options
- Create dry-run mode user guide with examples
- Document validation rules and error messages
- Add troubleshooting guide for common validation issues
- Create examples demonstrating dry-run workflows

## Risk Assessment

### High Risk
- **False Positives**: Dry-run might report errors that don't occur in actual execution
- **Performance Impact**: Large datasets might cause dry-run mode to be slow
- **Validation Gaps**: Some runtime errors might not be caught in dry-run mode

### Medium Risk
- **Resource Estimation Accuracy**: Estimates might be significantly off from actual usage
- **Variable Complexity**: Complex variable interpolation might be hard to validate
- **Output Overwhelm**: Too much information might make reports hard to read

### Mitigation Strategies
- Implement comprehensive test coverage comparing dry-run vs. actual execution
- Provide configurable verbosity levels for output
- Include confidence levels with resource estimates
- Add validation for only checkable aspects, clearly document limitations
- Provide sample size limits for large dataset previews