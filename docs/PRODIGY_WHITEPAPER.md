# The Prodigy Workflow Language: A Declarative Approach to AI-Powered Automation

## Abstract

As Large Language Models become integral to software development workflows, the lack of determinism, reproducibility, and cost control presents significant challenges for production use. Current approaches rely on imperative scripts that fail to handle the iterative, non-deterministic nature of LLM interactions.

This paper introduces the Prodigy Workflow Language, a declarative domain-specific language (DSL) for orchestrating AI-powered automation. By expressing workflows as goal-oriented specifications rather than step-by-step procedures, Prodigy enables deterministic execution of non-deterministic AI operations through systematic validation, iteration, and convergence mechanisms.

---

## 1. Introduction

### 1.1 Problem Statement

Large Language Models have demonstrated remarkable capabilities in code generation, refactoring, and analysis tasks. However, integrating these capabilities into production workflows faces three fundamental challenges:

1. **Non-determinism**: Identical prompts produce different outputs across invocations
2. **Lack of validation**: No systematic way to verify AI-generated outputs meet requirements
3. **Cost unpredictability**: Uncontrolled iteration leads to unbounded API costs

Traditional scripting approaches fail because they assume deterministic, single-pass execution—assumptions that don't hold for LLM interactions.

### 1.2 Design Philosophy

The Prodigy Workflow Language is built on four core principles:

- **Goal-oriented**: Specify desired outcomes, not implementation steps
- **Validation-driven**: Every AI interaction is validated against explicit criteria
- **Iterative refinement**: Automatic retry with validation feedback
- **Observable execution**: Full transparency into decision-making and progress

### 1.3 Contributions

This paper makes the following contributions:

1. A formal specification for a goal-oriented workflow language for AI automation
2. An execution model that provides deterministic outcomes from non-deterministic operations
3. A validation framework that ensures convergence toward specified goals
4. Integration patterns for incorporating AI workflows into existing development toolchains

---

## 2. Language Design

### 2.1 Core Concepts

#### 2.1.1 Goals
A goal represents a desired state with validation criteria:

```yaml
goals:
  - achieve: "All tests passing"      # Human-readable description
    check: "npm test"                 # Validation command (shell)
    improve:                           # Improvement command
      claude: "/fix-tests"             # Claude CLI command
    target: 100                       # Success threshold (percentage)
    max_attempts: 5                   # Iteration limit
```

**How it works:**
1. **Validation**: The `check` command (`npm test`) executes as a shell command
2. **Output capture**: Stdout and stderr are captured in `${check.output}` 
3. **Context passing**: On failure, the output is automatically available to the improve command
4. **Improvement**: The Claude command receives the test failures as context
5. **Iteration**: This repeats until tests pass or max_attempts is reached

The improve command can explicitly reference the validation output:
```yaml
improve:
  claude: "/fix-tests --errors ${check.output}"
```

#### 2.1.2 Commands
Commands are the atomic units of execution:

- `shell:` - Execute shell commands
- `claude:` - Invoke Claude through CLI

#### 2.1.3 Workflows
Workflows compose goals into larger automation pipelines:

```yaml
name: implement-feature
description: "Implement a feature from specification"
goals:
  - achieve: "Implementation complete"
    # ... goal specification
  - achieve: "Tests written and passing"
    # ... goal specification
```

### 2.2 Language Grammar

The Prodigy Workflow Language uses YAML as its concrete syntax. The abstract syntax is defined as:

```
Workflow ::= name: String
           | description: String?
           | variables: Map<String, Any>?
           | goals: Goal+

Goal ::= achieve: String
       | check: Command
       | improve: Command
       | start: Command?
       | target: Number?
       | max_attempts: Number?
       | on_failure: Command?
       | parallel: ParallelSpec?

Command ::= String | CommandSpec

CommandSpec ::= command: String
              | provider: String?
              | timeout: Number?
              | retry: RetrySpec?

ParallelSpec ::= input: String
               | max_parallel: Number?
               | strategy: String?
```

### 2.3 Type System

The language employs a simple type system:

- **Primitive types**: String, Number, Boolean
- **Composite types**: List, Map
- **Command types**: Shell, Claude
- **Result types**: Success, Failure, Timeout

Variable interpolation supports path expressions:
- `${variable}` - Simple variable reference
- `${item.field}` - Nested field access
- `${map.results[0]}` - Array indexing

---

## 3. Execution Model

### 3.1 Goal Achievement Algorithm

