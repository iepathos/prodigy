---
number: 58c
title: Incremental Migration Plan
category: implementation
priority: high
status: draft
parent: 58
created: 2025-09-04
---

# Specification 58c: Incremental Migration Plan

**Category**: implementation
**Priority**: high
**Status**: draft
**Parent**: [58 - Unified Execution Model]

## Context

Previous attempts to consolidate the execution paths failed because they tried to do too much at once. This specification provides a detailed, incremental migration plan that:
1. Maintains backward compatibility at every step
2. Uses feature flags for gradual rollout
3. Tests exhaustively before proceeding
4. Allows rollback at any checkpoint

## Migration Phases

### Phase 0: Preparation and Testing (Week 1)

**Goal**: Create comprehensive test suite for current behavior before any changes

1. **Capture Current Behavior**:
   ```rust
   // tests/workflow_execution_baseline.rs
   #[test]
   fn test_standard_workflow_with_validation() { 
       // Document that validation works here
   }
   
   #[test] 
   fn test_structured_workflow_loses_validation() { 
       // Document current bug
   }
   
   #[test]
   fn test_args_workflow_loses_validation() { 
       // Document current bug
   }
   
   #[test]
   fn test_mapreduce_workflow_missing_features() {
       // Document missing features
   }
   ```

2. **Document Feature Matrix**:
   | Feature | Standard | Structured | Args/Map | MapReduce |
   |---------|----------|------------|----------|-----------|
   | Validation | ✅ | ❌ | ❌ | ❌ |
   | Handlers | ✅ | ❌ | ❌ | ❌ |
   | Timeouts | ✅ | ✅ | ✅ | ✅ |
   | Outputs | ❌ | ✅ | ❌ | ❌ |
   | Variables | ✅ | ✅ | ✅ | ✅ |

### Phase 1: Extract Common Components (Week 2)

**Goal**: Extract shared logic without changing execution paths

1. **Extract Variable Substitution** (58b):
   ```rust
   // src/cook/workflow/variables.rs
   pub struct VariableContext { /* from spec 58b */ }
   ```

2. **Extract Command Normalization** (58a):
   ```rust
   // src/cook/workflow/normalization.rs
   pub struct NormalizedWorkflow { /* from spec 58a */ }
   ```

3. **Extract Git Verification**:
   ```rust
   // src/cook/workflow/git_verification.rs
   pub struct GitVerification { /* consolidated logic */ }
   ```

### Phase 2: Create Normalized Workflow (Week 3)

**Goal**: Implement normalization layer from spec 58a

```rust
impl DefaultCookOrchestrator {
    /// New normalization path - behind feature flag
    fn normalize_workflow(&self, config: &CookConfig) -> Result<NormalizedWorkflow> {
        let mode = NormalizedWorkflow::classify_workflow_type(config);
        NormalizedWorkflow::from_workflow_config(&config.workflow, mode)
    }
}
```

### Phase 3: Functional Orchestration (Week 4)

**Goal**: Create unified execution path using normalized workflows

```rust
impl DefaultCookOrchestrator {
    async fn execute_workflow(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        // Feature flag for gradual rollout
        if std::env::var("USE_UNIFIED_PATH").is_ok() {
            return self.execute_unified(env, config).await;
        }
        
        // Existing paths remain during migration
        match Self::classify_workflow_type(config) {
            WorkflowType::Standard => {
                self.execute_workflow_standard(env, config).await
            }
            WorkflowType::StructuredWithOutputs => {
                self.execute_structured_workflow(env, config).await
            }
            WorkflowType::WithArguments => {
                self.execute_workflow_with_args(env, config).await
            }
            WorkflowType::MapReduce => {
                self.execute_mapreduce_workflow(env, config).await
            }
        }
    }
    
    async fn execute_unified(
        &self,
        env: &ExecutionEnvironment,
        config: &CookConfig,
    ) -> Result<()> {
        // Normalize workflow
        let normalized = self.normalize_workflow(config)?;
        
        // Execute through unified path
        self.execute_normalized(normalized, env).await
    }
}
```

