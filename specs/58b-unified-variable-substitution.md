---
number: 58b
title: Unified Variable Substitution
category: architecture
priority: critical
status: draft
parent: 58
created: 2025-09-04
---

# Specification 58b: Unified Variable Substitution

**Category**: architecture
**Priority**: critical
**Status**: draft
**Parent**: [58 - Unified Execution Model]

## Context

**This is the most critical aspect of the unified execution model.** Like Ansible and Terraform, we need ONE consistent variable system across ALL execution paths. Currently each path uses different variable names and syntax:

| Execution Path | Same Data Referenced As | Syntax |
|----------------|-------------------------|--------|
| Standard | `${env_var}` | Simple replacement |
| Args Path | `${ARG}`, `${INDEX}` | Simple replacement |
| Map Path | `${FILE}`, `${FILE_PATH}` | Simple replacement |
| MapReduce | `${item.file_path}`, `${items[0]}` | Complex JSON paths |

This means the SAME workflow behaves differently depending on how it's invoked, breaking the fundamental principle of configuration management systems.

## Objective

Create a unified variable substitution system that:
1. Uses consistent variable names across ALL execution modes
2. Provides the same interpolation capabilities everywhere
3. Maintains backward compatibility during migration
4. Enables workflow portability between execution modes

## Technical Details

### Standard Variable Names

```rust
// src/cook/workflow/variables.rs
use std::collections::HashMap;
use serde_json::Value;

/// Standard variable names that work in ALL execution modes
/// These are the ONLY variable names that should be used
pub struct StandardVariables;

impl StandardVariables {
    // Input variables - consistent regardless of source
    pub const ITEM: &'static str = "item";           // Current item being processed
    pub const INDEX: &'static str = "item_index";    // Zero-based index
    pub const TOTAL: &'static str = "item_total";    // Total number of items
    
    // For backwards compatibility during migration
    pub const ITEM_VALUE: &'static str = "item.value";  // The actual value
    pub const ITEM_PATH: &'static str = "item.path";    // For file inputs
    pub const ITEM_NAME: &'static str = "item.name";    // Display name
    
    // Workflow context variables
    pub const WORKFLOW_NAME: &'static str = "workflow.name";
    pub const WORKFLOW_ID: &'static str = "workflow.id";
    pub const ITERATION: &'static str = "workflow.iteration";
    
    // Step context variables  
    pub const STEP_NAME: &'static str = "step.name";
    pub const STEP_INDEX: &'static str = "step.index";
    
    // Output capture variables
    pub const LAST_OUTPUT: &'static str = "last.output";
    pub const LAST_EXIT_CODE: &'static str = "last.exit_code";
    
    // MapReduce specific (only available in those contexts)
    pub const MAP_KEY: &'static str = "map.key";        // Key for map output
    pub const MAP_RESULTS: &'static str = "map.results"; // Aggregated map results
    pub const WORKER_ID: &'static str = "worker.id";     // Parallel worker ID
}
```

### Unified Variable Context

