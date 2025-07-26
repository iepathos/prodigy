# Feature: Iterative Improvement Loop Integration

## Objective
Integrate a self-sufficient iterative improvement loop directly into MMM's workflow system that can automatically chain Claude CLI review and improvement commands until code quality targets are achieved, working around Claude CLI session limitations through MMM's existing orchestration infrastructure.

## Acceptance Criteria
- [ ] New `mmm loop` CLI command for starting iterative improvement sessions
- [ ] Integration with existing MMM workflow engine for orchestration
- [ ] Enhanced Claude commands that output structured data for automation
- [ ] State persistence across iterations using existing MMM state management
- [ ] Convergence detection and intelligent termination conditions
- [ ] Git integration with safety mechanisms and automated commits
- [ ] Monitoring and reporting integration with existing MMM analytics
- [ ] Template-based workflow configuration for different improvement scenarios

## Technical Details

### 1. New Module: `src/loop/mod.rs`

```rust
pub mod engine;
pub mod config;
pub mod metrics;
pub mod session;

pub use engine::IterationEngine;
pub use config::{LoopConfig, TerminationCondition, QualityTarget};
pub use metrics::{LoopMetrics, IterationResult};
pub use session::{LoopSession, SessionState};

/// Main iterative improvement loop engine
pub struct IterationEngine {
    workflow_engine: Arc<WorkflowEngine>,
    claude_manager: Arc<ClaudeManager>,
    state_manager: Arc<StateManager>,
    config: LoopConfig,
}

/// Configuration for iterative improvement loops
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopConfig {
    pub target_score: f64,
    pub max_iterations: u32,
    pub scope: Vec<String>,
    pub severity_filter: Vec<SeverityLevel>,
    pub termination_conditions: Vec<TerminationCondition>,
    pub safety_settings: SafetySettings,
    pub workflow_template: String,
}

/// Termination condition types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TerminationCondition {
    TargetAchieved { threshold: f64 },
    MaxIterations { count: u32 },
    DiminishingReturns { min_improvement: f64, consecutive_iterations: u32 },
    NoAutomatedActions,
    QualityRegression { threshold: f64 },
    TimeLimit { duration: Duration },
    UserIntervention,
}

/// Safety settings for loop execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetySettings {
    pub create_git_stash: bool,
    pub validate_compilation: bool,
    pub run_tests: bool,
    pub max_file_changes_per_iteration: usize,
    pub rollback_on_regression: bool,
}
```

### 2. CLI Integration: Enhanced `main.rs`

Add new subcommand structure:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...
    
    /// Iterative improvement loop commands
    #[command(subcommand)]
    Loop(LoopCommands),
}

#[derive(Subcommand)]
enum LoopCommands {
    /// Start an iterative improvement session
    Start {
        /// Target quality score (0.0-10.0)
        #[arg(short, long, default_value = "8.5")]
        target: f64,
        
        /// Maximum iterations
        #[arg(short, long, default_value = "3")]
        max_iterations: u32,
        
        /// Code scope to improve (files/directories)
        #[arg(short, long, default_value = "src/")]
        scope: String,
        
        /// Severity levels to address
        #[arg(long, default_value = "critical,high")]
        severity: String,
        
        /// Workflow template to use
        #[arg(short, long, default_value = "code-quality-improvement")]
        workflow: String,
        
        /// Run in dry-run mode
        #[arg(long)]
        dry_run: bool,
    },
    
    /// List active and completed loop sessions
    Sessions {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
        
        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    
    /// Show detailed session information
    Show {
        /// Session ID
        session_id: String,
    },
    
    /// Stop a running session
    Stop {
        /// Session ID
        session_id: String,
        
        /// Force stop without cleanup
        #[arg(short, long)]
        force: bool,
    },
    
    /// Resume a paused session
    Resume {
        /// Session ID  
        session_id: String,
    },
    
    /// Configure default loop settings
    Config {
        /// Configuration key
        #[arg(short, long)]
        key: Option<String>,
        
        /// Configuration value
        #[arg(short, long)]
        value: Option<String>,
        
        /// List current settings
        #[arg(short, long)]
        list: bool,
    },
}
```

### 3. Workflow Templates

#### Core Template: `templates/workflows/code-quality-improvement.yaml`

```yaml
name: code-quality-improvement
description: Iterative code quality improvement workflow
version: "1.0"

triggers:
  - type: manual

parameters:
  target_score:
    type: float
    default: 8.5
    description: Target quality score to achieve
  max_iterations:
    type: integer  
    default: 3
    description: Maximum number of improvement iterations
  scope:
    type: string
    default: "src/"
    description: Code scope to analyze and improve
  severity_filter:
    type: string
    default: "critical,high"
    description: Comma-separated severity levels to address

stages:
  - name: initialization
    steps:
      - name: setup-session
        type: command
        command: loop-session-init
        outputs: [session_id, baseline_metrics]
        
