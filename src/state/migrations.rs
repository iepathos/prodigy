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

pub const MONITORING_MIGRATION: &str = r#"
-- Metrics table
CREATE TABLE IF NOT EXISTS metrics (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    value_type TEXT NOT NULL,
    value_json TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    labels_json TEXT NOT NULL,
    project_id TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_metrics_name ON metrics(name);
CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON metrics(timestamp);
CREATE INDEX IF NOT EXISTS idx_metrics_project ON metrics(project_id);
CREATE INDEX IF NOT EXISTS idx_metrics_name_timestamp ON metrics(name, timestamp);

-- Alerts table
CREATE TABLE IF NOT EXISTS alerts (
    id TEXT PRIMARY KEY,
    rule_name TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    acknowledged BOOLEAN NOT NULL DEFAULT FALSE,
    acknowledged_at TEXT,
    metadata_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_alerts_timestamp ON alerts(timestamp);
CREATE INDEX IF NOT EXISTS idx_alerts_severity ON alerts(severity);
CREATE INDEX IF NOT EXISTS idx_alerts_acknowledged ON alerts(acknowledged);

-- Performance traces table
CREATE TABLE IF NOT EXISTS traces (
    id TEXT PRIMARY KEY,
    operation TEXT NOT NULL,
    start_timestamp TEXT NOT NULL,
    end_timestamp TEXT,
    duration_ms INTEGER,
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_traces_operation ON traces(operation);
CREATE INDEX IF NOT EXISTS idx_traces_timestamp ON traces(start_timestamp);
CREATE INDEX IF NOT EXISTS idx_traces_duration ON traces(duration_ms);

-- Performance spans table
CREATE TABLE IF NOT EXISTS spans (
    id TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    start_timestamp TEXT NOT NULL,
    end_timestamp TEXT,
    duration_ms INTEGER,
    tags_json TEXT NOT NULL,
    FOREIGN KEY (trace_id) REFERENCES traces(id)
);

CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans(trace_id);
CREATE INDEX IF NOT EXISTS idx_spans_name ON spans(name);
"#;
