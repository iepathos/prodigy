# Feature: Core Architecture

## Objective
Establish a robust, extensible architecture for mmm that supports managing multiple projects, complex workflows, and scalable Claude integrations while maintaining simplicity for basic use cases.

## Acceptance Criteria
- [ ] Support for multiple projects in a single mmm instance
- [ ] Project isolation with separate configurations and state
- [ ] Global and project-specific configuration hierarchy
- [ ] Efficient state management with persistence and recovery
- [ ] Command routing system for extensibility
- [ ] Error handling with graceful degradation
- [ ] Async/concurrent spec processing support
- [ ] Database backend for complex state (SQLite)

## Technical Details

### Project Structure
```
~/.mmm/                        # Global mmm directory
├── config.toml               # Global configuration
├── projects/                 # Project registry
│   └── project-name.toml    # Project metadata
└── templates/               # Global spec templates

project-dir/
├── .mmm/                    # Project-specific mmm directory
│   ├── config.toml         # Project configuration
│   ├── state.db            # SQLite state database
│   └── logs/               # Execution logs
├── specs/                   # Specification files
└── mmm.toml                # Project manifest
```

### Core Components

1. **Project Manager**
   - Project registration and discovery
   - Project context switching
   - Project templates and scaffolding

2. **Specification Engine**
   - Spec parsing with frontmatter support
   - Spec dependencies and ordering
   - Spec templates with variables
   - Dynamic spec generation

3. **State Manager**
   - SQLite backend for complex queries
   - State versioning and history
   - Checkpoint and rollback support
   - State export/import

4. **Configuration System**
   - TOML-based configuration
   - Environment variable overrides
   - Configuration validation
   - Hot-reload support

5. **Command Dispatcher**
   - Plugin-based command system
   - Command aliases and shortcuts
   - Command history and replay
   - Batch command execution

### Database Schema
```sql
-- Projects table
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    path TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Specifications table
CREATE TABLE specifications (
    id INTEGER PRIMARY KEY,
    project_id INTEGER,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    dependencies TEXT, -- JSON array
    metadata TEXT, -- JSON object
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Execution history
CREATE TABLE executions (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER,
    iteration INTEGER,
    command TEXT,
    input TEXT,
    output TEXT,
    status TEXT,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    FOREIGN KEY (spec_id) REFERENCES specifications(id)
);

-- State snapshots
CREATE TABLE state_snapshots (
    id INTEGER PRIMARY KEY,
    project_id INTEGER,
    snapshot_data TEXT, -- JSON
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
```

### Error Handling Strategy
- Result<T, Error> for all fallible operations
- Custom error types with context
- Error recovery mechanisms
- User-friendly error messages
- Debug mode with detailed traces

### Performance Considerations
- Lazy loading of specifications
- Concurrent spec processing where possible
- Efficient state queries with indexes
- Minimal memory footprint
- Progress indication for long operations