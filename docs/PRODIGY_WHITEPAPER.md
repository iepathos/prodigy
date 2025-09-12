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
      ai: "/fix-tests"                 # AI command with configured provider
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
  ai: "/fix-tests --errors ${check.output}"
```

#### 2.1.2 Commands
Commands are the atomic units of execution:

- `shell:` - Execute shell commands
- `claude:` - Invoke Claude via CLI with markdown commands
- `ai:` - Future: Provider-agnostic AI invocation

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
  ai: "/fix-tests --errors '${check.output}' --attempt ${attempt}"

# Without explicit context, AI only sees what you pass
improve:
  ai: "/fix-tests"  # No automatic context injection
```

For workflows requiring memory of previous attempts, consider:
```yaml
goals:
  - achieve: "Tests passing"
    check: "cargo test 2>&1 | tee test-attempt-${attempt}.log"
    improve:
      ai: "/fix-tests --current-errors '${check.output}' --previous 'test-attempt-*.log'"
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
      claude: "/fix-tests --output '${check.output}'"
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
      claude: "/fix-tests --output '${check.output}'"
    
  - achieve: "Benchmarks improved"
    when: "${tests.passed} == true"
    check: "cargo bench"
    improve:
      claude: "/optimize-performance --benchmark '${check.output}'"
    
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
        ai: "/fix-imports"
      - when: "${check.output} contains 'type mismatch'"
        ai: "/fix-type-errors"
      - when: "${check.output} contains 'borrow checker'"
        ai: "/fix-lifetime-issues"
      - default:
        ai: "/fix-build-errors"
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

### 3.4 Parallel Execution Patterns

Prodigy provides two approaches for parallel processing: dedicated MapReduce workflows and parallel goal execution.

#### 3.4.1 MapReduce Workflows

For large-scale parallel processing, use the dedicated MapReduce mode:

```yaml
name: parallel-refactoring
mode: mapreduce

# Setup phase: Preparation and analysis
setup:
  - shell: "cargo test"  # Ensure tests pass before refactoring
  - shell: "find . -name '*.rs' > files.txt"
  - ai: "/analyze-codebase --output analysis.json"

# Map phase: Parallel processing of work items
map:
  input: files.txt                    # Input source
  # OR for JSON input:
  # input: analysis.json
  # json_path: "$.items[*]"          # JSONPath to extract items
  
  agent_template:
    commands:
      - ai: "/refactor-file ${item}"
        commit_required: true
      - shell: "cargo fmt -- ${item}"
      - shell: "cargo test"
        on_failure:
          ai: "/fix-test-failures --file ${item}"
          max_attempts: 2
  
  max_parallel: 5                     # Run up to 5 agents in parallel
  timeout_per_agent: 600s
  retry_on_failure: 1

# Reduce phase: Aggregate results and finalize
reduce:
  commands:
    - shell: "cargo test"              # Final test run
    - shell: "cargo clippy"
    - ai: |
        /summarize-refactoring \
          --successful ${map.successful} \
          --failed ${map.failed} \
          --total ${map.total}
      commit_required: true
```

#### 3.4.2 Goal-Based Parallel Processing

Goals can orchestrate parallel processing with setup, parallel execution, and aggregation phases:

```yaml
name: goal-based-refactoring
mode: goals  # Goal-seeking mode with parallel execution