      - name: create-git-stash
        type: command
        command: git-safety-stash
        condition: "git_has_changes()"
        outputs: [stash_id]

  - name: iteration-loop
    condition: "!termination_conditions_met()"
    for_each: "range(1, {{ max_iterations + 1 }})"
    steps:
      - name: review-phase
        type: command
        command: claude-review
        message: "Running code review (iteration {{ iteration }})"
        outputs: [review_results, quality_score, actionable_items]
        timeout: "10m"
        
      - name: evaluate-termination
        type: command
        command: check-termination-conditions
        outputs: [should_terminate, termination_reason]
        
      - name: improvement-phase
        type: command
        command: claude-improve
        condition: "!should_terminate"
        message: "Applying automated improvements (iteration {{ iteration }})"
        outputs: [improvement_results, changes_applied]
        timeout: "15m"
        
      - name: validation-phase
        type: command
        command: validate-changes
        condition: "changes_applied > 0"
        message: "Validating improvements"
        outputs: [validation_results, tests_passed]
        
      - name: iteration-checkpoint
        type: checkpoint
        condition: "validation_results.has_issues"
        message: "Iteration {{ iteration }} completed with issues. Continue?"
        options: [continue, stop, rollback]
        timeout: "30m"
        
      - name: update-metrics
        type: command
        command: update-iteration-metrics
        outputs: [iteration_metrics]

  - name: finalization
    steps:
      - name: generate-report
        type: command
        command: generate-improvement-report
        outputs: [final_report, summary_metrics]
        
      - name: create-commit
        type: command
        command: git-commit-improvements
        condition: "summary_metrics.total_improvements > 0"
        outputs: [commit_hash]
        
      - name: cleanup-session
        type: command
        command: loop-session-cleanup

on_success:
  - type: notification
    message: "Iterative improvement completed successfully. Final score: {{ final_score }}"
    
on_failure:
  - type: notification
    message: "Iterative improvement failed: {{ error_message }}"
  - type: custom
    command: rollback-changes
    condition: "safety_settings.rollback_on_failure"
```

### 4. Enhanced Claude Command Integration

#### Modified `src/claude/commands.rs`

Add structured output support for automation:

```rust
/// Enhanced command configuration with structured output support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    // ... existing fields ...
    pub structured_output: bool,
    pub output_schema: Option<String>,
    pub automation_friendly: bool,
}

/// Structured output wrapper for automated processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredCommandOutput {
    pub command: String,
    pub execution_id: String,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub data: serde_json::Value,
    pub metadata: CommandMetadata,
}

/// Command metadata for context and debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandMetadata {
    pub duration: Duration,
    pub token_usage: Option<TokenUsage>,
    pub model_used: String,
    pub context_size: usize,
}

