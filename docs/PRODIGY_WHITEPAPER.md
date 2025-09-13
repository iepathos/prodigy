# The Prodigy Workflow Language: A Declarative Approach to AI-Powered Automation

## Abstract

As Large Language Models become integral to software development workflows, the lack of determinism, reproducibility, and cost control presents significant challenges for production use. Current approaches rely on imperative scripts that fail to handle the iterative, non-deterministic nature of LLM interactions.

This paper introduces the Prodigy Workflow Language, a declarative domain-specific language (DSL) for orchestrating AI-powered automation. By expressing workflows as goal-oriented specifications rather than step-by-step procedures, Prodigy enables convergence toward validated outcomes from non-deterministic AI operations through systematic validation, iteration, and feedback mechanisms.

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
- **Iterative refinement**: Systematic retry with validation feedback
- **Observable execution**: Full transparency into decision-making and progress

### 1.3 Contributions

This paper makes the following contributions:

1. A formal specification for a goal-oriented workflow language for AI automation
2. An execution model that achieves convergence toward validated outcomes despite non-deterministic operations
3. A validation framework that guides iteration toward specified goals
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
      claude: "/fix-tests"             # AI command to fix issues
    max_attempts: 5                   # Iteration limit (with adaptive strategies available)
```

**How it works:**
1. **Validation**: The `check` command (`npm test`) executes as a shell command
2. **Output capture**: Stdout and stderr are captured in `${check.output}` 
3. **Context passing**: On failure, the output is available to the improve command via template variables
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
5. **Regression prevention**: Automatic rollback if iteration makes things worse

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

#### 3.2.1 Regression Prevention

Prodigy prevents iterations from making things worse:

```yaml
goals:
  - achieve: "Code optimized without breaking functionality"
    # Track metrics across iterations
    check: |
      pytest --json-report --json-report-file=test-results.json && \
      performance-check --output perf-results.json
    
    validate:
      tests_passing: "${test-results.summary.passed} == ${test-results.summary.total}"
      performance: "${perf-results.score}"
    
    # Prevent regression
    regression_guard:
      compare_to: "previous_iteration"
      metrics:
        - test_pass_rate: "must_not_decrease"
        - performance_score: "allow_decrease: 0.05"  # Allow 5% regression
        - code_coverage: "must_not_decrease"
      
      on_regression:
        action: "rollback"  # Automatically revert
        notify: true
        retry_with: "/fix-without-regression --constraint '${regression.details}'"
    
    improve:
      claude: "/optimize-code --baseline '${checkpoint.best.metrics}'"
      commit_required: true
```

#### 3.2.2 Validation-Driven Context Scoping

A key design principle of Prodigy is that validation-driven iteration helps work within provider context limits:

**Context Window Reality**: LLM providers like Claude have fixed context limits (e.g., 200K tokens for Claude 3, 1M for Claude 3.5)
**Prodigy Approach**: Each iteration receives fresh context based on current state, not accumulated history

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
2. These specific gaps are explicitly passed to the LLM via `${check.output}`
3. LLM sees the current code state and validation results
4. No need to track previous iterations - the current code reflects all changes

This approach provides benefits:
- **Works within provider limits**: Each call fits within the model's context window
- **Current state focus**: AI sees what the code is now, not how it got there
- **Explicit control**: Developers decide what context to pass via template variables
- **No history accumulation**: Avoids context bloat from previous attempts

The validation output combined with the current code state provides all necessary context. Since iteration 6 can see the current code (which embodies all previous changes), it doesn't need to know about iterations 1-5. Context limits are managed by the provider/model, not Prodigy.

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
1. **Work distribution**: Items processed by parallel agents in isolated worktrees
2. **Git isolation**: Each agent runs in its own git worktree, preventing file conflicts
3. **Independent execution**: Map phase agents work independently without coordination
4. **Merge reconciliation**: The reduce phase handles merging all worktree changes back together
5. **Conflict resolution**: Reduce phase agent resolves any merge conflicts from parallel changes
6. **Failure handling**: Failed items tracked in Dead Letter Queue
7. **Result aggregation**: Success/failure counts and outputs available to reduce phase

The key insight is that parallel agents don't need to coordinate during the map phase - they work independently in isolated worktrees. The reduce phase then acts as the reconciliation point, where a dedicated agent merges all changes and resolves any conflicts that arise from the parallel modifications.

Choose MapReduce for batch processing, goals with parallel for iterative refinement per item.

### 3.5 State Management and Checkpoints

#### 3.5.1 Checkpoint System

Prodigy automatically creates checkpoints to enable rollback and recovery:

```yaml
goals:
  - achieve: "Complex refactoring complete"
    checkpoint:
      strategy: "every_iteration"  # Default: checkpoint after each iteration
      include:
        - git_commit: true         # Git commit hash
        - variables: true           # All workflow variables
        - validation_scores: true   # Validation results
        - metrics: true            # Performance/quality metrics
    
    check: "make test && make benchmark"
    improve:
      claude: "/refactor --metrics '${check.output}'"
      commit_required: true  # Ensure git commit created
    
    # Automatic rollback if regression detected
    rollback:
      trigger: "${check.score} < ${checkpoint.best_score}"
      to: "best_checkpoint"  # Roll back to best-scoring iteration