goals:
  # Setup phase - runs sequentially before parallel processing
  - achieve: "Environment prepared"
    id: setup
    check: "cargo test"
    improve:
      shell: "cargo fix --allow-dirty"
    
  - achieve: "Work items identified"
    id: items
    requires: [setup]
    start:
      shell: "find . -name '*.rs' -type f > files.txt"
    check: "test -f files.txt && test -s files.txt"
    output: "files.txt"  # Output available to next goals
  
  # Parallel phase - processes items in parallel with goal-seeking per item
  - achieve: "All modules refactored and validated"
    id: refactor
    requires: [items]
    parallel:
      input: "${items.output}"        # Use output from previous goal
      max_parallel: 10
      isolation: worktree             # Each agent in git worktree
    # Per-item goal-seeking with validation and refinement
    check: "cargo clippy -- ${item} 2>&1 | grep -c warning"
    validate:
      target: "${check.output} == 0"  # No warnings
    improve:
      ai: "/refactor-module ${item} --warnings '${check.output}'"
    max_attempts: 3
    # Parallel execution results available as variables
    outputs:
      successful_items: "${parallel.successful}"
      failed_items: "${parallel.failed}"
      total_processed: "${parallel.total}"
  
  # Reduce phase - aggregates results after parallel execution
  - achieve: "Refactoring complete and tested"
    id: finalize
    requires: [refactor]
    check: "cargo test --all"
    improve:
      ai: |
        /fix-integration-issues \
          --successful ${refactor.successful_items} \
          --failed ${refactor.failed_items}
    on_success:
      ai: |
        /generate-refactoring-report \
          --total ${refactor.total_processed} \
          --successful ${refactor.successful_items} \
          --failed ${refactor.failed_items}
      commit_required: true
```

This approach combines goal-seeking's validation loops with MapReduce's parallel execution, providing:
- **Sequential setup** through goal dependencies
- **Parallel processing** with per-item validation and refinement
- **Result aggregation** through goal outputs and variables
- **Unified validation** ensuring both individual items and overall success

#### 3.4.3 Execution Semantics

Both patterns provide:
1. **Work distribution**: Items processed by parallel agents
2. **Git isolation**: Each agent runs in isolated git worktree
3. **Independent validation**: Each item validated separately
4. **Failure handling**: Failed items tracked in Dead Letter Queue
5. **Result aggregation**: Success/failure counts available to reduce phase

Choose MapReduce for batch processing, goals with parallel for iterative refinement per item.

### 3.5 State Management

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

#### 4.1.2 JSONPath-Based Validation
Using JSONPath expressions to evaluate any JSON structure:
```yaml
goals:
  - achieve: "80% test coverage"
    check:
      shell: "pytest --cov --json-report --json-report-file=coverage.json"
    validate:
      result_file: "coverage.json"
      target: "${result_file.totals.percent_covered} >= 80"
```

#### 4.1.3 Claude-Parsed Validation with Expressions
Using Claude to generate structured validation data:
```yaml
goals:
  - achieve: "Specification fully implemented"
    start:
      ai: "/implement-spec ${SPEC}"
    check:
      ai: "/validate-spec ${SPEC} --output validation.json"
    validate:
      result_file: "validation.json"
      # Boolean expression using JSONPath
      target: "${result_file.completion_percentage} >= 95"
    improve:
      # Access any field from the JSON for improvement
      ai: "/complete-spec ${SPEC} --gaps '${result_file.gaps}' --score ${result_file.completion_percentage}"
```

Example validation.json (from prodigy-validate-spec):
```json
{
  "completion_percentage": 85.0,
  "status": "incomplete",
  "implemented": ["Feature A", "Feature B"],
  "missing": ["Unit tests"],
  "gaps": {
    "missing_tests": {
      "description": "No tests for cleanup_worktree",
      "location": "src/worktree.rs:234",
      "severity": "high"
    }
  }
}
```

#### 4.1.4 Complex Boolean Expressions
Support for sophisticated validation logic:
```yaml
goals:
  - achieve: "Quality gates passed"
    check:
      shell: "sonarqube-scanner -Dsonar.format=json > quality.json"
    validate:
      result_file: "quality.json"
      # Multiple conditions with AND/OR
      target: |
        ${result_file.measures.coverage} >= 80 &&
        ${result_file.measures.bugs} == 0 &&
        ${result_file.measures.code_smells} < 10
        
  - achieve: "Performance benchmarks met"
    check:
      shell: "hyperfine --export-json bench.json './app'"
    validate:
      result_file: "bench.json"
      # Compare against baseline
      target: "${result_file.results[0].mean} < 0.100"  # Under 100ms
```

#### 4.1.5 Direct Shell Output Validation
For simple numeric outputs:
```yaml
goals:
  - achieve: "Line count under limit"
    check: "wc -l src/*.rs | tail -1 | awk '{print $1}'"
    validate:
      # Direct numeric comparison when check returns a number
      target: "${check.output} < 10000"
