# Evidence for Error Handling Chapter

## Source Definitions Found

### MapReduce Configuration
- MapPhaseYaml struct: src/config/mapreduce.rs:269
  - Field: `agent_timeout_secs: Option<String>`
  - Supports numeric values or env var references
- MapReduceConfig struct: src/cook/execution/mapreduce/types.rs:28
  - Field: `agent_timeout_secs: Option<u64>`
  - Runtime field, resolved to seconds

### Error Collection Strategy
- ErrorCollectionStrategy enum: src/cook/workflow/error_policy.rs:33-44
  - Variants: Aggregate, Immediate, Batched { size: usize }
  - Serde: `#[serde(rename_all = "snake_case")]`
  - Batched syntax in YAML: `batched:N` where N is batch size

### CLI Commands
- Events commands defined: src/cli/args.rs:457-575
- Implementation (stubs): src/cli/commands/events.rs
- Router: src/cli/router.rs:180
- Available subcommands: ls, stats, search, follow, clean, export

## Test Examples Found
- ErrorCollectionStrategy parsing: src/config/mapreduce.rs:484-493
- Batched format: "batched:10" → ErrorCollectionStrategy::Batched { size: 10 }

## Configuration Examples Found
- book/src/mapreduce/error-collection-strategies.md
  - Example: `error_collection: batched:5`

## Validation Results
✓ Field name verified: agent_timeout_secs (NOT timeout_per_agent)
✓ Batched syntax verified: batched:N format (NOT nested YAML)
✓ CLI commands verified: follow exists (NOT watch)
✗ Events CLI has stub implementation only (note added to docs)

## Issues to Fix
1. Line 332: Change "timeout_per_agent" → "agent_timeout_secs"
2. Line 499-500: Change batched YAML syntax from nested to "batched:10"
3. Line 239: Change "prodigy events watch" → "prodigy events follow"
4. Already fixed: Line 101 - Note about Detailed vs Advanced format
5. Already fixed: Line 137 - Explanation of capture field usage
6. Line 336: Add clarification about which errors get specific vs generic suggestions

## Discovery Notes
- Source directories: src/config/, src/cook/workflow/, src/cli/
- Config parsing handles special "batched:N" format
- CLI events commands are defined but not fully implemented