```

#### 3.5.2 Commit Tracking

Every modification must be tracked in git:

```yaml
commands:
  - claude: "/modify-code"
    commit_required: true  # Validates commit was created
    commit_message: "Iteration ${attempt}: ${achieve}"
    
  - shell: "cargo fmt"
    commit_required: false  # Formatting doesn't require commit
    
  - claude: "/generate-tests"  
    commit_required: true
    commit_validation:
      files_changed: "> 0"
      tests_added: "git diff HEAD~1 --stat | grep test"
```

#### 3.5.3 Rollback Mechanisms

Multiple rollback strategies for different scenarios:

```yaml
goals:
  - achieve: "Performance optimized"
    check: "benchmark --json > results.json"
    validate:
      performance: "${results.time} < ${baseline.time}"
    
    rollback:
      # Automatic rollback on regression
      auto_rollback:
        on_regression: true
        on_test_failure: true
        on_validation_decrease: 0.1  # 10% worse than previous
      
      # Manual rollback options
      manual_rollback:
        enable_command: "prodigy rollback --to-best"
        preserve_learning: true  # Keep learned patterns
      
      # Checkpoint selection strategy
      checkpoint_selection:
        strategy: "best_validation_score"
        fallback: "last_passing"
        max_checkpoints: 10  # Keep last 10 checkpoints
```

#### 3.5.4 State Persistence

Comprehensive state tracking across iterations:

```yaml
state:
  checkpoint_dir: ".prodigy/checkpoints/${workflow_id}"
  
  per_iteration:
    - git_commit: "${git rev-parse HEAD}"
    - timestamp: "${date -u +%Y-%m-%dT%H:%M:%SZ}"
    - variables: "${all_workflow_variables}"
    - metrics:
        validation_score: "${check.score}"
        test_coverage: "${coverage.percentage}"
        performance: "${benchmark.results}"
    - ai_context:
        tokens_used: "${claude.tokens}"
        confidence: "${claude.confidence}"
        
  recovery:
    on_crash: "restore_from_checkpoint"
    on_resume: "continue_from_last_successful"
```

#### 3.5.5 Checkpoint Comparison

Built-in tools for comparing iterations:

```yaml
goals:
  - achieve: "Best solution found"
    checkpoint:
      compare_iterations: true
      
    after_iteration:
      - shell: |
          prodigy compare \
            --current . \
            --best checkpoints/best \
            --output comparison.md
      
      - claude: |
          /analyze-comparison \
            --report comparison.md \
            --decide "keep_current or rollback"
    
    decision:
      if: "${claude.decision} == 'rollback'"
      then:
        rollback: "best"
      else:
        promote: "current_to_best"