```
function achieve_goal(goal):
    attempt = 0
    
    if goal.start:
        execute(goal.start)
    
    while attempt < goal.max_attempts:
        validation = execute(goal.check)
        
        if validation.success:
            if validation.score >= goal.target:
                return Success
        
        # Key insight: validation output becomes improvement context
        # The improve command has access to:
        # - ${check.output}: Combined stdout/stderr from validation
        # - ${check.exit_code}: Exit code from validation
        # - ${check.stderr}: Just stderr if needed
        # - ${attempt}: Current attempt number
        
        improvement = execute(goal.improve, validation_context=validation)
        attempt += 1
    
    if goal.on_failure:
        execute(goal.on_failure)
    
    return Failure
```

### 3.2 Iterative Refinement

Each improvement iteration receives validation feedback to guide refinement:

1. **Initial state**: Baseline from `start` command (if provided)
2. **Validation feedback**: Output from failed `check` commands is available as `${check.output}`
3. **Attempt tracking**: Current attempt number available as `${attempt}`
4. **Convergence detection**: Automatic termination when progress plateaus

**Important Note**: Context passing must be explicit in the improve command. Prodigy does NOT automatically inject previous attempt history. If you need historical context, you must explicitly pass it:

```yaml
# Explicit context passing (transparent and recommended)
improve:
  claude: "/fix-tests --errors '${check.output}' --attempt ${attempt}"

# Without explicit context, Claude only sees what you pass
improve:
  claude: "/fix-tests"  # No automatic context injection
```

For workflows requiring memory of previous attempts, consider:
```yaml
goals:
  - achieve: "Tests passing"
    check: "cargo test 2>&1 | tee test-attempt-${attempt}.log"
    improve:
      claude: "/fix-tests --current-errors '${check.output}' --previous 'test-attempt-*.log'"
```

### 3.2.1 Automatic Context Scoping Through Validation

A key innovation of Prodigy is that validation-driven iteration naturally solves the context window challenge that plagues traditional LLM integrations:

**Traditional Problem**: Managing what information to include in limited context windows
**Prodigy Solution**: Validation output automatically provides exactly the relevant context

Example:
```yaml
goals:
  - achieve: "All tests passing"
    check: "cargo test"
    improve:
      claude: "/fix-tests"
```

Each iteration:
1. Validation identifies current gaps (failing tests)
2. Only these gaps are passed to the LLM
3. LLM focuses on specific, actionable problems
4. No accumulation of irrelevant history

This approach is superior because:
- **Self-limiting**: Context never grows beyond current problems
- **Self-focusing**: Always addresses the most relevant issues
- **Self-correcting**: Each iteration gets fresh perspective
- **No manual management**: No chunking, windowing, or prioritization needed

The validation command acts as an intelligent filter, transforming potentially unlimited context into precisely scoped, actionable information. This eliminates the need for complex context window management strategies like sliding windows, chunking, or manual prioritization that burden traditional LLM integrations.

### 3.3 Conditional Execution and Branching

Prodigy supports conditional execution through boolean expressions, enabling workflows to adapt based on runtime conditions:

#### 3.3.1 Simple Conditionals

Goals can be conditionally executed using the `when:` clause:

```yaml
goals:
  - achieve: "Tests passing"
    check: "cargo test"
    improve:
      claude: "/fix-tests"
    
  - achieve: "Benchmarks improved"
    when: "${tests.passed} == true"
    check: "cargo bench"
    improve:
      claude: "/optimize-performance"
    
  - achieve: "Deploy to production"
    when: "${branch} == 'main' && ${benchmarks.passed}"
    check: "kubectl get deployment"
    improve: "kubectl apply -f k8s/"
```

#### 3.3.2 Error-Specific Strategies

Different improvement strategies based on validation output:

```yaml
goals:
  - achieve: "Build successful"
    check: "cargo build"
    improve:
      - when: "${check.output} contains 'unresolved import'"
        command: "claude: /fix-imports"
      - when: "${check.output} contains 'type mismatch'"
        command: "claude: /fix-type-errors"
      - when: "${check.output} contains 'borrow checker'"
        command: "claude: /fix-lifetime-issues"
      - default:
        command: "claude: /fix-build-errors"
```

#### 3.3.3 Boolean Expression Support

Conditions support common boolean operators:

- **Comparison**: `==`, `!=`, `>`, `<`, `>=`, `<=`
- **Logical**: `&&`, `||`, `!`
- **String matching**: `contains`, `starts_with`, `ends_with`
- **Regex**: `matches`
- **Existence**: `exists`, `is_empty`

