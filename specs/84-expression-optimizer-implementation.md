---
number: 84
title: Expression Optimizer Implementation
category: optimization
priority: high
status: draft
dependencies: []
created: 2025-09-17
---

# Specification 84: Expression Optimizer Implementation

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The expression optimizer in `src/cook/execution/expression/optimizer.rs` is currently non-functional, returning expressions as-is without any optimization (line 107). This results in inefficient execution of complex expressions in workflows, particularly affecting:
- Variable interpolation with repeated sub-expressions
- Complex conditional logic in workflows
- MapReduce filter and transformation expressions
- Goal-seeking validation expressions

Without optimization, expressions are evaluated naively, leading to:
- Redundant calculations of identical sub-expressions
- Unnecessary variable lookups
- Inefficient string concatenations
- Poor performance in tight loops

For large-scale workflows processing thousands of items, this can significantly impact execution time and resource usage.

## Objective

Implement a comprehensive expression optimizer that applies standard optimization techniques to improve expression evaluation performance while maintaining correctness.

## Requirements

### Functional Requirements

1. **Constant Folding**
   - Evaluate constant expressions at compile time
   - Fold arithmetic operations with constants
   - Simplify boolean expressions
   - Pre-compute string concatenations

2. **Common Sub-expression Elimination**
   - Identify duplicate sub-expressions
   - Cache and reuse results
   - Handle scope-aware caching
   - Invalidate cache on variable changes

3. **Dead Code Elimination**
   - Remove unreachable expressions
   - Eliminate redundant conditionals
   - Remove no-op operations
   - Simplify always-true/false conditions

4. **Algebraic Simplification**
   - Apply mathematical identities
   - Simplify boolean logic
   - Optimize string operations
   - Reduce expression complexity

5. **Short-Circuit Evaluation**
   - Optimize AND/OR operations
   - Early exit for conditional expressions
   - Lazy evaluation of expensive operations
   - Skip unnecessary computations

### Non-Functional Requirements

1. **Performance**
   - Optimization overhead < 10ms for typical expressions
   - 30-50% performance improvement for complex expressions
   - Memory-efficient caching

2. **Correctness**
   - Preserve expression semantics
   - Handle edge cases properly
   - Maintain numeric precision

3. **Debuggability**
   - Option to disable optimization
   - Trace optimization steps
   - Preserve source mapping

## Acceptance Criteria

- [ ] Constant folding reduces expression complexity
- [ ] Common sub-expressions are evaluated only once
- [ ] Dead code is eliminated from expressions
- [ ] Algebraic simplifications are correctly applied
- [ ] Short-circuit evaluation improves performance
- [ ] All existing expression tests pass
- [ ] New tests validate each optimization technique
- [ ] Performance benchmarks show 30%+ improvement
- [ ] Optimization can be disabled for debugging
- [ ] No semantic changes to expression evaluation

## Technical Details

### Implementation Approach

1. **Replace Placeholder Implementation**
   ```rust
   // Current (line 107)
   // For now, just return the expression as-is

   // New implementation
   pub fn optimize(&mut self, expr: Expression) -> Result<Expression> {
       let mut optimized = expr;

       // Apply optimization passes
       optimized = self.constant_fold(optimized)?;
       optimized = self.eliminate_common_subexpressions(optimized)?;
       optimized = self.eliminate_dead_code(optimized)?;
       optimized = self.simplify_algebraic(optimized)?;
       optimized = self.apply_short_circuit(optimized)?;

       Ok(optimized)
   }
   ```

2. **Constant Folding**
   ```rust
   fn constant_fold(&self, expr: Expression) -> Result<Expression> {
       match expr {
           Expression::Binary(op, left, right) => {
               let left = self.constant_fold(*left)?;
               let right = self.constant_fold(*right)?;

               if let (Expression::Constant(l), Expression::Constant(r)) = (&left, &right) {
                   return Ok(Expression::Constant(eval_binary(op, l, r)?));
               }

               Ok(Expression::Binary(op, Box::new(left), Box::new(right)))
           }
           Expression::If(cond, then_expr, else_expr) => {
               let cond = self.constant_fold(*cond)?;

               if let Expression::Constant(Value::Bool(b)) = cond {
                   return Ok(if b {
                       self.constant_fold(*then_expr)?
                   } else {
                       self.constant_fold(*else_expr)?
                   });
               }

               // ... continue folding branches
           }
           _ => Ok(expr),
       }
   }
   ```