```

Prodigy maintains execution state at multiple levels:

- **Session state**: Overall workflow progress with checkpoint history
- **Goal state**: Per-goal iteration history with rollback points
- **Command state**: Individual command results with git commits
- **Context state**: Accumulated knowledge with ability to revert
- **Checkpoint state**: Complete snapshots for recovery and comparison

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
4. **Convergence** → Termination when target expression evaluates to true

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

## 5. Real-World Scenarios

### 5.1 Architectural Refactoring

Large-scale architectural changes require coordinated analysis, planning, and execution:

```yaml
name: microservices-migration
goals:
  - achieve: "Architecture analyzed and migration plan created"
    id: analysis
    check:
      claude: "/analyze-monolith --output architecture.json"
    validate:
      confidence: 0.9
      components_identified: "${check.output.components.length} > 0"
    
  - achieve: "Service boundaries defined"
    requires: [analysis]
    check:
      composite:
        - claude: "/define-service-boundaries --arch '${analysis.output}'"
          weight: 0.5
        - shell: "domain-analyzer --validate boundaries.json"
          weight: 0.3
        - claude: "/review-cohesion --boundaries boundaries.json"
          weight: 0.2
    validate:
      threshold: 8.5  # High quality threshold
    improve:
      claude: "/refine-boundaries --feedback '${check.output}'"
    max_attempts: 10  # Allow extensive refinement
    
  - achieve: "Services extracted and tested"
    requires: [boundaries]
    parallel:
      input: "${boundaries.output.services}"
      max_parallel: 5
    check: |
      docker build -t ${item.name} ${item.path} && \
      docker-compose -f test-${item.name}.yml up --abort-on-container-exit
    improve:
      claude: "/fix-service --name ${item.name} --errors '${check.output}'"
```

### 5.2 API Design and Evolution

API design involves subjective quality criteria and stakeholder alignment:

```yaml
name: api-redesign
goals:
  - achieve: "API spec modernized with backward compatibility"
    check:
      claude: |
        /modernize-api \
          --spec openapi.yml \
          --constraints "backward compatible, versioned"
    validate:
      composite:
        - shell: "spectral lint new-api.yml --ruleset .spectral.yml"
          weight: 0.3
        - claude: "/evaluate-developer-experience --spec new-api.yml"
          weight: 0.4
        - shell: "backwards-compat-check old-api.yml new-api.yml"
          weight: 0.3
      threshold: 9.0  # Very high quality bar
    improve:
      claude: "/refine-api --feedback '${check.output}'"
    attempts:
      initial: 5
      extend_if_improving: true
      max_total: 15
```

### 5.3 Performance Optimization

Performance work requires iterative refinement with measurable outcomes:

```yaml
name: performance-optimization
goals:
  - achieve: "Database queries optimized"
    check:
      composite:
        - shell: "pgbadger postgresql.log --outfile query-report.html"
        - shell: "explain-analyzer --slow-queries > slow.json"
        - claude: "/analyze-query-patterns --report slow.json"
    improve:
      claude: |
        /optimize-queries \
          --slow-queries '${check[1].output}' \
          --schema schema.sql
    validate:
      # Measure actual performance improvement
      shell: "benchmark --baseline baseline.json --current optimized.json"
      improvement: "${validate.output.improvement} >= 0.25"  # 25% improvement
    budget:
      max_cost: 10.00  # Cap spending on optimization attempts
```

### 5.4 Security Hardening

Security requires comprehensive validation across multiple dimensions:

```yaml
name: security-hardening
goals:
  - achieve: "Vulnerabilities identified and fixed"
    check:
      composite:
        - shell: "snyk test --json > snyk-report.json"
        - shell: "semgrep --config=auto --json > semgrep-report.json"  
        - claude: "/analyze-security-reports --reports '*.json'"
    validate:
      critical_issues: "${check.output.critical_count} == 0"
      high_issues: "${check.output.high_count} <= 3"
    improve:
      claude: "/fix-vulnerabilities --report '${check.output}'"
      
  - achieve: "Authentication system hardened"
    check:
      claude: |
        /audit-auth-system \
          --standards "OWASP, OAuth2, OIDC"
    validate:
      confidence: 0.95  # Very high confidence required
    improve:
      claude: "/harden-auth --issues '${check.output.issues}'"
