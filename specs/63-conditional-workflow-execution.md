---
number: 63
title: Conditional Workflow Execution with When Clauses
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 63: Conditional Workflow Execution with When Clauses

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The whitepaper shows conditional workflow execution as a core feature:
```yaml
tasks:
  - name: "Deploy if tests pass"
    when: "${tests.passed}"
    shell: "./deploy.sh"
```

Currently, Prodigy lacks proper conditional execution support. While there are on_failure handlers, there's no general-purpose `when` clause for controlling workflow flow based on conditions, previous step results, or variable values.

## Objective

Implement comprehensive conditional execution support using `when` clauses, enabling dynamic workflow control based on expressions, variables, and previous step outcomes.

## Requirements

### Functional Requirements
- Support `when` clause on any workflow step
- Expression evaluation for boolean conditions
- Access to previous step results via variables
- Support comparison operators: `==`, `!=`, `>`, `<`, `>=`, `<=`
- Boolean operators: `&&`, `||`, `!`
- String and numeric comparisons
- Check for variable existence: `${var.exists}`
- Skip steps when condition evaluates to false
- Clear logging of skipped steps

### Non-Functional Requirements
- Fast expression evaluation (< 1ms)
- Safe expression execution (no code injection)
- Clear error messages for invalid expressions
- Intuitive syntax matching common languages

## Acceptance Criteria

- [ ] `when: "${build.success}"` works for boolean variables
- [ ] `when: "${exit_code} == 0"` works for numeric comparison
- [ ] `when: "${env} == 'production'"` works for string comparison
- [ ] `when: "${tests.passed} && ${lint.passed}"` works with boolean operators
- [ ] Complex expressions: `when: "${score} >= 80 || ${override} == true"`
- [ ] Undefined variables treated as false/null appropriately
- [ ] Skipped steps logged with reason
- [ ] Invalid expressions provide helpful error messages
- [ ] Nested variable access: `when: "${result.data.status} == 'ready'"`
- [ ] Integration with MapReduce and foreach constructs

## Technical Details

### Implementation Approach

1. **Expression Evaluator**:
   ```rust
   pub struct ExpressionEvaluator {
       parser: ExpressionParser,
       variable_resolver: VariableResolver,
   }

   impl ExpressionEvaluator {
       pub fn evaluate(
           &self,
           expression: &str,
           context: &VariableContext,
       ) -> Result<bool> {
           // Parse expression into AST
           let ast = self.parser.parse(expression)?;

           // Evaluate with variable context
           let result = self.evaluate_node(&ast, context)?;

           // Convert to boolean
           self.to_boolean(result)
       }

       fn evaluate_node(
           &self,
           node: &AstNode,
           context: &VariableContext,
       ) -> Result<Value> {
           match node {
               AstNode::Variable(name) => {
                   self.variable_resolver.resolve(name, context)
               }
               AstNode::Comparison { left, op, right } => {
                   let left_val = self.evaluate_node(left, context)?;
                   let right_val = self.evaluate_node(right, context)?;
                   self.compare(left_val, op, right_val)
               }
               AstNode::Logical { left, op, right } => {
                   let left_bool = self.to_boolean(
                       self.evaluate_node(left, context)?
                   )?;
                   match op {
                       LogicalOp::And => {
                           if !left_bool {
                               return Ok(Value::Bool(false));
                           }
                           self.evaluate_node(right, context)
                       }
                       LogicalOp::Or => {
                           if left_bool {
                               return Ok(Value::Bool(true));
                           }
                           self.evaluate_node(right, context)
                       }
                   }
               }
               AstNode::Literal(val) => Ok(val.clone()),
           }
       }
   }
   ```

2. **Enhanced Workflow Step**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ConditionalStep {
       #[serde(flatten)]
       pub step: WorkflowStep,

       /// Condition for execution
       #[serde(skip_serializing_if = "Option::is_none")]
       pub when: Option<String>,
   }

   impl ConditionalStep {
       pub async fn should_execute(
           &self,
           context: &ExecutionContext,
       ) -> Result<bool> {
           match &self.when {
               Some(expr) => {
                   let evaluator = ExpressionEvaluator::new();
                   evaluator.evaluate(expr, &context.variables)
               }
               None => Ok(true),
           }
       }
   }
   ```

3. **Variable Context Management**:
   ```rust
   #[derive(Debug, Clone)]
   pub struct VariableContext {
       variables: HashMap<String, Value>,
       step_results: HashMap<String, StepResult>,
   }

   impl VariableContext {
       pub fn set_step_result(&mut self, name: &str, result: StepResult) {
           // Store step result
           self.step_results.insert(name.to_string(), result.clone());

           // Make common fields available as variables
           self.variables.insert(
               format!("{}.success", name),
               Value::Bool(result.success),
           );
           self.variables.insert(
               format!("{}.exit_code", name),
               Value::Number(result.exit_code.into()),
           );
           if let Some(output) = result.output {
               self.variables.insert(
                   format!("{}.output", name),
                   Value::String(output),
               );
           }
       }
   }
   ```

### Architecture Changes
- Add `ExpressionEvaluator` module
- Enhance `WorkflowStep` with `when` field
- Extend `VariableContext` for step results
- Modify executor to check conditions before execution
- Add expression validation during workflow parsing

### Data Structures
```yaml
# Example conditional workflow
tasks:
  - name: "build"
    shell: "cargo build --release"

  - name: "test"
    shell: "cargo test"
    when: "${build.success}"

  - name: "deploy-staging"
    shell: "./deploy.sh staging"
    when: "${test.success} && ${branch} != 'main'"

  - name: "deploy-production"
    shell: "./deploy.sh production"
    when: "${test.success} && ${branch} == 'main' && ${manual_approval} == true"

  - name: "notify-failure"
    shell: "notify-slack 'Build failed'"
    when: "${build.success} == false || ${test.success} == false"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/workflow/` - Add conditional evaluation
  - `src/cook/execution/` - Skip logic for conditions
  - `src/config/workflow.rs` - Parse when clauses
- **External Dependencies**: Consider `evalexpr` crate for expression parsing

## Testing Strategy

- **Unit Tests**:
  - Expression parsing and evaluation
  - Variable resolution
  - Boolean conversion logic
  - Operator precedence
- **Integration Tests**:
  - Conditional workflows end-to-end
  - Variable propagation between steps
  - Skipped step handling
  - Complex nested expressions
- **Error Tests**:
  - Invalid expression syntax
  - Type mismatches in comparisons
  - Undefined variable handling

## Documentation Requirements

- **Code Documentation**: Document expression syntax and evaluation
- **User Documentation**:
  - Conditional execution guide
  - Expression syntax reference
  - Common patterns and examples
- **Architecture Updates**: Add conditional flow to execution diagrams

## Implementation Notes

- Start with simple comparisons, add complex expressions later
- Consider caching parsed expressions for performance
- Provide dry-run mode to test conditions without execution
- Support debugging with condition evaluation logs
- Future: Support regex matching and function calls

## Migration and Compatibility

- Workflows without `when` clauses work unchanged
- No breaking changes to existing workflows
- Clear migration guide for converting on_failure to when
- Backwards compatible with current handler system