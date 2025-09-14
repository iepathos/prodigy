---
number: 69
title: Git Commit Tracking and Verification
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 69: Git Commit Tracking and Verification

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The whitepaper shows `commit_required: true` as a flag for MapReduce operations:
```yaml
agent_template:
  commands:
    - claude: "/fix-debt-item --json '${item}'"
      commit_required: true
```

This indicates that certain operations should create git commits, and the workflow should verify that commits were actually made. Currently, while Prodigy uses git worktrees, there's no explicit tracking or verification of commit creation.

## Objective

Implement git commit tracking and verification to ensure that operations requiring commits actually produce them, enabling audit trails, rollback capabilities, and verification of AI-generated changes.

## Requirements

### Functional Requirements
- Support `commit_required: true` flag on workflow steps
- Detect and track git commits created during step execution
- Fail steps that require commits but don't produce them
- Capture commit metadata (hash, message, files changed)
- Support commit message templates and validation
- Enable automatic commit creation with generated messages
- Track commits per agent in MapReduce operations
- Support commit squashing and cleanup options
- Integration with worktree merge strategies

### Non-Functional Requirements
- Minimal git operation overhead
- Clear commit attribution
- Atomic commit operations
- Detailed commit tracking logs

## Acceptance Criteria

- [ ] `commit_required: true` enforces commit creation
- [ ] Step fails if no commit created when required
- [ ] Commit metadata captured and available as variables
- [ ] `${step.commits}` contains list of commit hashes
- [ ] `${step.files_changed}` contains modified files
- [ ] Auto-commit with generated messages when configured
- [ ] MapReduce tracks commits per agent
- [ ] Commit verification in validation phase
- [ ] Clear error when commit required but not created
- [ ] Commit history preserved through worktree merges

## Technical Details

### Implementation Approach