Example:
```yaml
when: "${exit_code} != 0 && ${retry_count} < 3"
when: "${output} matches 'error:.*timeout' || ${duration} > 300"
when: "!${cache.exists} && ${env.CI} == 'true'"
```

### 3.4 Parallel Execution

The MapReduce pattern enables parallel processing:

```yaml
goals:
  - achieve: "All modules refactored"
    parallel:
      input: "find . -name '*.py'"    # Generate work items
      max_parallel: 10                # Concurrency limit
    check: "pylint ${item}"           # Per-item validation
    improve:
      claude: "/refactor ${item}"    # Per-item improvement
```

Execution semantics:
1. **Map phase**: Distribute work items to parallel workers
2. **Execute phase**: Each worker processes items independently
3. **Reduce phase**: Aggregate results and determine success

### 3.4 State Management

Prodigy maintains execution state at multiple levels:

- **Session state**: Overall workflow progress
- **Goal state**: Per-goal iteration history
- **Command state**: Individual command results
- **Context state**: Accumulated knowledge across iterations

---

## 4. Validation Framework

### 4.1 Validation Types

#### 4.1.1 Binary Validation
Success determined by exit code:
```yaml
check: "npm test"  # Success if exit code = 0
```

#### 4.1.2 Threshold Validation
Success determined by score:
```yaml
check: "pytest --cov"
target: 80  # Require 80% coverage
```

#### 4.1.3 Custom Validation
Complex validation logic:
```yaml
check: 
  command: "./validate.sh"
  success_pattern: "VALID"
  extract_score: 'Score: (\d+)'
```

### 4.2 Validation Feedback Loop

The validation result influences the improvement phase:

1. **Error messages** → Passed to improvement command
2. **Partial success** → Score guides refinement intensity
3. **Regression detection** → Rollback mechanisms
4. **Convergence** → Automatic termination

---

## 5. Integration Architecture

### 5.1 LLM Provider Abstraction

Prodigy abstracts LLM interactions through a provider interface:

```yaml
improve:
  command: "/fix-issue"
  provider: "claude-3.5"      # Provider selection
  temperature: 0.2            # Provider-specific parameters
  max_tokens: 4000
```

Provider capabilities:
- **Multi-provider support**: Claude, GPT-4, Llama, etc.
- **Automatic failover**: Switch providers on failure
- **Cost optimization**: Route based on task complexity
- **Caching layer**: Reuse previous responses

### 5.2 Tool Integration

#### 5.2.1 Version Control
```yaml
goals:
  - achieve: "Changes committed"
    check: "git diff --exit-code"
    improve: "git add . && git commit -m '${message}'"
```

#### 5.2.2 CI/CD Systems
```yaml
# GitHub Actions integration
on:
  pull_request:
    types: [opened, synchronize]
jobs:
  prodigy:
    runs-on: ubuntu-latest
    steps:
      - uses: prodigy/action@v1
        with:
          workflow: .prodigy/review.yml
```

#### 5.2.3 Testing Frameworks
```yaml
goals:
  - achieve: "Unit tests passing"
    check: "pytest tests/unit"
    improve:
      claude: "/fix-test-failures"
    
  - achieve: "Integration tests passing"
    check: "pytest tests/integration"
    improve:
      claude: "/fix-integration-issues"
```

### 5.3 Context Management

Prodigy maintains context across iterations:

```
~/.prodigy/
├── events/
│   └── {repo_name}/
│       └── {job_id}/
│           └── events-{timestamp}.jsonl
├── state/
│   └── {repo_name}/
│       └── sessions/
│           └── {session_id}/
│               ├── context.json
│               └── iterations/
└── cache/
    └── {provider}/
        └── {hash}.json
```

---

## 6. Usage Patterns

### 6.1 Test-Driven Development

```yaml
name: tdd-cycle
goals:
  - achieve: "Tests written for feature"
    start: "claude: /write-tests-from-spec ${SPEC_FILE}"
    check: "test -f tests/test_${FEATURE}.py"
    
  - achieve: "Tests failing appropriately"
    check: "! pytest tests/test_${FEATURE}.py"
    
  - achieve: "Implementation passing tests"
    check: "pytest tests/test_${FEATURE}.py"
    improve:
      claude: "/implement-to-pass-tests"
```

### 6.2 Continuous Refactoring

```yaml
name: refactor-codebase
goals:
  - achieve: "No code smells detected"
    check: "sonarqube-scanner"
    improve:
      claude: "/fix-code-smells"
    parallel:
      input: "find src -name '*.js'"
      max_parallel: 5
```