```

### 5.5 Technical Debt Reduction

Managing technical debt across large codebases:

```yaml
name: tech-debt-reduction
goals:
  - achieve: "Legacy patterns modernized"
    parallel:
      input: "find . -name '*.py' -exec grep -l 'deprecated_pattern' {} +"
      max_parallel: 10
    check:
      claude: "/identify-legacy-patterns --file ${item}"
    improve:
      claude: |
        /modernize-code \
          --file ${item} \
          --preserve-behavior true
    validate:
      # Ensure behavior preservation
      shell: "pytest tests/${item.stem}_test.py"
        
  - achieve: "Test coverage improved"
    check:
      shell: "coverage report --format json | jq '.total_coverage'"
    validate:
      target: "${check.output} >= 80"
    improve:
      claude: "/generate-missing-tests --coverage-gaps coverage-gaps.json"
    attempts:
      initial: 3
      extend_if_improving: true
      improvement_threshold: 0.02  # 2% coverage improvement
```

---

## 6. Integration Architecture

### 6.1 Claude Integration

Prodigy is designed to work seamlessly with Claude through the Claude CLI and markdown command system:

#### 6.1.1 Claude Command Structure

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

#### 6.1.2 Claude Configuration

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

#### 6.1.3 Claude Command System

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

Prodigy:
- Passes workflow variables as arguments via templates
- Captures command output for validation checks
- Provides context through explicit template variables
- Tracks token usage and costs

#### 6.1.4 Cost and Performance Optimization

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
- **Efficiency**: Caching and parallel execution
- **Reliability**: Configurable retries and fallbacks

#### 6.1.5 Multi-Provider Architecture

Prodigy is architected from the ground up to support multiple AI providers, though v1.0 prioritizes exceptional Claude integration. The design separates provider-specific implementation from the core workflow engine:

```yaml
# Provider-agnostic design (future)
goals:
  - achieve: "Code refactored"
    check: "make test"
    improve:
      ai: "/refactor"  # Provider determined by config
      provider: "${PRODIGY_PROVIDER}"  # Or explicit override
      
# Current Claude-focused implementation
goals:
  - achieve: "Code refactored"
    check: "make test"
    improve:
      claude: "/refactor"  # Explicit Claude usage
```

The architecture ensures:
- **Command abstraction**: Commands are provider-independent concepts
- **Configuration-driven routing**: Switch providers without changing workflows
- **Provider-specific optimizations**: Each provider can have custom handling
- **Fallback chains**: Define primary and backup providers

While v1.0 focuses on Claude excellence, the foundational architecture supports seamless multi-provider expansion without breaking existing workflows.

### 6.2 Tool Integration

#### 6.2.1 Version Control
```yaml
goals:
  - achieve: "Changes committed"
    check: "git diff --exit-code"
    improve: "git add . && git commit -m '${message}'"
```

#### 6.2.2 CI/CD Systems

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

#### 6.2.3 Testing Frameworks
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

### 7.3 Design Considerations

#### 7.3.1 Parallel Execution Trade-offs
- Parallel execution benefits large-scale operations
- Overhead may not justify parallelization for small tasks
- Configurable parallelization based on workload size

#### 7.3.2 Resource Management
- Context size bounded by provider limits
- Trade-off between detail and processing capacity
- User-configurable resource limits

#### 7.3.3 Network Considerations
- API rate limits require throttling
- Retry strategies for transient failures
- Cost implications of retries and parallel calls

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

### 8.3 Execution Properties

While individual LLM calls remain non-deterministic, Prodigy provides:
- **Bounded execution**: Guaranteed termination via max_attempts
- **Convergence tracking**: Progress toward validation criteria
- **Reproducible structure**: Same workflow steps executed in same order
- **Auditable decisions**: All validation results and retry logic logged

Note: The actual LLM outputs will vary between runs. Prodigy manages this non-determinism through validation and iteration rather than eliminating it.

---

## 9. Addressing Practical Challenges

### 9.1 Validation for Subjective Criteria

Traditional validation assumes binary pass/fail, but many development goals involve subjective quality. Prodigy addresses this through:

#### 9.1.1 AI-Assisted Validation
```yaml
goals:
  - achieve: "Code is well-refactored"
    check:
      claude: "/evaluate-refactoring --criteria 'SOLID principles, DRY, clarity'"
    validate:
      confidence: 0.85  # Accept if AI confidence >= 85%
    improve:
      claude: "/refactor --feedback '${check.output}'"
