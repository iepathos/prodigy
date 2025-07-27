# Centralized Worktree State Management

## Problem

Currently, MMM has several limitations in worktree management:
1. Focus directive is embedded in directory names, causing issues with long prompts
2. No tracking of worktree-specific state (iterations completed, status)
3. No persistence of worktree metadata after cleanup

## Solution

Implement centralized state tracking in the main repository's `.mmm/worktrees/` directory, storing metadata separately from the actual worktrees.

## Directory Structure

```
# Main repository
.mmm/
├── state.json                    # Global state
└── worktrees/                    # Worktree metadata (NOT the worktrees themselves)
    ├── session-1737890123.json   # State for worktree session-1737890123
    └── session-1737890456.json   # State for worktree session-1737890456

# Home directory (actual worktrees)
~/.mmm/worktrees/
└── myrepo/
    ├── session-1737890123/       # Actual git worktree
    └── session-1737890456/       # Actual git worktree
```

## Worktree Session State Schema

```json
{
  "session_id": "session-1737890123",
  "worktree_name": "session-1737890123",
  "branch": "mmm-session-1737890123",
  "created_at": "2024-01-26T10:30:00Z",
  "updated_at": "2024-01-26T11:45:00Z",
  "status": "in_progress",  // in_progress, completed, failed, abandoned
  "focus": "Improve error handling and add retry logic",
  "iterations": {
    "completed": 3,
    "max": 10
  },
  "stats": {
    "files_changed": 12,
    "commits": 9,
    "last_commit_sha": "abc123def"
  },
  "merged": false,
  "merged_at": null,
  "error": null
}
```

## Implementation Changes

### 1. Update WorktreeManager

```rust
// src/worktree/manager.rs
impl WorktreeManager {
    pub fn create_session(&self, focus: Option<&str>) -> Result<WorktreeSession> {
        let timestamp = Utc::now().timestamp();
        // Simple name without focus
        let name = format!("session-{timestamp}");
        let branch = format!("mmm-{}", name);
        let worktree_path = self.base_dir.join(&name);
        
        // Create worktree
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["worktree", "add", "-b", &branch])
            .arg(&worktree_path)
            .output()?;
            
        // Create session state
        let session = WorktreeSession::new(name.clone(), branch, worktree_path, focus.map(String::from));
        self.save_session_state(&session)?;
        
        Ok(session)
    }
    
    fn save_session_state(&self, session: &WorktreeSession) -> Result<()> {
        let state_dir = self.repo_path.join(".mmm").join("worktrees");
        fs::create_dir_all(&state_dir)?;
        
        let state_file = state_dir.join(format!("{}.json", session.name));
        let state = WorktreeState {
            session_id: session.name.clone(),
            worktree_name: session.name.clone(),
            branch: session.branch.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            status: WorktreeStatus::InProgress,
            focus: session.focus.clone(),
            iterations: IterationInfo { completed: 0, max: 10 },
            stats: WorktreeStats::default(),
            merged: false,
            merged_at: None,
            error: None,
        };
        
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(state_file, json)?;
        Ok(())
    }
    
    pub fn update_session_state<F>(&self, name: &str, updater: F) -> Result<()> 
    where
        F: FnOnce(&mut WorktreeState)
    {
        let state_file = self.repo_path.join(".mmm").join("worktrees").join(format!("{}.json", name));
        let mut state: WorktreeState = serde_json::from_str(&fs::read_to_string(&state_file)?)?;
        
        updater(&mut state);
        state.updated_at = Utc::now();
        
        let json = serde_json::to_string_pretty(&state)?;
        fs::write(state_file, json)?;
        Ok(())
    }
}
```

### 2. New Types for Worktree State

```rust
// src/worktree/state.rs
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorktreeState {
    pub session_id: String,
    pub worktree_name: String,
    pub branch: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub status: WorktreeStatus,
    pub focus: Option<String>,
    pub iterations: IterationInfo,
    pub stats: WorktreeStats,
    pub merged: bool,
    pub merged_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum WorktreeStatus {
    InProgress,
    Completed,
    Failed,
    Abandoned,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IterationInfo {
    pub completed: u32,
    pub max: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct WorktreeStats {
    pub files_changed: u32,
    pub commits: u32,
    pub last_commit_sha: Option<String>,
}
```

### 3. Update Improve Command

```rust
// src/improve/mod.rs
async fn run_in_worktree(
    cmd: command::ImproveCommand,
    session: crate::worktree::WorktreeSession,
) -> Result<()> {
    let worktree_manager = WorktreeManager::new(/* get original repo path */)?;
    
    // Run improvement loop
    let result = run_improvement_loop(cmd, &session, &worktree_manager).await;
    
    // Update final state
    worktree_manager.update_session_state(&session.name, |state| {
        match &result {
            Ok(_) => {
                state.status = WorktreeStatus::Completed;
            }
            Err(e) => {
                state.status = WorktreeStatus::Failed;
                state.error = Some(e.to_string());
            }
        }
    })?;
    
    result
}

async fn run_improvement_loop(
    cmd: command::ImproveCommand,
    session: &WorktreeSession,
    worktree_manager: &WorktreeManager,
) -> Result<()> {
    let mut iteration = 1;
    
    while iteration <= cmd.max_iterations {
        // ... existing improvement logic ...
        
        // Update state after each iteration
        worktree_manager.update_session_state(&session.name, |state| {
            state.iterations.completed = iteration;
            state.iterations.max = cmd.max_iterations;
            state.stats.files_changed += files_changed_this_iteration;
            state.stats.commits += 1;
            // Get last commit SHA if needed
        })?;
        
        iteration += 1;
    }
    
    Ok(())
}
```

### 4. Update List Command

```rust
// src/main.rs
WorktreeCommands::List => {
    let sessions = worktree_manager.list_sessions()?;
    
    // Load state for each session
    for session in sessions {
        let state_file = Path::new(".mmm/worktrees").join(format!("{}.json", session.name));
        if let Ok(state_json) = fs::read_to_string(&state_file) {
            if let Ok(state) = serde_json::from_str::<WorktreeState>(&state_json) {
                println!(
                    "  {} - {} - {} ({}/{})",
                    session.name,
                    state.status,
                    state.focus.as_deref().unwrap_or("no focus"),
                    state.iterations.completed,
                    state.iterations.max
                );
            }
        } else {
            // Fallback to current display
            println!("  {} - {}", session.name, session.path.display());
        }
    }
}
```

## Migration

For existing worktrees without state files:
1. Create state files with default values
2. Mark status as "abandoned" if worktree is old
3. Attempt to extract focus from worktree name for legacy sessions

## Benefits

1. **Clean worktree names**: Just `session-{timestamp}`, no embedded focus
2. **Rich state tracking**: Iterations, status, stats all preserved
3. **Survives cleanup**: State persists even after worktree removal
4. **Better UX**: Can show progress, filter by status, etc.
5. **No merge conflicts**: State files never enter worktrees

## Testing

1. Create worktree with long focus - verify clean name
2. Run iterations - verify state updates
3. List worktrees - verify state display
4. Clean up worktree - verify state persists
5. Handle concurrent updates to state files