### 6.3 Documentation Generation

```yaml
name: maintain-docs
goals:
  - achieve: "API documentation complete"
    check: "npx docgen --check"
    improve:
      claude: "/generate-missing-docs"
    
  - achieve: "Examples provided"
    check: "./validate-examples.sh"
    improve:
      claude: "/add-usage-examples"
```

### 6.4 Security Patching

```yaml
name: security-fixes
goals:
  - achieve: "No critical vulnerabilities"
    check: "npm audit --audit-level=critical"
    improve:
      claude: "/fix-vulnerabilities"
    max_attempts: 3
    on_failure: "shell: echo 'Manual intervention required' | tee SECURITY.md"
```

---

## 7. Implementation Details

### 7.1 Caching Strategy

Prodigy implements multi-level caching:

1. **Response cache**: LLM responses by hash(prompt + context)
2. **Validation cache**: Test results with TTL
3. **Workflow cache**: Entire workflow outputs for replay

Cache invalidation:
- File modification timestamps
- Dependency version changes
- Explicit cache busting

### 7.2 Error Handling

Error handling follows a hierarchical approach:

```yaml
goals:
  - achieve: "Build successful"
    check: "make build"
    improve:
      claude: "/fix-build"
    on_failure:                    # Goal-level handler
      command: "git reset --hard"
    
workflow_on_failure:                # Workflow-level handler
  command: "prodigy notify-team"
  
global_on_failure:                  # Global handler (config file)
  command: "prodigy rollback"
```

### 7.3 Performance Considerations

#### 7.3.1 Parallel Execution
- Work stealing scheduler for load balancing
- Automatic batching for small work items
- Resource limits to prevent system overload

#### 7.3.2 Memory Management
- Streaming processing for large outputs
- Context pruning after N iterations
- Lazy loading of historical data

#### 7.3.3 Network Optimization
- Request batching for LLM calls
- Automatic retry with exponential backoff
- Provider-specific rate limiting

---

## 8. Formal Semantics

### 8.1 Operational Semantics

We define the operational semantics using small-step reduction:

```
⟨goal, σ⟩ →* ⟨success, σ'⟩  if validation passes
⟨goal, σ⟩ →* ⟨failure, σ'⟩  if max_attempts exceeded

where σ represents the execution state:
σ ::= ⟨context, iterations, cache⟩
```

### 8.2 Convergence Properties

Theorem: Every goal with finite `max_attempts` terminates.

Proof sketch: Each iteration either:
1. Succeeds (terminates with success)
2. Fails validation (increments attempt counter)
3. The attempt counter is bounded by `max_attempts`

### 8.3 Determinism Guarantees

Given identical:
- Initial state
- Random seeds
- Cache contents
- External service responses

The workflow execution is deterministic and reproducible.

---

## 9. Limitations and Future Work

### 9.1 Current Limitations

1. **No distributed execution**: Single-machine parallelism only
2. **Limited state persistence**: Session state not durable across crashes
3. **No workflow composition**: Cannot call workflows from workflows
4. **Static validation only**: No runtime type checking

### 9.2 Planned Extensions

#### 9.2.1 Workflow Composition
```yaml
goals:
  - achieve: "Feature complete"
    workflow: "implement-feature.yml"
    inputs:
      spec: ${SPEC_FILE}
```

#### 9.2.2 Conditional Execution
```yaml
goals:
  - achieve: "Deployment ready"
    when: "${BRANCH} == 'main'"
    check: "kubectl diff"
    improve: "kubectl apply"
```

#### 9.2.3 Event-Driven Workflows
```yaml
triggers:
  - on: "github.pull_request.opened"
    workflow: "review-pr.yml"
  - on: "schedule.daily"
    workflow: "maintenance.yml"
```

### 9.3 Research Directions

1. **Formal verification** of workflow properties
2. **Machine learning** for optimal provider selection
3. **Automatic workflow generation** from natural language
4. **Cross-workflow optimization** and resource sharing

---

## 10. Related Work

### 10.1 Workflow Orchestration
- **Apache Airflow**: DAG-based orchestration, but not AI-native
- **Temporal**: Durable execution, but imperative programming model
- **Prefect**: Python-native, but lacks goal-oriented semantics

### 10.2 AI Development Tools
- **LangChain**: LLM application framework, but imperative
- **GitHub Copilot**: IDE integration, but no workflow orchestration
- **Cursor/Windsurf**: AI-powered IDEs, but single-user focused