1. **Enhanced Step Configuration**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowStep {
       #[serde(flatten)]
       pub command: CommandType,

       /// Whether this step must create a git commit
       #[serde(default)]
       pub commit_required: bool,

       /// Commit configuration
       #[serde(skip_serializing_if = "Option::is_none")]
       pub commit_config: Option<CommitConfig>,

       /// Auto-commit if changes detected
       #[serde(default)]
       pub auto_commit: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CommitConfig {
       /// Commit message template
       pub message_template: Option<String>,

       /// Commit message validation regex
       pub message_pattern: Option<String>,

       /// Whether to sign commits
       #[serde(default)]
       pub sign: bool,

       /// Author override
       pub author: Option<String>,

       /// Files to include (glob patterns)
       pub include_files: Option<Vec<String>>,

       /// Files to exclude (glob patterns)
       pub exclude_files: Option<Vec<String>>,

       /// Squash commits at end of workflow
       #[serde(default)]
       pub squash: bool,
   }
   ```

2. **Git Commit Tracker**:
   ```rust
   pub struct CommitTracker {
       repo: Repository,
       initial_head: Oid,
       tracked_commits: Arc<RwLock<Vec<TrackedCommit>>>,
   }

   #[derive(Debug, Clone, Serialize)]
   pub struct TrackedCommit {
       pub hash: String,
       pub message: String,
       pub author: String,
       pub timestamp: DateTime<Utc>,
       pub files_changed: Vec<PathBuf>,
       pub insertions: usize,
       pub deletions: usize,
       pub step_name: String,
       pub agent_id: Option<String>,
   }

   impl CommitTracker {
       pub async fn track_step_execution<F, T>(
           &self,
           step: &WorkflowStep,
           execution: F,
       ) -> Result<(T, Vec<TrackedCommit>)>
       where
           F: Future<Output = Result<T>>,
       {
           // Record current HEAD
           let before_head = self.repo.head()?.target()
               .ok_or_else(|| anyhow!("No HEAD"))?;

           // Execute the step
           let result = execution.await?;

           // Check for new commits
           let after_head = self.repo.head()?.target()
               .ok_or_else(|| anyhow!("No HEAD"))?;

           let new_commits = if before_head != after_head {
               self.get_commits_between(before_head, after_head).await?
           } else if step.auto_commit {
               // Check for uncommitted changes
               if self.has_changes()? {
                   vec![self.create_auto_commit(step).await?]
               } else {
                   vec![]
               }
           } else {
               vec![]
           };

           // Verify commit requirement
           if step.commit_required && new_commits.is_empty() {
               return Err(anyhow!(
                   "Step '{}' requires a commit but none was created",
                   step.name()
               ));
           }

           // Track commits
           for commit in &new_commits {
               self.tracked_commits.write().await.push(commit.clone());
           }

           Ok((result, new_commits))
       }

       async fn create_auto_commit(&self, step: &WorkflowStep) -> Result<TrackedCommit> {
           // Get changes
           let statuses = self.repo.statuses(None)?;
           let mut files_changed = Vec::new();

           for entry in statuses.iter() {
               if let Some(path) = entry.path() {
                   files_changed.push(PathBuf::from(path));
               }
           }

           // Generate commit message
           let message = if let Some(template) = &step.commit_config
               .as_ref()
               .and_then(|c| c.message_template.as_ref())
           {
               self.interpolate_template(template, step)?
           } else {
               format!("Auto-commit: {}", step.name())
           };

           // Stage changes
           let mut index = self.repo.index()?;
           for file in &files_changed {
               index.add_path(file)?;
           }
           index.write()?;

           // Create commit
           let tree_id = index.write_tree()?;
           let tree = self.repo.find_tree(tree_id)?;
           let parent = self.repo.head()?.peel_to_commit()?;
           let signature = self.repo.signature()?;

           let commit_id = self.repo.commit(
               Some("HEAD"),
               &signature,
               &signature,
               &message,
               &tree,
               &[&parent],
           )?;

           Ok(TrackedCommit {
               hash: commit_id.to_string(),
               message,
               author: signature.name().unwrap_or("Unknown").to_string(),
               timestamp: Utc::now(),
               files_changed,
               insertions: 0, // Would need diff analysis
               deletions: 0,
               step_name: step.name(),
               agent_id: None,
           })
       }

       pub fn has_changes(&self) -> Result<bool> {
           let statuses = self.repo.statuses(Some(
               StatusOptions::new()
                   .include_untracked(true)
                   .include_ignored(false)
           ))?;
           Ok(!statuses.is_empty())
       }
   }
   ```

3. **Integration with MapReduce**:
   ```rust
   impl MapReduceExecutor {
       async fn execute_agent_with_commit_tracking(
           &self,
           agent_id: &str,
           work_item: &Value,
           worktree_path: &Path,
       ) -> Result<AgentResult> {
           let tracker = CommitTracker::new(worktree_path)?;

           // Execute agent commands
           let mut commits = Vec::new();
           for step in &self.agent_template {
               let (result, step_commits) = tracker
                   .track_step_execution(step, async {
                       self.execute_step(step, work_item).await
                   })
                   .await?;

               commits.extend(step_commits);
           }

           // Include commit info in agent result
           Ok(AgentResult {
               item_id: agent_id.to_string(),
               commits: commits.iter().map(|c| c.hash.clone()).collect(),
               files_modified: commits
                   .iter()
                   .flat_map(|c| c.files_changed.clone())
                   .collect(),
               ..Default::default()
           })
       }
   }
   ```

### Architecture Changes
- Add `CommitTracker` to execution context
- Enhance `AgentResult` with commit information
- Integrate with worktree management
- Add commit metrics collection
- Update merge strategies for commit preservation

### Data Structures
```yaml
# Example with commit tracking
tasks:
  - name: "Refactor module"
    claude: "/refactor user.py"
    commit_required: true
    commit_config:
      message_template: "refactor: improve ${file} structure"
      sign: true

  - name: "Auto-fix issues"
    claude: "/fix-all-issues"
    auto_commit: true
    commit_config:
      message_template: "fix: resolve issues in ${step.name}"

# MapReduce with commit tracking
map:
  agent_template:
    commands:
      - claude: "/modernize ${item}"
        commit_required: true
        commit_config:
          message_template: "feat: modernize ${item.name}"
      - validate: "npm test ${item}"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/worktree/manager.rs` - Git operations
  - `src/cook/execution/` - Commit tracking integration
  - `src/config/workflow.rs` - Commit configuration
- **External Dependencies**: `git2` for git operations

## Testing Strategy

- **Unit Tests**:
  - Commit detection logic
  - Auto-commit creation
  - Message template interpolation
  - Change detection
- **Integration Tests**:
  - End-to-end commit tracking
  - Commit requirement enforcement
  - MapReduce commit aggregation
  - Worktree merge with commits
- **Scenario Tests**:
  - Multi-step workflows with commits
  - Rollback scenarios
  - Commit squashing
  - Parallel agent commits

## Documentation Requirements

- **Code Documentation**: Document commit tracking flow
- **User Documentation**:
  - Commit tracking guide
  - Auto-commit configuration
  - Message template syntax
  - Best practices for commit management
- **Architecture Updates**: Add commit flow to git integration

## Implementation Notes

- Use libgit2 for efficient git operations
- Consider commit signing for security
- Support commit hooks integration
- Enable commit message linting
- Future: Integration with PR creation

## Migration and Compatibility

- Workflows without commit_required work unchanged
- No breaking changes to existing workflows
- Gradual adoption of commit tracking
- Clear examples for common scenarios