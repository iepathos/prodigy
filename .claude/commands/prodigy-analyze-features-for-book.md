# /prodigy-analyze-features-for-book

Perform comprehensive analysis of Prodigy codebase to identify features and capabilities that should be documented in the book.

## Variables

None required - analyzes codebase and creates .prodigy/book-analysis/features.json

## Execute

### Phase 1: Understand Context

You are analyzing the Prodigy workflow orchestration tool to create a comprehensive feature inventory for the documentation book. This will be used to detect drift between the book documentation and actual implementation.

### Phase 2: Analyze Core Features

**Key Areas to Document:**

#### 1. Workflow Basics
**Key Files:**
- `src/config/workflow.rs` - WorkflowConfig
- `src/cook/workflow/executor.rs` - WorkflowExecutor

**Extract:**
- Standard workflow structure
- Command execution model
- Basic YAML syntax
- Common workflow patterns

#### 2. MapReduce Workflows
**Key Files:**
- `src/config/mapreduce.rs` - MapReduceWorkflowConfig, MapPhaseYaml, ReducePhaseYaml
- `src/cook/execution/mapreduce/` - Implementation details

**Extract:**
- Setup phase configuration
- Map phase configuration
- Reduce phase configuration
- Parallel execution capabilities
- Work distribution model
- Results aggregation

#### 3. Command Types
**Key Files:**
- `src/config/command.rs` - WorkflowStepCommand, WorkflowCommand
- `src/cook/workflow/executor.rs` - CommandType enum

**Extract:**
- All command types (shell, claude, goal_seek, foreach, validation, handler)
- Fields for each command type
- Required vs optional fields
- Common use cases for each

#### 4. Variables and Interpolation
**Key Files:**
- `src/cook/workflow/variables.rs` - VariableStore, CaptureFormat

**Extract:**
- Standard variables (shell.output, claude.output, etc.)
- MapReduce variables (item, map.total, etc.)
- Git context variables
- Validation variables
- Merge variables
- Custom capture syntax

#### 5. Environment Configuration
**Key Files:**
- `src/cook/environment/` - Environment management
- `src/config/workflow.rs` - Environment fields

**Extract:**
- Global environment variables
- Secrets management
- Environment profiles
- Dynamic environment variables
- Step-level environment overrides

#### 6. Advanced Features
**Key Files:**
- `src/cook/workflow/validation.rs` - ValidationConfig
- `src/cook/goal_seek/mod.rs` - GoalSeekConfig

**Extract:**
- Conditional execution (when)
- Output capture formats
- Nested conditionals
- Timeout configuration
- Working directory control
- Auto-commit functionality

#### 7. Error Handling
**Key Files:**
- `src/cook/workflow/error_policy.rs` - WorkflowErrorPolicy
- `src/config/command.rs` - on_failure configuration

**Extract:**
- Workflow-level error policies
- Command-level error handling
- Retry mechanisms
- Circuit breaker
- DLQ (Dead Letter Queue)
- Error collection strategies

#### 8. Examples and Use Cases
**Key Files:**
- `workflows/` - Example workflows
- `tests/` - Integration tests

**Extract:**
- Common workflow patterns
- Real-world use cases
- Best practices
- Anti-patterns to avoid

### Phase 3: Create Feature Inventory

Create a JSON file at `.prodigy/book-analysis/features.json` with this structure:

