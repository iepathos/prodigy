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
    error_message TEXT,
    estimated_iterations INTEGER DEFAULT 3,
    
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
    completed_at DATETIME,
    error_message TEXT,
    
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
    duration_seconds INTEGER,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    
    FOREIGN KEY (iteration_id) REFERENCES loop_iterations (id)
);

-- Loop termination conditions table
CREATE TABLE IF NOT EXISTS loop_termination_events (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    condition_type TEXT NOT NULL,
    condition_data TEXT,
    triggered_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    reason TEXT,
    
    FOREIGN KEY (session_id) REFERENCES loop_sessions (id)
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_loop_sessions_project ON loop_sessions (project_id);
CREATE INDEX IF NOT EXISTS idx_loop_sessions_status ON loop_sessions (status);
CREATE INDEX IF NOT EXISTS idx_loop_sessions_created ON loop_sessions (created_at);
CREATE INDEX IF NOT EXISTS idx_loop_iterations_session ON loop_iterations (session_id);
CREATE INDEX IF NOT EXISTS idx_loop_iterations_number ON loop_iterations (session_id, iteration_number);
CREATE INDEX IF NOT EXISTS idx_loop_actions_iteration ON loop_actions (iteration_id);
CREATE INDEX IF NOT EXISTS idx_loop_actions_file ON loop_actions (file_path);
CREATE INDEX IF NOT EXISTS idx_loop_actions_type ON loop_actions (action_type);
CREATE INDEX IF NOT EXISTS idx_loop_termination_session ON loop_termination_events (session_id);