impl CommandRegistry {
    /// Execute command with structured output for automation
    pub async fn execute_structured(
        &self,
        command_name: &str,
        args: Vec<String>,
        context: &WorkflowContext,
    ) -> Result<StructuredCommandOutput> {
        let config = self.get_command(command_name)?;
        
        if !config.automation_friendly {
            return Err(Error::Config(format!(
                "Command '{}' is not configured for automation",
                command_name
            )));
        }
        
        // Execute command with enhanced context
        let start_time = Utc::now();
        let execution_id = Uuid::new_v4().to_string();
        
        let result = self.execute_command_internal(
            command_name,
            args,
            Some(context),
            config.structured_output,
        ).await;
        
        let duration = Utc::now().signed_duration_since(start_time);
        
        match result {
            Ok(output) => {
                let data = if config.structured_output {
                    self.parse_structured_output(&output, config.output_schema.as_deref())?
                } else {
                    serde_json::Value::String(output)
                };
                
                Ok(StructuredCommandOutput {
                    command: command_name.to_string(),
                    execution_id,
                    timestamp: start_time,
                    success: true,
                    data,
                    metadata: CommandMetadata {
                        duration: duration.to_std().unwrap_or_default(),
                        token_usage: None, // TODO: Extract from response
                        model_used: config.settings.model_override
                            .unwrap_or_else(|| "default".to_string()),
                        context_size: 0, // TODO: Calculate
                    },
                })
            }
            Err(e) => {
                Ok(StructuredCommandOutput {
                    command: command_name.to_string(),
                    execution_id,
                    timestamp: start_time,
                    success: false,
                    data: serde_json::json!({
                        "error": e.to_string(),
                        "error_type": "execution_failed"
                    }),
                    metadata: CommandMetadata {
                        duration: duration.to_std().unwrap_or_default(),
                        token_usage: None,
                        model_used: "unknown".to_string(),
                        context_size: 0,
                    },
                })
            }
        }
    }
}
```

### 5. Enhanced mmm-code-review Command

Update `.claude/commands/mmm-code-review.md` to include structured output:

```markdown
## Structured Output for Automation

**CRITICAL**: When invoked in automation mode, always end response with this exact JSON structure:

```json
{
  "mmm_structured_output": {
    "review_id": "review-{{ timestamp }}-{{ uuid }}",
    "timestamp": "{{ current_timestamp }}",
    "overall_score": {{ calculated_score }},
    "scope": "{{ analyzed_scope }}",
    "actions": [
      {
        "id": "action_{{ sequence }}",
        "type": "fix_error|improve_code|improve_performance|fix_style|add_tests|refactor",
        "severity": "critical|high|medium|low", 
        "file": "{{ file_path }}",
        "line": {{ line_number }},
        "line_range": [{{ start_line }}, {{ end_line }}],
        "title": "{{ brief_description }}",
        "description": "{{ detailed_explanation }}",
        "suggestion": "{{ specific_fix_recommendation }}",
        "automated": {{ true_if_automatable }},
        "estimated_effort": "{{ time_estimate }}",
        "category": "{{ issue_category }}",
        "impact": "{{ business_impact }}"
      }
    ],
    "summary": {
      "total_issues": {{ total_count }},
      "critical": {{ critical_count }},
      "high": {{ high_count }},
      "medium": {{ medium_count }},
      "low": {{ low_count }},
      "automated_fixes": {{ automatable_count }},
      "manual_fixes": {{ manual_count }},
      "compilation_errors": {{ error_count }},
      "test_failures": {{ test_failure_count }},
      "clippy_warnings": {{ warning_count }}
    },
    "metrics": {
      "code_complexity": {{ complexity_score }},
      "test_coverage": {{ coverage_percentage }},
      "technical_debt_ratio": {{ debt_ratio }},
      "maintainability_index": {{ maintainability_score }}
    },
    "recommendations": {
      "next_iteration_focus": "{{ focus_area }}",
      "architecture_improvements": ["{{ suggestion_1 }}", "{{ suggestion_2 }}"],
      "priority_actions": ["{{ action_id_1 }}", "{{ action_id_2 }}"]
    }
  }
}
```

**Automation Detection**: The command detects automation mode when:
- Invoked with `--format=json` parameter
- Environment variable `MMM_AUTOMATION=true` is set
- Called from within an MMM workflow context
```

### 6. New Workflow Step Commands

#### `src/loop/commands.rs`

```rust
/// Workflow step commands for iterative improvement
pub struct LoopCommands {
    engine: Arc<IterationEngine>,
    workflow_context: Arc<WorkflowContext>,
}

impl LoopCommands {
    /// Initialize a new loop session
    pub async fn session_init(&self, params: &WorkflowParameters) -> Result<SessionInitResult> {
        let config = LoopConfig {
            target_score: params.get_float("target_score")?,
            max_iterations: params.get_u32("max_iterations")?,
            scope: params.get_string("scope")?.split(',').map(|s| s.trim().to_string()).collect(),
            severity_filter: self.parse_severity_filter(params.get_string("severity_filter")?)?,
            termination_conditions: self.build_default_termination_conditions(),
            safety_settings: SafetySettings::default(),
            workflow_template: "code-quality-improvement".to_string(),
        };
        
        let session = self.engine.create_session(config).await?;
        
        Ok(SessionInitResult {
            session_id: session.id.to_string(),
            baseline_metrics: session.baseline_metrics.clone(),
            estimated_iterations: session.estimated_iterations,
        })
    }
    