```

**Note**: All validation requires explicit expressions. There's no magic - you must specify exactly what to compare and how.

### 4.2 Validation Feedback Loop

The validation result influences the improvement phase through variable access:

1. **Check output** → Available as `${check.output}` (stdout/stderr)
2. **JSON fields** → Accessible via `${result_file.path.to.field}`
3. **Expression result** → Boolean from target evaluation
4. **Convergence** → Automatic termination when target expression evaluates to true

Example showing complete feedback loop:
```yaml
goals:
  - achieve: "Implementation complete"
    check:
      claude: "/validate-implementation --output result.json"
    validate:
      result_file: "result.json"
      target: "${result_file.completion_percentage} >= 95"
    improve:
      # All JSON fields are accessible for improvement
      claude: |
        /fix-implementation \
          --score ${result_file.completion_percentage} \
          --gaps '${result_file.gaps}' \
          --missing '${result_file.missing}'
    max_attempts: 5
```

The power of this approach:
- **No fixed schema** - Works with any JSON structure
- **Transparent logic** - Expression shows exactly what's being evaluated
- **Full access** - Any JSON field can be used in improvement commands
- **Type flexible** - Supports numbers, strings, booleans, arrays

---

## 5. Integration Architecture

### 5.1 Claude Integration

Prodigy is designed to work seamlessly with Claude through the Claude CLI and markdown command system:

#### 5.1.1 Claude Command Structure

```yaml
# Claude commands use markdown files in .claude/commands/
improve:
  claude: "/fix-issue"              # Invokes .claude/commands/fix-issue.md

# Pass arguments to commands
improve:
  claude: "/fix-tests --errors '${check.output}' --file ${item}"

# Commands can specify model preferences
improve:
  claude: "/complex-analysis"
  model: "claude-3-opus"            # Use Opus for complex tasks
  temperature: 0.2                  # Lower temperature for consistency
```

#### 5.1.2 Claude Configuration

Global configuration in `~/.prodigy/config.yml`:
```yaml
claude:
  # Model selection
  default_model: "claude-3-sonnet"
  
  # Model-specific settings
  models:
    claude-3-opus:
      temperature: 0.3
      max_tokens: 4096
      use_for: ["/analyze-*", "/design-*"]  # Complex reasoning tasks
      
    claude-3-sonnet:
      temperature: 0.5
      max_tokens: 4096
      use_for: ["/fix-*", "/refactor-*"]    # Code generation
      
    claude-3-haiku:
      temperature: 0.7
      max_tokens: 2048
      use_for: ["/format-*", "/lint-*"]     # Simple tasks

  # Cost optimization
  routing:
    - match: "/fix-simple-*"
      model: "claude-3-haiku"    # Use faster model for simple tasks
    - match: "/analyze-*"
      model: "claude-3-opus"     # Use best model for analysis
  
  # Caching configuration
  cache:
    enabled: true
    ttl: 3600
    path: "~/.prodigy/cache/claude"
```

Project-level overrides in `.prodigy/config.yml`:
```yaml
claude:
  default_model: "claude-3-opus"  # This project needs higher quality
  
  # Project-specific routing
  routing:
    - match: "/security-*"
      model: "claude-3-opus"      # Security requires best model
      temperature: 0.1            # Low temperature for consistency
```

#### 5.1.3 Claude Command System

Prodigy leverages Claude's markdown command system in `.claude/commands/`:

```markdown
# .claude/commands/fix-tests.md
Fix failing test cases

Arguments: $ARGUMENTS

## Instructions

1. Analyze the test failures provided
2. Identify root causes, not symptoms
3. Fix the underlying issues
4. Ensure no regressions
5. Add comments explaining the fix

## Context

You have access to:
- Full codebase via file reading
- Test execution via shell commands
- Git history for understanding changes

## Output