```json
{
  "workflow_basics": {
    "structure": {
      "simple_array": "Direct command array",
      "full_config": "With env, secrets, profiles"
    },
    "execution_model": "Sequential command execution",
    "commit_tracking": "Git integration for audit trail"
  },
  "mapreduce": {
    "phases": ["setup", "map", "reduce"],
    "capabilities": {
      "parallel_execution": true,
      "work_distribution": "Automatic across agents",
      "result_aggregation": "In reduce phase",
      "checkpoint_resume": true
    },
    "configuration": {
      "setup": ["commands", "timeout", "capture_outputs"],
      "map": ["input", "json_path", "agent_template", "max_parallel", "filter", "sort_by"],
      "reduce": ["commands", "aggregation"]
    }
  },
  "command_types": {
    "shell": {
      "description": "Execute shell commands",
      "common_fields": ["shell", "timeout", "capture", "on_failure"],
      "use_cases": ["Build", "Test", "Deploy", "Data processing"]
    },
    "claude": {
      "description": "Execute Claude AI commands",
      "common_fields": ["claude", "commit_required", "validate"],
      "use_cases": ["Code generation", "Analysis", "Refactoring"]
    },
    "goal_seek": {
      "description": "Iterative refinement to reach quality threshold",
      "fields": ["goal", "validate", "threshold", "max_attempts"],
      "use_cases": ["Coverage improvement", "Performance optimization"]
    },
    "foreach": {
      "description": "Iterate over lists with optional parallelism",
      "fields": ["input", "do", "parallel", "continue_on_error"],
      "use_cases": ["File processing", "Batch operations"]
    }
  },
  "variables": {
    "standard": {
      "shell.output": "Last shell command output",
      "claude.output": "Last Claude command output",
      "last.output": "Last command output (any type)",
      "last.exit_code": "Exit code from last command"
    },
    "mapreduce": {
      "item": "Current work item in map phase",
      "item.*": "Access item fields with wildcard",
      "map.total": "Total items processed",
      "map.successful": "Successfully processed items",
      "map.failed": "Failed items",
      "map.results": "Aggregated results"
    },
    "validation": {
      "validation.completion": "Completion percentage",
      "validation.gaps": "Missing requirements",
      "validation.status": "Status (complete/incomplete/failed)"
    }
  },
  "environment": {
    "global_env": "Static and dynamic variables",
    "secrets": "Masked in logs, supports providers",
    "profiles": "Environment-specific configurations",
    "step_env": "Command-level overrides"
  },
  "advanced_features": {
    "conditional_execution": "when: expression",
    "output_capture": ["string", "number", "json", "lines", "boolean"],
    "nested_handlers": "on_success, on_failure, on_exit_code",
    "timeout_control": "Command and workflow level",
    "working_directory": "Per-command cwd control"
  },
  "error_handling": {
    "workflow_level": {
      "on_item_failure": ["dlq", "retry", "skip", "stop"],
      "error_collection": ["aggregate", "immediate", "batched"],
      "circuit_breaker": true,
      "max_failures": "Stop after N failures"
    },
    "command_level": {
      "on_failure": "Nested command execution",
      "on_success": "Success handlers",
      "on_exit_code": "Map exit codes to actions",
      "retry_config": "With exponential backoff"
    }
  },
  "best_practices": {
    "workflow_design": [
      "Keep workflows simple and focused",
      "Use validation for quality gates",
      "Handle errors gracefully",
      "Capture important outputs"
    ],
    "mapreduce": [
      "Set appropriate parallelism",
      "Use DLQ for failed items",
      "Monitor with events",
      "Design idempotent work items"
    ],
    "testing": [
      "Include test steps in workflows",
      "Use on_failure for debugging",
      "Validate before deploying"
    ]
  },
  "common_patterns": [
    {
      "name": "Build and Test",
      "description": "Standard CI workflow",
      "example": "workflows/examples/build-test.yml"
    },
    {
      "name": "Parallel Processing",
      "description": "MapReduce for independent items",
      "example": "workflows/examples/parallel-review.yml"
    },
    {
      "name": "Goal Seeking",
      "description": "Iterative improvement to threshold",
      "example": "workflows/examples/coverage-improvement.yml"
    }
  ],
  "troubleshooting": {
    "common_issues": [
      {
        "issue": "Variables not interpolating",
        "solution": "Check ${} syntax and variable availability"
      },
      {
        "issue": "MapReduce items not found",
        "solution": "Verify JSONPath expression"
      },
      {
        "issue": "Timeout errors",
        "solution": "Increase timeout or optimize commands"
      }
    ]
  },
  "version_info": {
    "analyzed_version": "0.2.0+",
    "analysis_date": "2025-01-XX"
  }
}
```

### Phase 4: Analysis Method

1. **Read Source Files**: Examine all key implementation files
2. **Parse Struct Definitions**: Extract fields, types, serde attributes
3. **Identify Capabilities**: What can users actually do?
4. **Find Examples**: Look in workflows/ and tests/
5. **Document Patterns**: Common use cases and best practices

### Phase 5: Quality Guidelines

- Focus on user-facing features, not implementation details
- Document capabilities, not just configuration options
- Include practical use cases for each feature
- Note common pitfalls and solutions
- Provide realistic examples
- Keep language accessible for book audience

### Phase 6: Validation

The features.json file should:
1. Cover all major feature areas
2. Include practical use cases
3. Provide examples for each capability
4. Document common patterns
5. Include troubleshooting guidance
6. Be user-focused, not developer-focused
