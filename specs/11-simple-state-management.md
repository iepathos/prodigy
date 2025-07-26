# Spec 11: Simple State Management

## Objective

Replace complex SQLite database with dead simple JSON-based state management that just works. State should be human-readable, git-friendly, and require zero configuration.

## Design Principles

1. **Simple**: Just JSON files on disk
2. **Transparent**: Human-readable and debuggable
3. **Git-Friendly**: Mergeable and diffable
4. **Recoverable**: Corruption-resistant with backups
5. **Fast**: Cached in memory during execution

## State Structure

### Directory Layout

```
.mmm/
├── state.json           # Current session state
├── history/            # Historical runs
│   └── 2024-01-15/
│       ├── 001-improve.json
│       ├── 002-improve.json
│       └── summary.json
├── cache/              # Temporary caches
│   ├── analysis.json   # Project analysis cache
│   └── context.json    # Context cache
└── learning.json       # Learned patterns and preferences
```

### Core State File

```json
// .mmm/state.json
{
  "version": "1.0",
  "project_id": "uuid-here",
  "last_run": "2024-01-15T10:30:00Z",
  "current_score": 7.8,
  "sessions": {
    "active": null,
    "last_completed": "session-uuid"
  },
  "stats": {
    "total_runs": 42,
    "total_improvements": 156,
    "average_improvement": 0.3,
    "favorite_improvements": ["error_handling", "documentation"]
  }
}
```

### Session State

```json
// .mmm/history/2024-01-15/001-improve.json
{
  "session_id": "uuid-here",
  "started_at": "2024-01-15T10:30:00Z",
  "completed_at": "2024-01-15T10:35:00Z",
  "initial_score": 7.5,
  "final_score": 7.8,
  "improvements": [
    {
      "type": "error_handling",
      "file": "src/main.rs",
      "line": 42,
      "description": "Replaced unwrap with proper error handling",
      "impact": 0.1
    }
  ],
  "files_changed": ["src/main.rs", "src/lib.rs"],
  "metrics": {
    "duration_seconds": 300,
    "claude_calls": 2,
    "tokens_used": 4500
  }
}
```

### Learning State

```json
// .mmm/learning.json
{
  "patterns": {
    "successful_improvements": {
      "error_handling": {
        "success_rate": 0.92,
        "average_impact": 0.15,
        "examples": ["unwrap_to_result", "error_propagation"]
      },
      "documentation": {
        "success_rate": 0.88,
        "average_impact": 0.10,
        "examples": ["add_doc_comments", "examples_in_docs"]
      }
    },
    "failed_patterns": {
      "over_abstraction": {
        "failure_rate": 0.75,
        "avoid": true
      }
    }
  },
  "preferences": {
    "focus_areas": ["error_handling", "tests"],
    "skip_patterns": ["generated_files", "vendor/"],
    "style_hints": {
      "error_style": "result_based",
      "test_style": "descriptive_names"
    }
  }
}
```

## Implementation

### State Manager

```rust
use std::path::{Path, PathBuf};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

pub struct StateManager {
    root: PathBuf,
    state: State,
}

#[derive(Serialize, Deserialize, Default)]
struct State {
    version: String,
    project_id: String,
    last_run: Option<DateTime<Utc>>,
    current_score: f32,
    sessions: SessionInfo,
    stats: Statistics,
}

impl StateManager {
    pub fn new() -> Result<Self> {
        let root = PathBuf::from(".mmm");
        
        // Create directory if needed
        fs::create_dir_all(&root)?;
        
        // Load or create state
        let state = Self::load_or_create(&root)?;
        
        Ok(Self { root, state })
    }
    
    pub fn save(&self) -> Result<()> {
        // Save with atomic write
        let temp_file = self.root.join("state.json.tmp");
        let final_file = self.root.join("state.json");
        
        // Write to temp file
        let json = serde_json::to_string_pretty(&self.state)?;
        fs::write(&temp_file, json)?;
        
        // Atomic rename
        fs::rename(temp_file, final_file)?;
        
        Ok(())
    }
    
    pub fn record_session(&mut self, session: SessionRecord) -> Result<()> {
        // Create date directory
        let date_dir = self.root
            .join("history")
            .join(Utc::today().format("%Y-%m-%d").to_string());
        fs::create_dir_all(&date_dir)?;
        
        // Find next session number
        let session_num = self.next_session_number(&date_dir)?;
        let filename = format!("{:03}-improve.json", session_num);
        
        // Save session
        let session_file = date_dir.join(filename);
        let json = serde_json::to_string_pretty(&session)?;
        fs::write(session_file, json)?;
        
        // Update main state
        self.state.last_run = Some(session.completed_at);
        self.state.current_score = session.final_score;
        self.state.stats.total_runs += 1;
        self.state.stats.total_improvements += session.improvements.len() as u32;
        
        self.save()?;
        Ok(())
    }
}
```

### Cache Manager