```

#### 9.1.2 Composite Validation
```yaml
goals:
  - achieve: "High quality API design"
    check:
      composite:
        - shell: "spectral lint openapi.yml --format json"
          weight: 0.4
        - claude: "/evaluate-api-design --score"
          weight: 0.6
    validate:
      threshold: 8.0  # Weighted score must exceed threshold
```

### 9.2 Adaptive Iteration Strategies

Fixed `max_attempts` is too rigid. Prodigy supports adaptive strategies:

#### 9.2.1 Progress-Based Adaptation
```yaml
goals:
  - achieve: "Performance optimized"
    check: "benchmark --format json | jq '.score'"
    improve:
      claude: "/optimize-performance"
    attempts:
      initial: 3
      extend_if_improving: true  # Add attempts if making progress
      improvement_threshold: 0.1  # Require 10% improvement to continue
      max_total: 10
```

#### 9.2.2 Cost-Aware Budgeting
```yaml
goals:
  - achieve: "Complete refactoring"
    budget:
      max_tokens: 100000     # Token limit
      max_cost: 5.00        # Dollar limit
      strategy: "front_loaded"  # Use more resources early
    check: "make validate"
    improve:
      claude: "/refactor"
```

### 9.3 Declarative Preconditions

Instead of imperative `start` commands, use declarative preconditions:

```yaml
goals:
  - achieve: "Tests passing"
    preconditions:
      - exists: "node_modules"
        ensure: "npm install"
      - exists: ".env"
        ensure:
          claude: "/generate-env-file"
    check: "npm test"
    improve:
      claude: "/fix-tests"
```

### 9.4 Validation Templates

Reduce validation burden through reusable templates:

```yaml
templates:
  rust_quality:
    check:
      - shell: "cargo build --release"
      - shell: "cargo test"
      - shell: "cargo clippy -- -D warnings"
    weights: [0.2, 0.5, 0.3]

goals:
  - achieve: "Production ready"
    validate_with: rust_quality
    improve:
      claude: "/fix-issues --report '${check.output}'"
```

### 9.5 Workflow Composability

Prodigy supports Ansible-style workflow composition for maximum reusability:

#### 9.5.1 Including External Workflows

```yaml
# test-validation.yml - Reusable test validation workflow
name: test-validation
goals:
  - achieve: "All tests passing with coverage"
    check: 
      composite:
        - shell: "cargo test"
        - shell: "cargo tarpaulin --out Json | jq '.coverage'"
    validate:
      all_tests_pass: "${check[0].exit_code} == 0"
      coverage_threshold: "${check[1].output} >= 80"
    improve:
      claude: "/fix-tests --coverage-report '${check.output}'"
    export:  # Make results available to parent workflow
      coverage: "${check[1].output}"
      test_status: "${check[0].exit_code}"
```

```yaml
# main-workflow.yml - Includes the test validation
name: feature-implementation
goals:
  - achieve: "Feature implemented"
    include: "workflows/implement-feature.yml"
    
  - achieve: "Tests validated"
    include: "workflows/test-validation.yml"  # Include reusable workflow
    inputs:
      min_coverage: 85
    
  - achieve: "Documentation updated"
    when: "${test-validation.coverage} >= 90"  # Reference included workflow outputs
    include: "workflows/update-docs.yml"