    /// Execute Claude review phase
    pub async fn claude_review(&self, session_id: &str, iteration: u32) -> Result<ReviewResult> {
        let session = self.engine.get_session(session_id).await?;
        let scope = session.config.scope.join(" ");
        
        // Build context for Claude command
        let mut context = WorkflowContext::new();
        context.insert("iteration".to_string(), json!(iteration));
        context.insert("session_id".to_string(), json!(session_id));
        context.insert("previous_results".to_string(), json!(session.get_previous_results()));
        
        // Execute structured Claude command
        let output = self.engine.claude_manager
            .execute_structured("mmm-code-review", vec![scope], &context)
            .await?;
            
        if !output.success {
            return Err(Error::Command(format!("Code review failed: {:?}", output.data)));
        }
        
        // Parse structured output
        let review_data: ReviewData = serde_json::from_value(
            output.data.get("mmm_structured_output")
                .ok_or_else(|| Error::Parse("Missing structured output".to_string()))?
                .clone()
        )?;
        
        // Update session with review results
        self.engine.update_session_review(session_id, iteration, &review_data).await?;
        
        Ok(ReviewResult {
            review_id: review_data.review_id,
            quality_score: review_data.overall_score,
            actionable_items: review_data.actions.len(),
            automated_fixes: review_data.summary.automated_fixes,
            manual_fixes: review_data.summary.manual_fixes,
            critical_issues: review_data.summary.critical,
            recommendations: review_data.recommendations,
        })
    }
    
