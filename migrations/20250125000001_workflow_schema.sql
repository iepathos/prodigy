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