# Implementation Plan: Reduce Complexity in execute_map_phase_internal

## Problem Summary

**Location**: src/cook/execution/mapreduce/coordination/executor.rs:MapReduceCoordinator::execute_map_phase_internal:618
**Priority Score**: 19.7925
**Debt Type**: ComplexityHotspot (Cyclomatic: 14, Cognitive: 75)
**Current Metrics**:
- Function Length: 208 lines
- Cyclomatic Complexity: 14
- Cognitive Complexity: 75
- Nesting Depth: 2
- Purity Confidence: 0.81 (marked as PureLogic role)

**Issue**: High cyclomatic complexity (14) and extreme cognitive complexity (75) make this function hard to test and maintain. The function handles too many responsibilities: parallel execution orchestration, agent lifecycle management, event logging, error handling, DLQ integration, and retry tracking.

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: 7.0 (from 14 to ~10 cyclomatic)
- Risk Reduction: 6.927375
- Coverage Improvement: 0.0

**Success Criteria**:
- [x] Cyclomatic complexity reduced to 10 or lower
- [x] Cognitive complexity reduced to under 50
- [x] Extract at least 4 focused helper functions
- [x] All existing tests continue to pass
- [x] No clippy warnings
- [x] Proper formatting with cargo fmt

## Implementation Phases

### Phase 1: Extract Result Processing Logic

**Goal**: Extract the complex result-to-AgentResult conversion logic into a dedicated pure function.

**Changes**:
- Create `convert_execution_result_to_agent_result()` function that handles:
  - Converting `Ok(agent_result)` to final AgentResult with event logging
  - Converting `Err(e)` to failed AgentResult with error details
  - Structuring AgentResult fields consistently
- Move the entire `match result { Ok(...) => {...} Err(e) => {...} }` block (lines 629-669) into this function
- Function signature: `async fn convert_execution_result_to_agent_result(result: Result<AgentResult, MapReduceError>, agent_id: String, item_id: String, duration: Duration, event_logger: Arc<dyn EventLogger>) -> MapReduceResult<AgentResult>`

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [x] Result conversion logic extracted to dedicated function
- [x] Cyclomatic complexity reduced by ~2
- [x] All tests pass
- [x] Ready to commit

### Phase 2: Extract DLQ Integration Logic

**Goal**: Extract Dead Letter Queue integration logic into a separate pure function.

**Changes**:
- Create `handle_dlq_for_failed_item()` function that:
  - Retrieves retry count for the item
  - Converts agent result to DLQ item if needed
  - Adds item to DLQ with proper error handling
  - Increments retry count in state
  - Logs appropriate info/warn messages
- Move the entire DLQ handling block (lines 674-704) into this function
- Function signature: `async fn handle_dlq_for_failed_item(agent_result: &AgentResult, item: &Value, item_id: &str, dlq: Arc<dyn DeadLetterQueue>, retry_counts: Arc<RwLock<HashMap<String, usize>>>) -> ()`

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [x] DLQ logic extracted to dedicated function
- [x] Cyclomatic complexity reduced by ~2
- [x] All tests pass
- [x] Ready to commit

### Phase 3: Extract Agent Execution Orchestration

**Goal**: Extract the core agent execution flow into a focused function.

**Changes**:
- Create `process_single_work_item()` function that handles:
  - Acquiring semaphore permit
  - Creating item_id and agent_id
  - Logging agent start event
  - Executing agent with timing
  - Converting result to AgentResult
  - Handling DLQ integration
  - Adding result to collector
- This extracts the entire tokio::spawn closure body (lines 583-707) into a well-defined function
- Function signature: `async fn process_single_work_item(index: usize, item: Value, job_id: String, map_phase: MapPhase, env: ExecutionEnvironment, semaphore: Arc<Semaphore>, agent_manager: Arc<dyn AgentLifecycleManager>, merge_queue: Arc<MergeQueue>, event_logger: Arc<dyn EventLogger>, result_collector: Arc<ResultCollector>, user_interaction: Arc<dyn UserInteraction>, command_executor: Arc<dyn CommandExecutor>, dlq: Arc<dyn DeadLetterQueue>, retry_counts: Arc<RwLock<HashMap<String, usize>>>, timeout_enforcer: Option<Arc<TimeoutEnforcer>>, total_items: usize) -> MapReduceResult<AgentResult>`

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [x] Single item processing extracted to dedicated function
- [x] Cyclomatic complexity reduced by ~2
- [x] All tests pass
- [x] Ready to commit

### Phase 4: Simplify Main Function and Extract Future Collection

**Goal**: Simplify the main function by extracting the future collection and result aggregation logic.

**Changes**:
- Create `collect_agent_results()` function that:
  - Takes the Vec of agent futures
  - Waits for all to complete
  - Handles Ok/Err cases with appropriate logging
  - Returns Vec<AgentResult>
- Move the result collection loop (lines 712-725) into this function
- Main function becomes more declarative: setup -> spawn agents -> collect results -> log summary
- Function signature: `async fn collect_agent_results(agent_futures: Vec<JoinHandle<MapReduceResult<AgentResult>>>) -> Vec<AgentResult>`

**Testing**:
- Run `cargo test --lib` to verify existing tests pass
- Run `cargo clippy` to check for warnings

**Success Criteria**:
- [x] Future collection logic extracted
- [x] Main function is now more readable and focused
- [x] Cyclomatic complexity reduced to target (~10)
- [x] All tests pass
- [x] Ready to commit

### Phase 5: Final Verification and Documentation

**Goal**: Verify all improvements meet targets and add documentation.

**Changes**:
- Add doc comments to all new helper functions
- Ensure main function has clear documentation of its flow
- Run full test suite
- Verify complexity metrics

**Testing**:
- Run `cargo test --all` (full test suite)
- Run `cargo clippy -- -D warnings` (fail on any warnings)
- Run `cargo fmt --check` (verify formatting)
- Run complexity analysis to confirm improvements

**Success Criteria**:
- [x] All new functions have clear documentation
- [x] Full test suite passes
- [x] No clippy warnings
- [x] Code is properly formatted
- [x] Complexity targets achieved

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. Run `cargo fmt` to ensure consistent formatting
4. Review extracted functions for clarity and correctness

**Final verification**:
1. `cargo test --all` - Full test suite
2. `cargo clippy -- -D warnings` - No warnings allowed
3. `cargo fmt --check` - Verify formatting
4. Optionally run `cargo tarpaulin` to check if coverage is maintained

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure (test output, clippy warnings, compilation errors)
3. Adjust the approach:
   - If function signature is wrong, revise parameter types
   - If tests fail, check for logic errors in extraction
   - If clippy warns, fix the specific issue
4. Retry the phase with corrections

## Notes

**Key Complexity Sources Identified**:
1. **Result conversion** - Large match block with event logging (lines 629-669)
2. **DLQ integration** - Retry tracking, error handling, state updates (lines 674-704)
3. **Agent orchestration** - Semaphore, timing, execution flow (lines 583-707)
4. **Future collection** - Result aggregation with error handling (lines 712-725)

**Refactoring Strategy**:
- Focus on extracting logical units that can be tested independently
- Maintain all existing behavior - no functional changes
- Use function composition to reduce nesting and branching
- Keep Arc-cloned dependencies explicit in signatures for clarity

**Expected Outcome**:
- Main function becomes a high-level orchestrator (~50 lines)
- Helper functions are focused and testable
- Cyclomatic complexity drops from 14 to ~10
- Cognitive complexity drops from 75 to under 50
- Code is more maintainable and easier to understand