Provide fixes that:
- Address all test failures
- Maintain backward compatibility
- Include clear explanations
```

Prodigy automatically:
- Passes workflow variables as arguments
- Captures command output for validation
- Manages context between iterations
- Tracks token usage and costs

#### 5.1.4 Cost and Performance Optimization

```yaml
claude:
  # Intelligent model routing
  optimization:
    auto_routing: true            # Choose model based on task
    
    # Model selection strategy
    strategy:
      simple_tasks: "claude-3-haiku"
      standard_tasks: "claude-3-sonnet"
      complex_tasks: "claude-3-opus"
    
    # Cost controls
    budget:
      max_per_command: "$1.00"
      max_per_workflow: "$10.00"
      max_daily: "$100.00"
      
    # Performance tuning
    parallel_calls: 5             # Max concurrent Claude calls
    retry_on_rate_limit: true
    backoff_strategy: "exponential"
```

This Claude-focused approach provides:
- **Optimal Claude usage**: Right model for each task
- **Cost control**: Budget limits and smart routing
- **Performance**: Caching and parallel execution
- **Reliability**: Automatic retries and fallbacks

#### 5.1.5 Future Multi-Provider Support

While Prodigy v1.0 focuses on excellent Claude integration, the architecture is designed to support future expansion to other providers. The `ai:` command namespace is reserved for this future capability, where workflows could seamlessly switch between providers through configuration alone.

Future considerations:
- **OpenAI**: Function calling integration
- **Google Gemini**: Grounding and search integration  
- **Local models**: Ollama/llama.cpp support
- **Universal commands**: Single format working across all providers

For v1.0, the focus remains on making Claude workflows exceptional, with the confidence that the architecture can expand when needed.

### 5.2 Tool Integration

#### 5.2.1 Version Control
```yaml
goals:
  - achieve: "Changes committed"
    check: "git diff --exit-code"
    improve: "git add . && git commit -m '${message}'"
```

#### 5.2.2 CI/CD Systems

Prodigy workflows can be executed in CI/CD pipelines by installing and running the CLI:

```yaml
# GitHub Actions example
name: Automated PR Review
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  prodigy-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Prodigy
        run: |
          curl -sSL https://install.prodigy.dev | sh
          # Or: cargo install prodigy
      
      - name: Set up Claude API key
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
      
      - name: Run Prodigy workflow
        run: |
          # User must create .prodigy/workflows/review-pr.yml
          prodigy cook .prodigy/workflows/review-pr.yml
```

The user creates their own workflow file (e.g., `.prodigy/workflows/review-pr.yml`):
```yaml
name: review-pr
goals:
  - achieve: "Code review complete"
    check:
      shell: "git diff origin/main --name-only"
    improve:
      claude: "/review-changes --files '${check.output}'"
    
  - achieve: "Tests passing"
    check: "cargo test"
    improve:
      claude: "/fix-test-failures --output '${check.output}'"
```

Note: There is no official GitHub Action yet. Users run Prodigy via standard CLI commands in their CI/CD environment.

#### 5.2.3 Testing Frameworks
```yaml
goals:
  - achieve: "Unit tests passing"
    check: "pytest tests/unit"
    improve:
      claude: "/fix-test-failures --output '${check.output}'"
    
  - achieve: "Integration tests passing"
    check: "pytest tests/integration"
    improve:
      claude: "/fix-integration-issues --failures '${check.output}' --exit-code ${check.exit_code}"
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
      claude: "/implement-to-pass-tests --failures '${check.output}'"
```

### 6.2 Continuous Refactoring

```yaml
name: refactor-codebase
goals:
  - achieve: "No code smells detected"
    check: "sonarqube-scanner"
    improve:
      claude: "/fix-code-smells --report '${check.output}'"
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
      claude: "/generate-missing-docs --missing '${check.output}'"
    
  - achieve: "Examples provided"
    check: "./validate-examples.sh"
    improve:
      claude: "/add-usage-examples --validation '${check.output}'"
```

### 6.4 Security Patching

```yaml
name: security-fixes
goals:
  - achieve: "No critical vulnerabilities"
    check: "npm audit --audit-level=critical"
    improve:
      claude: "/fix-vulnerabilities --audit-report '${check.output}'"
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
      claude: "/fix-build --errors '${check.output}'"
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