```

#### 9.5.2 Workflow Libraries

```yaml
# Import shared workflow libraries
imports:
  - "@prodigy/standard-validations"  # Community-maintained validations
  - "./company-workflows"            # Organization-specific workflows
  - "https://github.com/org/workflows"  # Remote workflow repositories

goals:
  - achieve: "Production ready"
    include: "@prodigy/standard-validations/rust-production"
    overrides:
      coverage_threshold: 95  # Override default values
```

#### 9.5.3 Nested Composition

```yaml
# Workflows can compose other workflows recursively
name: full-release
stages:
  - name: "Quality checks"
    include: 
      - "workflows/lint-validation.yml"
      - "workflows/test-validation.yml"
      - "workflows/security-scan.yml"
    parallel: true  # Run included workflows in parallel
    
  - name: "Build and deploy"
    requires: ["Quality checks"]
    include: "workflows/build-deploy.yml"
    inputs:
      environment: "${DEPLOY_ENV}"
      test_results: "${quality-checks.outputs}"
```

#### 9.5.4 Parameterized Workflows

```yaml
# Reusable parameterized workflow
name: generic-validation
parameters:
  build_command:
    type: string
    default: "make build"
  test_command:
    type: string
    default: "make test"
  lint_command:
    type: string
    default: "make lint"

goals:
  - achieve: "Code validated"
    check:
      - shell: "${params.build_command}"
      - shell: "${params.test_command}"
      - shell: "${params.lint_command}"
```

```yaml
# Using the parameterized workflow
name: my-project
goals:
  - achieve: "Rust project validated"
    include: "workflows/generic-validation.yml"
    parameters:
      build_command: "cargo build --release"
      test_command: "cargo test --all"
      lint_command: "cargo clippy -- -D warnings"
```

This composability enables:
- **DRY principle**: Write validation logic once, use everywhere
- **Standardization**: Share best practices across teams
- **Modularity**: Combine small, focused workflows into complex pipelines
- **Versioning**: Pin specific versions of included workflows
- **Testing**: Test workflows in isolation before composition

## 10. Safety and Safeguards

### 10.1 Progressive Rollout for Scale

Prodigy enforces staged deployment for large-scale modifications:

```yaml
name: safe-large-scale-refactoring
safeguards:
  progressive_rollout:
    enabled: true  # Mandatory for >10 files
    stages:
      - canary: 3 files
        human_review: required
      - pilot: 10% of files
        auto_rollback: true
      - production: remaining
        circuit_breaker: 0.1  # Stop if >10% fail

goals:
  - achieve: "Refactoring safely deployed"
    parallel:
      input: "find . -name '*.py'"
      max_parallel: 100
    validate:
      # Multiple validation layers
      - syntax: "python -m py_compile ${item}"
      - tests: "pytest tests/${item.stem}_test.py"
      - behavior: "diff-detector --ensure-equivalent ${item}"
      - security: "semgrep --config=auto ${item}"
```

### 10.2 Defense-in-Depth Validation

Prodigy addresses validation trust through multiple verification layers:

```yaml
goals:
  - achieve: "High confidence validation"
    validation_ensemble:
      - technical:
          command: "make test"
          weight: 0.3
      - behavioral:
          command: "e2e-test --comprehensive"
          weight: 0.3
      - quality:
          command:
            claude: "/assess-quality"
          weight: 0.2
      - comparative:
          command: "behavior-diff --before HEAD~1"
          weight: 0.2
    
    confidence_required: 0.9  # Ensemble score
    
    # Acknowledge limitations
    validation_gaps:
      - "Performance under load"
      - "Long-term maintainability"
    manual_review: recommended
```

### 10.3 Blast Radius Control

Limit the impact of potential failures:

```yaml
safeguards:
  blast_radius:
    max_files_per_iteration: 10
    isolation:
      - git_worktree: true  # Each agent isolated
      - rollback_on_failure: true
      - test_before_merge: true
    
  pattern_detection:
    abort_on:
      - "rm -rf"
      - "DROP TABLE"
      - "eval\\("
    alert_on_systematic_changes: true
    
  human_checkpoints:
    required_at:
      - after_canary: true
      - after_pilot: true
      - random_sampling: 5%  # Review 5% randomly
