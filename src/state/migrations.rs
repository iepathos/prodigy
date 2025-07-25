pub const INITIAL_MIGRATION: &str = r#"
-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT UNIQUE NOT NULL,
    path TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Specifications table
CREATE TABLE IF NOT EXISTS specifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    status TEXT DEFAULT 'pending',
    dependencies TEXT, -- JSON array
    metadata TEXT, -- JSON object
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    UNIQUE(project_id, name)
);

-- Execution history
CREATE TABLE IF NOT EXISTS executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    spec_id INTEGER NOT NULL,
    iteration INTEGER NOT NULL,
    command TEXT NOT NULL,
    input TEXT NOT NULL,
    output TEXT NOT NULL,
    status TEXT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    completed_at TIMESTAMP,
    FOREIGN KEY (spec_id) REFERENCES specifications(id)
);

-- State snapshots
CREATE TABLE IF NOT EXISTS state_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    snapshot_data TEXT NOT NULL, -- JSON
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_specifications_project_id ON specifications(project_id);
CREATE INDEX IF NOT EXISTS idx_specifications_status ON specifications(status);
CREATE INDEX IF NOT EXISTS idx_executions_spec_id ON executions(spec_id);
CREATE INDEX IF NOT EXISTS idx_executions_started_at ON executions(started_at);
CREATE INDEX IF NOT EXISTS idx_state_snapshots_project_id ON state_snapshots(project_id);
CREATE INDEX IF NOT EXISTS idx_state_snapshots_created_at ON state_snapshots(created_at);
"#;

pub const WORKFLOW_MIGRATION: &str = r#"
-- Workflow executions table
CREATE TABLE IF NOT EXISTS workflow_executions (
    workflow_id TEXT PRIMARY KEY,
    workflow_name TEXT NOT NULL,
    spec_id TEXT,
    status TEXT NOT NULL,
    state_json TEXT NOT NULL,
    started_at TIMESTAMP NOT NULL,
    completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Workflow checkpoints table
CREATE TABLE IF NOT EXISTS workflow_checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    description TEXT NOT NULL,
    state_json TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    FOREIGN KEY (workflow_id) REFERENCES workflow_executions(workflow_id)
);

-- Create indexes for workflows
CREATE INDEX IF NOT EXISTS idx_workflow_executions_name ON workflow_executions(workflow_name);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_status ON workflow_executions(status);
CREATE INDEX IF NOT EXISTS idx_workflow_executions_started_at ON workflow_executions(started_at);
CREATE INDEX IF NOT EXISTS idx_workflow_checkpoints_workflow_id ON workflow_checkpoints(workflow_id);
"#;