    /// Check termination conditions
    pub async fn check_termination_conditions(&self, session_id: &str) -> Result<TerminationResult> {
        let session = self.engine.get_session(session_id).await?;
        let current_metrics = session.get_current_metrics();
        
        for condition in &session.config.termination_conditions {
            if let Some(reason) = self.evaluate_condition(condition, &session, &current_metrics).await? {
                return Ok(TerminationResult {
                    should_terminate: true,
                    reason,
                    final_score: current_metrics.quality_score,
                    iterations_completed: session.current_iteration,
                });
            }
        }
        
        Ok(TerminationResult {
            should_terminate: false,
            reason: "Continue iteration".to_string(),
            final_score: current_metrics.quality_score,
            iterations_completed: session.current_iteration,
        })
    }
}
```

### 7. State Management Integration

#### Enhanced `src/state/manager.rs`

```rust
/// Enhanced state manager with loop session support
impl StateManager {
    /// Store iterative improvement session
    pub async fn store_loop_session(&self, session: &LoopSession) -> Result<()> {
        let session_json = serde_json::to_string(session)?;
        
        sqlx::query!(
            r#"
            INSERT INTO loop_sessions (
                id, project_id, config, status, current_iteration,
                baseline_metrics, current_metrics, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            session.id.to_string(),
            self.project_name,
            session_json,
            session.status.to_string(),
            session.current_iteration as i64,
            serde_json::to_string(&session.baseline_metrics)?,
            serde_json::to_string(&session.current_metrics)?,
            session.created_at,
            session.updated_at
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Load loop session by ID
    pub async fn load_loop_session(&self, session_id: &Uuid) -> Result<Option<LoopSession>> {
        let row = sqlx::query!(
            "SELECT * FROM loop_sessions WHERE id = ? AND project_id = ?",
            session_id.to_string(),
            self.project_name
        )
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(row) => {
                let session: LoopSession = serde_json::from_str(&row.config)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }
    
    /// List loop sessions with filtering
    pub async fn list_loop_sessions(
        &self,
        status_filter: Option<&str>,
        limit: u32,
    ) -> Result<Vec<LoopSessionSummary>> {
        let mut query = QueryBuilder::new(
            "SELECT id, status, current_iteration, created_at, updated_at FROM loop_sessions WHERE project_id = "
        );
        query.push_bind(&self.project_name);
        
        if let Some(status) = status_filter {
            query.push(" AND status = ");
            query.push_bind(status);
        }
        
        query.push(" ORDER BY created_at DESC LIMIT ");
        query.push_bind(limit as i64);
        
        let rows = query.build().fetch_all(&self.pool).await?;
        
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(LoopSessionSummary {
                id: Uuid::parse_str(&row.get::<String, _>("id"))?,
                status: row.get::<String, _>("status").parse()?,
                current_iteration: row.get::<i64, _>("current_iteration") as u32,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }
        
        Ok(sessions)
    }
}
```

### 8. Database Schema Migration

#### `migrations/20250126000000_loop_sessions.sql`

```sql
-- Loop sessions table
CREATE TABLE IF NOT EXISTS loop_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    config TEXT NOT NULL,
    status TEXT NOT NULL,
    current_iteration INTEGER NOT NULL DEFAULT 0,
    baseline_metrics TEXT,
    current_metrics TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,
    
    FOREIGN KEY (project_id) REFERENCES projects (name)
);

-- Loop iterations table (detailed results per iteration)
CREATE TABLE IF NOT EXISTS loop_iterations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    iteration_number INTEGER NOT NULL,
    review_results TEXT,
    improvement_results TEXT,
    validation_results TEXT,
    metrics TEXT,
    duration_seconds INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (session_id) REFERENCES loop_sessions (id),
    UNIQUE (session_id, iteration_number)
);

-- Loop actions table (individual improvements applied)
CREATE TABLE IF NOT EXISTS loop_actions (
    id TEXT PRIMARY KEY,
    iteration_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_number INTEGER,
    severity TEXT NOT NULL,
    description TEXT NOT NULL,
    applied BOOLEAN NOT NULL DEFAULT FALSE,
    success BOOLEAN,
    error_message TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (iteration_id) REFERENCES loop_iterations (id)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_loop_sessions_project ON loop_sessions (project_id);
CREATE INDEX IF NOT EXISTS idx_loop_sessions_status ON loop_sessions (status);
CREATE INDEX IF NOT EXISTS idx_loop_iterations_session ON loop_iterations (session_id);
CREATE INDEX IF NOT EXISTS idx_loop_actions_iteration ON loop_actions (iteration_id);
CREATE INDEX IF NOT EXISTS idx_loop_actions_file ON loop_actions (file_path);
```

### 9. Monitoring Integration

#### Enhanced `src/monitor/metrics.rs`

```rust
/// Loop-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopMetrics {
    pub session_id: Uuid,
    pub iteration: u32,
    pub quality_score: f64,
    pub score_improvement: f64,
    pub actions_applied: u32,
    pub actions_successful: u32,
    pub files_modified: u32,
    pub compilation_status: bool,
    pub test_status: bool,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
}

impl MetricsCollector {
    /// Record loop iteration metrics
    pub async fn record_loop_iteration(&self, metrics: &LoopMetrics) -> Result<()> {
        self.record_gauge(
            "loop_quality_score",
            metrics.quality_score,
            vec![
                ("session_id", metrics.session_id.to_string()),
                ("iteration", metrics.iteration.to_string()),
            ],
        ).await?;
        
        self.record_counter(
            "loop_actions_applied",
            metrics.actions_applied as f64,
            vec![("session_id", metrics.session_id.to_string())],
        ).await?;
        
        self.record_histogram(
            "loop_iteration_duration",
            metrics.duration.as_secs_f64(),
            vec![("session_id", metrics.session_id.to_string())],
        ).await?;
        
        Ok(())
    }
    
    /// Generate loop session report
    pub async fn generate_loop_report(&self, session_id: &Uuid) -> Result<LoopReport> {
        let metrics = self.query_loop_metrics(session_id).await?;
        
        Ok(LoopReport {
            session_id: *session_id,
            total_iterations: metrics.len() as u32,
            initial_score: metrics.first().map(|m| m.quality_score).unwrap_or(0.0),
            final_score: metrics.last().map(|m| m.quality_score).unwrap_or(0.0),
            total_improvement: metrics.last().map(|m| m.quality_score).unwrap_or(0.0) 
                - metrics.first().map(|m| m.quality_score).unwrap_or(0.0),
            total_actions: metrics.iter().map(|m| m.actions_applied).sum(),
            success_rate: {
                let total = metrics.iter().map(|m| m.actions_applied).sum::<u32>() as f64;
                let successful = metrics.iter().map(|m| m.actions_successful).sum::<u32>() as f64;
                if total > 0.0 { successful / total } else { 0.0 }
            },
            total_duration: metrics.iter().map(|m| m.duration).sum(),
            files_affected: self.count_unique_files_in_session(session_id).await?,
        })
    }
}
```

### 10. Configuration Templates

#### `templates/loop-configs/standard-quality.toml`

```toml
[general]
name = "standard-quality"
description = "Standard code quality improvement configuration"

[targets]
quality_score = 8.5
max_iterations = 3

[scope]
include = ["src/", "lib/", "tests/"]
exclude = ["target/", "node_modules/", ".git/"]

[severity]
levels = ["critical", "high", "medium"]
required_fixes = ["critical", "high"]

[termination]
conditions = [
    { type = "target_achieved", threshold = 8.5 },
    { type = "max_iterations", count = 3 },
    { type = "diminishing_returns", min_improvement = 0.1, consecutive = 2 },
    { type = "no_automated_actions" },
    { type = "quality_regression", threshold = 0.5 },
]

[safety]
create_git_stash = true
validate_compilation = true
run_tests = true
max_file_changes_per_iteration = 20
rollback_on_regression = true

[workflow]
template = "code-quality-improvement"
checkpoint_on_issues = true
human_review_threshold = 5.0

[reporting]
generate_detailed_report = true
include_metrics = true
export_formats = ["json", "markdown"]
```

## Implementation Plan

### Phase 1: Core Infrastructure (Week 1)
1. Create `src/loop/mod.rs` module with basic structures
2. Add database migrations for loop session storage
3. Enhance CLI with `mmm loop` subcommands
4. Create basic workflow template

### Phase 2: Claude Integration (Week 2)
1. Enhance Claude command registry for structured output
2. Modify `mmm-code-review` command for automation
3. Implement workflow step commands
4. Add termination condition evaluation

### Phase 3: Engine Implementation (Week 3)
1. Implement `IterationEngine` core logic
2. Add session management and state persistence
3. Integrate with existing workflow system
4. Add safety mechanisms and git integration

### Phase 4: Monitoring & Reporting (Week 4)
1. Integrate with existing monitoring system
2. Add loop-specific metrics and alerts
3. Implement comprehensive reporting
4. Add configuration templates

### Phase 5: Testing & Documentation (Week 5)
1. Unit and integration tests
2. End-to-end workflow testing
3. Documentation and examples
4. Performance optimization

## Success Metrics

### Functional Requirements
- [ ] Complete workflow execution without manual intervention
- [ ] Proper termination condition handling
- [ ] State persistence across MMM restarts
- [ ] Git safety mechanisms prevent data loss
- [ ] Integration with existing MMM features

### Performance Requirements
- [ ] Session startup time < 10 seconds
- [ ] Iteration cycle time < 5 minutes for typical projects
- [ ] Memory usage stays within existing MMM bounds
- [ ] Database queries remain performant with session history

### Quality Requirements
- [ ] 90%+ automated action success rate
- [ ] Accurate convergence detection
- [ ] Comprehensive error handling and recovery
- [ ] Clear user feedback and progress indication

## Dependencies

### Internal MMM Modules
- `workflow::WorkflowEngine` - Core orchestration
- `claude::ClaudeManager` - Command execution
- `state::StateManager` - Session persistence
- `monitor::MetricsCollector` - Performance tracking
- `project::ProjectManager` - Project context

### External Dependencies
- No new external dependencies required
- Leverages existing MMM infrastructure

## Risks and Mitigations

### Risk: Claude CLI Rate Limiting
**Mitigation**: Built-in retry logic with exponential backoff, session pause/resume capability

### Risk: Long-running Sessions
**Mitigation**: Checkpoint system allows interruption and resumption, timeout protections

### Risk: Code Quality Regression
**Mitigation**: Git stashing, compilation validation, test execution, automatic rollback

### Risk: Session State Corruption
**Mitigation**: Atomic database transactions, session validation, recovery procedures

This specification provides a comprehensive integration of iterative improvement loops directly into MMM's existing architecture, leveraging its robust workflow, state management, and monitoring systems while maintaining the project's architectural principles.