```rust
/// Unified variable context that ALL paths use
#[derive(Debug, Clone)]
pub struct VariableContext {
    variables: HashMap<String, Value>,  // ALL variables stored here
    aliases: HashMap<String, String>,   // For backwards compatibility
}

impl VariableContext {
    /// Create context for any execution mode with STANDARD variable names
    pub fn from_execution_input(
        mode: &ExecutionMode,
        input: &ExecutionInput,
        index: usize,
        total: usize,
    ) -> Self {
        let mut variables = HashMap::new();
        let mut aliases = HashMap::new();
        
        // Standard variables that work everywhere
        match input {
            ExecutionInput::Argument(arg) => {
                variables.insert(StandardVariables::ITEM.into(), json!(arg));
                variables.insert(StandardVariables::ITEM_VALUE.into(), json!(arg));
                // Legacy compatibility
                aliases.insert("ARG".into(), StandardVariables::ITEM_VALUE.into());
            }
            ExecutionInput::FilePath(path) => {
                variables.insert(StandardVariables::ITEM.into(), json!(path));
                variables.insert(StandardVariables::ITEM_PATH.into(), json!(path));
                // Legacy compatibility  
                aliases.insert("FILE".into(), StandardVariables::ITEM_PATH.into());
                aliases.insert("FILE_PATH".into(), StandardVariables::ITEM_PATH.into());
            }
            ExecutionInput::JsonObject(obj) => {
                // MapReduce items - use the SAME variable names!
                variables.insert(StandardVariables::ITEM.into(), obj.clone());
                // Flatten for convenience
                if let Some(path) = obj.get("file_path") {
                    variables.insert(StandardVariables::ITEM_PATH.into(), path.clone());
                }
            }
        }
        
        // Always set standard context variables
        variables.insert(StandardVariables::INDEX.into(), json!(index));
        variables.insert(StandardVariables::TOTAL.into(), json!(total));
        
        Self { variables, aliases }
    }
    
    /// Use the SAME interpolation engine for ALL paths
    /// This ensures consistent behavior across all execution modes
    pub fn interpolate(&self, template: &str) -> Result<String> {
        // First resolve aliases for backwards compatibility
        let template = self.resolve_aliases(template);
        
        // Use the existing MapReduce InterpolationEngine for ALL paths!
        // This gives everyone nested access, defaults, etc.
        let mut engine = InterpolationEngine::new(false);
        let context = self.to_interpolation_context();
        
        engine.interpolate(&template, &context)
            .context("Failed to interpolate variables")
    }
    
    fn resolve_aliases(&self, template: &str) -> String {
        self.aliases.iter().fold(template.to_string(), |acc, (old, new)| {
            acc.replace(&format!("${{{}}}", old), &format!("${{{}}}", new))
               .replace(&format!("${}", old), &format!("${}", new))
        })
    }
}
```

## Example: Workflow Portability

With the unified system, the SAME workflow works identically regardless of how it's invoked:

```yaml
# workflow.yml - This now works the same in ALL modes!
name: process_files
steps:
  - name: Process item
    command: 
      type: shell
      cmd: echo "Processing ${item.path} (${item_index} of ${item_total})"
    
  - name: Validate
    command:
      type: claude  
      prompt: "Check if ${item.path} was processed correctly"
```

This workflow now works consistently whether invoked as:
- `prodigy cook workflow.yml --args "file1.txt,file2.txt"`
- `prodigy cook workflow.yml --map "*.txt"`

In ALL cases:
- `${item.path}` contains the current file path
- `${item_index}` contains the current index
- `${item_total}` contains the total count
- Nested access like `${item.metadata.author}` works everywhere
- Default values like `${timeout:-600}` work everywhere

## MapReduce Workflows

MapReduce workflows have a different structure with setup/map/reduce phases, but use the SAME variable naming:

```yaml
# mapreduce-workflow.yml - Different structure, SAME variable names
name: analyze_files
mode: mapreduce

setup:
  - name: Initialize
    command:
      type: shell
      cmd: echo "Starting analysis of ${item_total} files"

map:
  # Each worker gets consistent variables
  - name: Analyze file
    command:
      type: claude
      prompt: "Analyze ${item.path} for complexity"
    outputs:
      complexity_score: "${last.output}"

reduce:
  # Reduce phase aggregates results
  - name: Generate report  
    command:
      type: shell
      # Access to map results via standard names
      cmd: echo "Processed ${item_total} files with results: ${map.results}"
```

## Backward Compatibility

The system maintains aliases for legacy variables:
- `${ARG}` → `${item.value}`
- `${FILE}` → `${item.path}`
- `${FILE_PATH}` → `${item.path}`
- `${INDEX}` → `${item_index}`

This allows existing workflows to continue working while teams migrate to standard names.

## Benefits

1. **Workflow Portability**: Same workflow works in all execution modes
2. **Consistent Capabilities**: Nested access and defaults work everywhere
3. **Reduced Complexity**: One variable system to understand and test
4. **Better Documentation**: Standard names are self-documenting
5. **Easier Testing**: One interpolation engine to test

## Success Criteria

- [ ] All execution paths use the same variable names
- [ ] All paths use the same InterpolationEngine
- [ ] Backward compatibility through aliases
- [ ] Workflows are portable between execution modes
- [ ] MapReduce workflows use consistent naming within their structure