### Phase 4: Gradual Migration (Week 5)

**Goal**: Enable unified path for each workflow type incrementally

1. **Week 5.1**: Enable for standard workflows
   ```bash
   USE_UNIFIED_PATH=1 WORKFLOW_TYPE=standard prodigy cook
   ```

2. **Week 5.2**: Enable for args/map workflows
   ```bash
   USE_UNIFIED_PATH=1 WORKFLOW_TYPE=args prodigy cook
   ```

3. **Week 5.3**: Enable for structured workflows
   ```bash
   USE_UNIFIED_PATH=1 WORKFLOW_TYPE=structured prodigy cook
   ```

4. **Week 5.4**: Enable for MapReduce
   ```bash
   USE_UNIFIED_PATH=1 WORKFLOW_TYPE=mapreduce prodigy cook
   ```

### Phase 5: Cleanup (Week 6)

**Goal**: Remove old execution paths after validation

1. **Remove from DefaultCookOrchestrator**:
   - `execute_structured_workflow()` 
   - `execute_workflow_with_args()`
   - `execute_mapreduce_workflow()`
   - Helper methods specific to old paths

2. **Remove legacy variable handling**

3. **Update documentation**

## Rollback Strategy

### Checkpoints

1. **After Phase 1**: No execution changes, can stop here
2. **After Phase 2**: Normalization exists but unused
3. **After Phase 3**: Feature flag controls new path
4. **After Phase 4**: Can disable per workflow type
5. **After Phase 5**: Need git revert

### Rollback Commands

```bash
# Disable unified path entirely
unset USE_UNIFIED_PATH

# Disable for specific workflow type
USE_UNIFIED_PATH=1 DISABLE_UNIFIED_MAPREDUCE=1

# Emergency revert
git revert HEAD~n
```

## Testing Strategy

### Phase Testing

Each phase requires:
1. Unit tests for new components
2. Integration tests comparing old vs new
3. Performance benchmarks
4. Manual testing of critical workflows

### Comparison Testing

```rust
#[tokio::test]
async fn test_both_paths_produce_same_result() {
    let workflow = load_test_workflow();
    let env = create_test_env();
    
    // Run through old path
    let old_result = OldOrchestrator::execute(&workflow, &env).await?;
    
    // Run through new unified path
    let new_result = UnifiedExecutor::execute(&workflow, &env).await?;
    
    // Results must be identical
    assert_eq!(old_result, new_result);
}
```

## Success Metrics

### Per Phase
- Phase 0: 100% test coverage of current behavior
- Phase 1: All components extracted with tests
- Phase 2: Normalization preserves all fields
- Phase 3: Unified path passes all tests
- Phase 4: Each workflow type migrated successfully
- Phase 5: Old code removed, all tests green

### Overall
- Validation works in 100% of paths (vs 25% currently)
- Zero performance regression
- Code reduction of ~70% in orchestrator
- Single test suite instead of 4

## Risk Mitigation

### High Risk Areas

1. **Variable Substitution**: Different names in each path
   - Mitigation: Aliases maintain compatibility
   
2. **MapReduce Complexity**: Worktrees and parallel execution
   - Mitigation: Migrate MapReduce last
   
3. **Validation Loss**: Current bug we're fixing
   - Mitigation: Test validation explicitly in each phase

4. **Breaking Changes**: User workflows might break
   - Mitigation: Feature flags and gradual rollout

## Timeline

| Week | Phase | Deliverable | Risk |
|------|-------|-------------|------|
| 1 | Phase 0 | Test suite | Low |
| 2 | Phase 1 | Common components | Low |
| 3 | Phase 2 | Normalization | Medium |
| 4 | Phase 3 | Unified path | Medium |
| 5 | Phase 4 | Migration | High |
| 6 | Phase 5 | Cleanup | Medium |
| 7 | Buffer | Documentation | Low |

Total: 7 weeks with buffer