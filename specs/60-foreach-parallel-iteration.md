---
number: 60
title: Foreach Parallel Iteration Support
category: parallel
priority: high
status: draft
dependencies: [58, 59]
created: 2025-01-14
---

# Specification 60: Foreach Parallel Iteration Support

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: [58 - Command Output Input, 59 - Simplified CLI]

## Context

The whitepaper specifies a `foreach` construct for simpler parallel processing that doesn't require the full MapReduce pattern:

```yaml
tasks:
  - foreach: "find . -name '*.js'"
    parallel: true
    do:
      claude: "/convert-to-typescript ${item}"
```

This provides a more intuitive way to express parallel operations without the complexity of map/reduce phases. Currently, users must use MapReduce even for simple parallel iterations, which is unnecessarily complex.

## Objective

Implement the `foreach` construct to enable simple parallel iteration over command outputs or lists, providing an intuitive alternative to MapReduce for straightforward parallel operations.

## Requirements

### Functional Requirements
- Support `foreach` with command execution: `foreach: "find . -name '*.py'"`
- Support `foreach` with static lists: `foreach: ["file1.py", "file2.py"]`
- Enable parallel execution with `parallel: true` or `parallel: N`
- Provide sequential execution when `parallel: false` (default)
- Support nested `do` blocks with multiple commands
- Enable variable interpolation with `${item}` in commands
- Track progress and provide status updates
- Support early termination on failure

### Non-Functional Requirements
- Performance equivalent to MapReduce for same operations
- Memory efficient for large item lists
- Clear error reporting per item
- Intuitive syntax matching whitepaper examples

## Acceptance Criteria

- [ ] `foreach: "ls *.py"` iterates over Python files
- [ ] `parallel: 5` limits concurrent executions to 5
- [ ] `${item}` correctly interpolates in nested commands
- [ ] Sequential execution works when parallel not specified
- [ ] Failed items reported clearly with context
- [ ] Progress bar shows items completed/remaining
- [ ] Nested do blocks execute in order per item
- [ ] Integration with existing retry mechanisms
- [ ] Validation commands work within do blocks
- [ ] Early exit on critical failures respected

## Technical Details

### Implementation Approach

1. **Foreach Configuration Structure**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ForeachTask {
       /// Input source (command or list)
       pub foreach: ForeachInput,
       /// Parallel execution config
       pub parallel: ParallelConfig,
       /// Commands to execute per item
       pub do_block: Vec<WorkflowStep>,
       /// Optional: Continue on item failure
       #[serde(default)]
       pub continue_on_error: bool,
       /// Optional: Maximum items to process
       pub max_items: Option<usize>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum ForeachInput {
       Command(String),
       List(Vec<String>),
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum ParallelConfig {
       Boolean(bool),
       Count(usize),
   }
   ```

2. **Execution Engine**:
   ```rust
   impl ForeachExecutor {
       pub async fn execute(
           &self,
           task: &ForeachTask,
           context: &ExecutionContext,
       ) -> Result<ForeachResult> {
           // Get items from input source
           let items = self.get_items(&task.foreach).await?;

           // Determine parallelism
           let max_parallel = match task.parallel {
               ParallelConfig::Boolean(true) => 10, // Default
               ParallelConfig::Boolean(false) => 1,
               ParallelConfig::Count(n) => n,
           };

           // Execute with parallelism control
           if max_parallel > 1 {
               self.execute_parallel(items, &task.do_block, max_parallel).await
           } else {
               self.execute_sequential(items, &task.do_block).await
           }
       }

       async fn execute_parallel(
           &self,
           items: Vec<String>,
           commands: &[WorkflowStep],
           max_parallel: usize,
       ) -> Result<ForeachResult> {
           let semaphore = Arc::new(Semaphore::new(max_parallel));
           let tasks = items.into_iter().map(|item| {
               let sem = semaphore.clone();
               let cmds = commands.to_vec();
               async move {
                   let _permit = sem.acquire().await?;
                   self.execute_item(item, cmds).await
               }
           });

           let results = futures::future::join_all(tasks).await;
           self.aggregate_results(results)
       }
   }
   ```

3. **Variable Interpolation**:
   ```rust
   fn interpolate_item(command: &str, item: &str) -> String {
       command.replace("${item}", item)
   }
   ```

### Architecture Changes
- Add `ForeachTask` to workflow step types
- Implement `ForeachExecutor` in execution module
- Extend variable interpolation for item context
- Add progress tracking for foreach operations

### Data Structures
```yaml
# Example workflow with foreach
tasks:
  - name: "Convert JavaScript to TypeScript"
    foreach: "find src -name '*.js'"
    parallel: 10
    do:
      - shell: "cp ${item} ${item}.backup"
      - claude: "/convert-to-typescript ${item}"
      - shell: "mv ${item%.js}.ts ${item}"
      - validate: "tsc --noEmit ${item%.js}.ts"
    continue_on_error: true

  - name: "Process specific files"
    foreach: ["critical.js", "main.js", "app.js"]
    parallel: false  # Sequential for critical files
    do:
      - claude: "/add-error-handling ${item}"
```

## Dependencies

- **Prerequisites**:
  - [58 - Command Output Input] for command execution
  - [59 - Simplified CLI] for integration
- **Affected Components**:
  - `src/config/workflow.rs` - Add foreach configuration
  - `src/cook/execution/` - Foreach executor
  - `src/cook/workflow/` - Workflow step processing
- **External Dependencies**: `tokio::sync::Semaphore` for parallelism control

## Testing Strategy

- **Unit Tests**:
  - Foreach input parsing (command vs list)
  - Parallel execution with semaphore
  - Variable interpolation in commands
  - Error aggregation
- **Integration Tests**:
  - End-to-end foreach with file processing
  - Parallel vs sequential execution
  - Failure handling and continue_on_error
  - Progress tracking accuracy
- **Performance Tests**:
  - Large item lists (1000+ items)
  - Parallel efficiency vs MapReduce
  - Memory usage during execution

## Documentation Requirements

- **Code Documentation**: Document foreach execution flow
- **User Documentation**:
  - Foreach vs MapReduce decision guide
  - Common patterns and examples
  - Performance considerations
- **Architecture Updates**: Add foreach to execution model diagram

## Implementation Notes

- Reuse MapReduce infrastructure where possible
- Consider unified progress tracking interface
- Support dry-run to preview items before execution
- Allow foreach within MapReduce reduce phase
- Future: Support async iterators for streaming

## Migration and Compatibility

- No breaking changes to existing workflows
- Foreach is additive feature
- Can convert simple MapReduce to foreach
- Guidelines for choosing foreach vs MapReduce