```rust
pub struct CacheManager {
    root: PathBuf,
}

impl CacheManager {
    pub fn new() -> Result<Self> {
        let root = PathBuf::from(".mmm/cache");
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }
    
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let cache_file = self.root.join(format!("{}.json", key));
        
        if !cache_file.exists() {
            return Ok(None);
        }
        
        // Check age (cache for 1 hour)
        let metadata = fs::metadata(&cache_file)?;
        let age = SystemTime::now().duration_since(metadata.modified()?)?;
        
        if age > Duration::from_secs(3600) {
            fs::remove_file(&cache_file)?;
            return Ok(None);
        }
        
        let contents = fs::read_to_string(&cache_file)?;
        let value = serde_json::from_str(&contents)?;
        Ok(Some(value))
    }
    
    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let cache_file = self.root.join(format!("{}.json", key));
        let json = serde_json::to_string_pretty(value)?;
        fs::write(cache_file, json)?;
        Ok(())
    }
    
    pub fn clear(&self) -> Result<()> {
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.path().extension() == Some("json".as_ref()) {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
}
```

### Learning Manager

```rust
pub struct LearningManager {
    learning: Learning,
    path: PathBuf,
}

#[derive(Serialize, Deserialize, Default)]
struct Learning {
    patterns: PatternStats,
    preferences: Preferences,
}

impl LearningManager {
    pub fn load() -> Result<Self> {
        let path = PathBuf::from(".mmm/learning.json");
        let learning = if path.exists() {
            let contents = fs::read_to_string(&path)?;
            serde_json::from_str(&contents)?
        } else {
            Learning::default()
        };
        
        Ok(Self { learning, path })
    }
    
    pub fn record_improvement(&mut self, improvement: &Improvement) -> Result<()> {
        // Update pattern statistics
        let pattern = self.learning.patterns
            .successful_improvements
            .entry(improvement.improvement_type.clone())
            .or_default();
            
        pattern.total_attempts += 1;
        pattern.successful += 1;
        pattern.success_rate = pattern.successful as f32 / pattern.total_attempts as f32;
        pattern.impacts.push(improvement.impact);
        pattern.average_impact = pattern.impacts.iter().sum::<f32>() / pattern.impacts.len() as f32;
        
        self.save()?;
        Ok(())
    }
    
    pub fn suggest_improvements(&self, project: &ProjectInfo) -> Vec<String> {
        // Return improvements sorted by success rate * impact
        let mut suggestions: Vec<_> = self.learning.patterns
            .successful_improvements
            .iter()
            .map(|(name, stats)| (name, stats.success_rate * stats.average_impact))
            .collect();
            
        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        
        suggestions.into_iter()
            .take(5)
            .map(|(name, _)| name.clone())
            .collect()
    }
}
```

## Migration Support

### Import from Old System

```rust
pub fn migrate_from_sqlite(db_path: &Path) -> Result<()> {
    // Read old SQLite data
    let conn = sqlite::open(db_path)?;
    
    // Convert to new format
    let state = convert_database_to_state(&conn)?;
    let sessions = convert_sessions(&conn)?;
    
    // Save in new format
    let state_manager = StateManager::new()?;
    state_manager.import_legacy(state, sessions)?;
    
    println!("✅ Migrated {} sessions to new format", sessions.len());
    Ok(())
}
```

## Benefits

1. **Simplicity**: No database dependencies
2. **Transparency**: Can inspect/edit state with text editor
3. **Git-Friendly**: Track improvement history in git
4. **Debugging**: Easy to see what MMM is doing
5. **Recovery**: Just delete .mmm to reset

## Error Handling

```rust
impl StateManager {
    fn load_or_create(root: &Path) -> Result<State> {
        let state_file = root.join("state.json");
        
        if state_file.exists() {
            // Try to load, with fallback
            match fs::read_to_string(&state_file) {
                Ok(contents) => {
                    match serde_json::from_str(&contents) {
                        Ok(state) => Ok(state),
                        Err(e) => {
                            // Backup corrupted file
                            let backup = root.join("state.json.corrupted");
                            fs::rename(&state_file, backup)?;
                            eprintln!("⚠️  State file corrupted, backed up to {:?}", backup);
                            Ok(State::default())
                        }
                    }
                },
                Err(e) => {
                    eprintln!("⚠️  Cannot read state file: {}", e);
                    Ok(State::default())
                }
            }
        } else {
            Ok(State::default())
        }
    }
}
```

## Example Usage

```rust
// In improve command
let mut state = StateManager::new()?;
let cache = CacheManager::new()?;
let mut learning = LearningManager::load()?;

// Check cache
if let Some(analysis) = cache.get::<ProjectAnalysis>("analysis")? {
    println!("Using cached analysis");
} else {
    let analysis = analyze_project()?;
    cache.set("analysis", &analysis)?;
}

// Run improvements
let session = run_improvements()?;

// Record results
state.record_session(session.clone())?;

// Update learning
for improvement in &session.improvements {
    learning.record_improvement(improvement)?;
}

// Get suggestions for next time
let suggestions = learning.suggest_improvements(&project_info);
println!("Next time try focusing on: {:?}", suggestions);
```