### 10.3 Domain-Specific Languages
- **Ansible**: Declarative automation, but not AI-aware
- **Terraform**: Infrastructure as code, inspired goal-oriented approach
- **Make**: Build automation, influenced dependency model

---

## 11. Conclusion

The Prodigy Workflow Language provides a systematic approach to AI-powered automation through goal-oriented specifications, iterative refinement, and validation-driven execution. By abstracting the complexity of LLM interactions behind a declarative interface, Prodigy enables developers to leverage AI capabilities while maintaining the predictability and control required for production systems.

The language's focus on goals rather than procedures represents a fundamental shift in how we approach AI automation—from imperatively controlling every step to declaratively specifying desired outcomes and letting the system determine the path to achievement.

---

## Appendix A: Complete Grammar Specification

```yaml
# EBNF Grammar for Prodigy Workflow Language

Workflow     ::= WorkflowMeta Goals
WorkflowMeta ::= "name:" String 
               | "description:" String?
               | "variables:" Variables?
               | "inputs:" Inputs?

Goals        ::= "goals:" Goal+
Goal         ::= "- achieve:" String
               & CheckSpec
               & ImproveSpec?
               & StartSpec?
               & TargetSpec?
               & AttemptsSpec?
               & ParallelSpec?
               & OnFailureSpec?

CheckSpec    ::= "check:" Command
ImproveSpec  ::= "improve:" Command
StartSpec    ::= "start:" Command
TargetSpec   ::= "target:" Number
AttemptsSpec ::= "max_attempts:" Number
ParallelSpec ::= "parallel:" Parallel
OnFailureSpec::= "on_failure:" Command

Command      ::= String | CommandObject
CommandObject::= "command:" String
               & "provider:" String?
               & "timeout:" Number?

Parallel     ::= "input:" String
               & "max_parallel:" Number?
               & "strategy:" ("fifo" | "lifo" | "random")?

Variables    ::= (String ":" Value)+
Value        ::= String | Number | Boolean | List | Map
```

---

## Appendix B: Standard Library Commands

### B.1 Code Quality Commands
- `/fix-linting-errors` - Resolve linting issues
- `/fix-type-errors` - Fix TypeScript/Python type errors
- `/fix-test-failures` - Debug and fix failing tests
- `/improve-coverage` - Add tests to improve coverage

### B.2 Refactoring Commands
- `/extract-function` - Extract code into functions
- `/simplify-logic` - Reduce complexity
- `/remove-duplication` - DRY principle application
- `/modernize-syntax` - Update to modern language features

### B.3 Documentation Commands
- `/write-docstrings` - Add function documentation
- `/update-readme` - Maintain README accuracy
- `/generate-api-docs` - Create API documentation
- `/add-examples` - Include usage examples

### B.4 Security Commands
- `/fix-vulnerabilities` - Patch security issues
- `/add-input-validation` - Prevent injection attacks
- `/implement-auth` - Add authentication
- `/add-rate-limiting` - Prevent abuse

---

## Appendix C: Configuration Reference

### C.1 Global Configuration
```yaml
# ~/.prodigy/config.yml
providers:
  default: claude-3.5
  fallback: gpt-4
  
cache:
  enabled: true
  ttl: 3600
  max_size: 1GB
  
execution:
  max_parallel: 10
  timeout_default: 300
  retry_attempts: 3
  
storage:
  events_dir: ~/.prodigy/events
  state_dir: ~/.prodigy/state
  cache_dir: ~/.prodigy/cache
```

### C.2 Project Configuration
```yaml
# .prodigy/config.yml
extends: ~/.prodigy/config.yml

overrides:
  providers:
    default: gpt-4  # Project prefers GPT-4
    
  validation:
    strict_mode: true
    require_tests: true
    
  context:
    include:
      - "src/**/*.ts"
      - "tests/**/*.ts"
    exclude:
      - "node_modules/**"
      - "dist/**"
```

---

## References

1. Workflow Orchestration Patterns - van der Aalst et al.
2. Declarative Programming Paradigms - Lloyd, J.W.
3. Large Language Models as Tool Makers - Schick et al.
4. Convergence in Iterative Systems - Bertsekas, D.P.

---

## Acknowledgments

The Prodigy Workflow Language design has been influenced by conversations with the developer community and lessons learned from production deployments. Special thanks to early adopters who provided invaluable feedback on the goal-oriented approach.

---

**Document Version**: 1.0.0  
**Last Updated**: January 2025  
**License**: MIT  
**Repository**: github.com/iepathos/prodigy  
**Documentation**: docs.prodigy.dev