```

### 10.4 Emergency Controls

Built-in safety mechanisms:

```yaml
safeguards:
  kill_switch:
    enabled: true
    triggers:
      - failure_rate: 0.2
      - unexpected_patterns: 5
      - security_violations: any
    
  rollback:
    automatic: true
    checkpoint_before: true
    test_rollback: true
    
  audit_trail:
    log_all_changes: true
    track_validation_misses: true
    report_anomalies: true
```

## 11. Limitations and Future Work

### 11.1 Current Limitations

1. **No distributed execution**: Single-machine parallelism only
2. **Limited state persistence**: Session state not durable across crashes
3. **Static validation only**: No runtime type checking
4. **No dynamic workflow generation**: Workflows must be predefined

### 10.2 Planned Extensions

#### 10.2.1 Distributed Execution
```yaml
cluster:
  nodes:
    - "worker-1.example.com"
    - "worker-2.example.com"
  
map:
  distributed: true
  max_parallel: 100  # Across all nodes
```

#### 10.2.2 Event-Driven Workflows
```yaml
triggers:
  - on: "github.pull_request.opened"
    workflow: "review-pr.yml"
  - on: "schedule.daily"
    workflow: "maintenance.yml"
```

### 10.3 Research Directions

1. **Formal verification** of workflow properties
2. **Machine learning** for optimal provider selection
3. **Workflow generation assistance** from natural language
4. **Cross-workflow optimization** and resource sharing

---

## 11. Why Prodigy: The Value Proposition

### 11.1 Beyond IDE Integration

While IDE-integrated AI assistants excel at point solutions, they struggle with:

**Scale and Coordination**
- Single-file focus vs. codebase-wide changes
- No orchestration across multiple operations
- Manual coordination of multi-step tasks

Prodigy provides:
```yaml
# Refactor entire codebase in parallel
map:
  input: "find . -name '*.py'"
  max_parallel: 20
  agent_template:
    commands:
      - claude: "/modernize-code ${item}"
      - shell: "pytest tests/${item.stem}_test.py"
```

### 11.2 Beyond Programmatic Frameworks

Programmatic AI frameworks offer flexibility but require:

**Development Overhead**
- Writing validation logic for every workflow
- Managing retry logic and error handling
- Building parallelization infrastructure
- Maintaining complex codebases for automation

Prodigy's declarative approach:
```yaml
# Same power, 10x less code
goals:
  - achieve: "API modernized"
    check: "spectral lint api.yml"
    improve:
      claude: "/modernize-api"
    attempts:
      extend_if_improving: true  # Built-in adaptive logic
```

**Key Advantages**:
- **10x less code**: Declarative > imperative for workflows
- **Built-in patterns**: Validation, retry, parallelization included
- **Git-friendly**: YAML workflows are reviewable, versionable
- **No programming required**: Accessible to entire team

### 11.3 Beyond Manual Coordination

Human review with AI assistance is safe but doesn't scale:

**Manual Bottlenecks**
- Human must orchestrate each step
- No parallel execution across files
- Inconsistent approaches between developers
- Knowledge isn't captured or reusable

Prodigy enables:
```yaml
# Captured, repeatable, parallel expertise
name: security-audit
goals:
  - achieve: "Security audit complete"
    include: "@company/security-workflows/full-audit"
    parallel:
      max_parallel: 50  # Audit 50 components simultaneously
    validate:
      compliance: ["SOC2", "HIPAA", "PCI"]