3. **Common Sub-expression Elimination**
   ```rust
   struct SubExpressionCache {
       expressions: HashMap<ExpressionHash, CachedResult>,
       access_count: HashMap<ExpressionHash, usize>,
   }

   fn eliminate_common_subexpressions(&mut self, expr: Expression) -> Result<Expression> {
       let hash = self.hash_expression(&expr);

       if let Some(cached) = self.cache.expressions.get(&hash) {
           self.cache.access_count.entry(hash).and_modify(|c| *c += 1);
           return Ok(Expression::Cached(cached.id));
       }

       let optimized = self.optimize_children(expr)?;

       if self.is_worth_caching(&optimized) {
           let id = self.cache_expression(optimized.clone())?;
           Ok(Expression::Cached(id))
       } else {
           Ok(optimized)
       }
   }
   ```

### Architecture Changes

- Add multi-pass optimization pipeline
- Implement expression hashing for caching
- Add cost model for optimization decisions
- Create optimization configuration system

### Data Structures

```rust
pub struct ExpressionOptimizer {
    config: OptimizerConfig,
    cache: SubExpressionCache,
    stats: OptimizationStats,
}

pub struct OptimizerConfig {
    pub enable_constant_folding: bool,
    pub enable_cse: bool,
    pub enable_dce: bool,
    pub enable_algebraic: bool,
    pub enable_short_circuit: bool,
    pub cache_threshold: usize,
    pub max_passes: usize,
}

pub struct OptimizationStats {
    pub expressions_optimized: usize,
    pub constants_folded: usize,
    pub subexpressions_eliminated: usize,
    pub dead_code_removed: usize,
    pub optimization_time: Duration,
}

pub enum OptimizedExpression {
    Original(Expression),
    Optimized {
        expr: Expression,
        original: Box<Expression>,
        optimizations: Vec<OptimizationStep>,
    },
}

pub struct OptimizationStep {
    pub technique: OptimizationTechnique,
    pub before: String,
    pub after: String,
    pub improvement: f64,
}
```

### APIs and Interfaces

```rust
pub trait ExpressionOptimizer {
    fn optimize(
        &mut self,
        expr: Expression,
        context: &OptimizationContext,
    ) -> Result<OptimizedExpression>;

    fn set_config(&mut self, config: OptimizerConfig);

    fn get_stats(&self) -> &OptimizationStats;

    fn reset_cache(&mut self);
}

pub trait OptimizationPass {
    fn apply(&self, expr: Expression) -> Result<Expression>;

    fn is_applicable(&self, expr: &Expression) -> bool;

    fn name(&self) -> &str;
}
```

## Dependencies

- **Prerequisites**: Expression parser and evaluator
- **Affected Components**:
  - Expression evaluator
  - Variable interpolation system
  - MapReduce filter evaluation
  - Goal-seeking validation
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Test each optimization technique individually
  - Verify correctness of optimizations
  - Edge cases and corner cases
  - Performance measurements

- **Property-Based Tests**:
  - Optimized expressions produce same results
  - Optimization preserves type safety
  - No infinite optimization loops

- **Integration Tests**:
  - Complex expressions with multiple optimizations
  - Real-world workflow expressions
  - Performance benchmarks

- **User Acceptance**:
  - Workflow execution with optimization enabled
  - Performance comparison with/without optimization
  - Debug mode with optimization disabled

## Documentation Requirements

- **Code Documentation**:
  - Document each optimization technique
  - Include examples of transformations
  - Performance characteristics

- **User Documentation**:
  - Guide to expression optimization
  - Performance tuning tips
  - Debugging optimized expressions

- **Architecture Updates**:
  - Update ARCHITECTURE.md with optimizer design
  - Document optimization pipeline
  - Include decision flowcharts

## Implementation Notes

- Start with constant folding as the foundation
- Implement CSE with configurable caching threshold
- Use visitor pattern for expression traversal
- Consider implementing optimization levels (O0, O1, O2)
- Add telemetry for optimization effectiveness
- Implement incremental optimization for interactive use
- Consider JIT compilation for hot expressions

## Migration and Compatibility

- Optimization is transparent to users
- Existing expressions work without modification
- Opt-in optimization for performance-critical workflows
- Debug mode preserves original behavior