```

### 11.4 The Synthesis Advantage

Prodigy combines the best of all approaches:

| Aspect | IDE Assistant | Programmatic Framework | Human + AI | Prodigy |
|--------|--------------|----------------------|------------|---------|
| **Scale** | Single file | Requires coding | Manual coordination | Built-in parallel execution |
| **Validation** | None | Custom code | Human judgment | Declarative + AI-assisted |
| **Repeatability** | No | Yes, with code | No | Yes, with workflows |
| **Team Sharing** | No | Via codebase | Informal | Workflow libraries |
| **Learning Curve** | Low | High | Low | Medium |
| **Safety** | Limited | Programmatic | Human control | Validation-driven |
| **Cost Control** | None | Custom | Manual | Built-in budgets |

### 11.5 Unique Capabilities

Prodigy enables workflows that are difficult or impossible with other approaches:

**1. Single-File Tasks with Validation**
```yaml
# Even single-file tasks benefit from validation loops
goals:
  - achieve: "Complex refactoring complete"
    check: "pytest tests/module_test.py -v"
    improve:
      claude: "/refactor-module --file module.py"
    validate:
      all_tests_pass: true
      performance: "benchmark.py module.py | jq '.time' < 100"
```

**2. Parallel Codebase Operations**
```yaml
# Scale from 1 to 1000 files with same workflow
map:
  input: "legacy_files.txt"
  max_parallel: 100
  agent_template:
    commands:
      - claude: "/modernize ${item}"
    validate:
      shell: "make test-${item.stem}"
```

**3. Composable Team Knowledge**
```yaml
# Reuse refined workflows across projects
imports:
  - "@team/validated-workflows/testing"
  - "@team/validated-workflows/security"
  - "@team/validated-workflows/performance"
```

**4. Adaptive Quality Gates**
```yaml
# Self-adjusting based on progress
goals:
  - achieve: "Quality improved"
    budget:
      max_cost: 10.00
    attempts:
      extend_if_improving: true
      diminishing_returns: 0.1
```

**5. Traceable AI Operations**
```yaml
# Full audit trail of AI decisions
goals:
  - achieve: "Compliance validated"
    check:
      claude: "/audit-compliance"
    validate:
      confidence: 0.95
      export_reasoning: true  # Capture AI's reasoning
```

### 11.6 When to Choose Prodigy

Prodigy excels when you need:

✅ **Validation**: Systematic quality assurance for any task
✅ **Scale**: Operations from single files to entire codebases  
✅ **Repeatability**: Workflows you'll run again or share
✅ **Parallelization**: Concurrent AI operations
✅ **Iteration**: Complex tasks requiring refinement
✅ **Cost Control**: Budgeted AI operations
✅ **Audit Trail**: Traceable AI decisions

Consider alternatives when:

⚠️ **Pure exploration**: No clear success criteria
⚠️ **Instant feedback**: Sub-second response required
⚠️ **Custom algorithms**: Complex procedural logic
⚠️ **UI interaction**: Tasks requiring visual feedback

### 11.7 Return on Investment

The investment in learning Prodigy pays off through:

**Time Savings**
- Parallel execution across entire codebases
- Reusable workflows eliminate repetitive work
- Batch processing of similar tasks

**Quality Improvements**
- Systematic validation catches issues
- Consistent approaches across team
- Best practices encoded in workflows

**Risk Reduction**
- Validation gates prevent bad changes
- Git worktree isolation ensures safety
- Budget controls prevent runaway costs

**Team Efficiency**
- Junior developers can run expert workflows
- Knowledge captured and shared
- Reduced cognitive load for complex tasks

---

## 12. Related Work

Prodigy builds upon established concepts from multiple domains:

### 12.1 Workflow Orchestration
The DAG-based execution model draws from Apache Airflow and similar systems, while the durable execution patterns are influenced by Temporal's approach to reliability.

### 12.2 Configuration Management
The declarative syntax and idempotent operations are inspired by Ansible and Terraform, adapting their principles to AI-driven workflows.

### 12.3 Build Systems
The dependency resolution and parallel execution strategies leverage concepts from Make and modern build systems like Bazel.

### 12.4 AI Orchestration
The goal-oriented approach and validation loops represent a novel contribution to the emerging field of AI workflow orchestration.

---

## 13. Conclusion

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
