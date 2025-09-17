//! Data models for Claude session analytics

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a Claude session from JSONL logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub project_path: String,
    pub jsonl_path: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub model: Option<String>,
    pub events: Vec<SessionEvent>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_tokens: u64,
    pub tool_invocations: Vec<ToolInvocation>,
}

impl Session {
    pub fn total_input_tokens(&self) -> u64 {
        self.total_input_tokens
    }

    pub fn total_output_tokens(&self) -> u64 {
        self.total_output_tokens
    }

    pub fn total_cache_tokens(&self) -> u64 {
        self.total_cache_tokens
    }

    pub fn tool_invocations(&self) -> &[ToolInvocation] {
        &self.tool_invocations
    }
}

/// Event types found in Claude JSONL logs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEvent {
    System {
        timestamp: DateTime<Utc>,
        message: String,
    },
    Assistant {
        timestamp: DateTime<Utc>,
        content: String,
        model: Option<String>,
    },
    ToolUse {
        timestamp: DateTime<Utc>,
        tool_name: String,
        parameters: serde_json::Value,
    },
    ToolResult {
        timestamp: DateTime<Utc>,
        tool_name: String,
        result: serde_json::Value,
        duration_ms: Option<u64>,
    },
    Error {
        timestamp: DateTime<Utc>,
        error_type: String,
        message: String,
    },
}

impl SessionEvent {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::System { timestamp, .. }
            | Self::Assistant { timestamp, .. }
            | Self::ToolUse { timestamp, .. }
            | Self::ToolResult { timestamp, .. }
            | Self::Error { timestamp, .. } => *timestamp,
        }
    }
}

/// Tool invocation with usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub name: String,
    pub invoked_at: DateTime<Utc>,
    pub duration_ms: Option<u64>,
    pub parameters: serde_json::Value,
    pub result_size: Option<usize>,
}

/// Cost calculation for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_tokens: u64,
    pub estimated_cost_usd: f64,
}

impl Cost {
    /// Export cost data to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "input_tokens": self.input_tokens,
            "output_tokens": self.output_tokens,
            "cache_tokens": self.cache_tokens,
            "estimated_cost_usd": self.estimated_cost_usd,
            "total_tokens": self.input_tokens + self.output_tokens + self.cache_tokens
        })
    }

    /// Export cost data to CSV format
    pub fn to_csv_row(&self) -> String {
        format!(
            "{},{},{},{:.4}",
            self.input_tokens,
            self.output_tokens,
            self.cache_tokens,
            self.estimated_cost_usd
        )
    }

    /// Get CSV header for cost data
    pub fn csv_header() -> &'static str {
        "input_tokens,output_tokens,cache_tokens,estimated_cost_usd"
    }
}

/// Statistics for tool usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStats {
    pub stats: HashMap<String, ToolStat>,
}

impl ToolStats {
    /// Export tool stats to JSON
    pub fn to_json(&self) -> serde_json::Value {
        let tools: Vec<serde_json::Value> = self.stats
            .values()
            .map(|stat| serde_json::json!({
                "name": stat.name,
                "total_invocations": stat.total_invocations,
                "total_duration_ms": stat.total_duration_ms,
                "average_duration_ms": stat.average_duration_ms,
                "min_duration_ms": stat.min_duration_ms,
                "max_duration_ms": stat.max_duration_ms,
                "failure_count": stat.failure_count,
                "success_rate": stat.success_rate
            }))
            .collect();

        serde_json::json!({
            "tools": tools,
            "total_tools": self.stats.len()
        })
    }

    /// Export tool stats to CSV format
    pub fn to_csv(&self) -> String {
        let mut csv = String::from("tool_name,total_invocations,avg_duration_ms,min_duration_ms,max_duration_ms,failure_count,success_rate\n");

        let mut sorted_stats: Vec<_> = self.stats.values().collect();
        sorted_stats.sort_by(|a, b| b.total_invocations.cmp(&a.total_invocations));

        for stat in sorted_stats {
            csv.push_str(&format!(
                "{},{},{},{},{},{},{:.2}\n",
                stat.name,
                stat.total_invocations,
                stat.average_duration_ms,
                stat.min_duration_ms,
                stat.max_duration_ms,
                stat.failure_count,
                stat.success_rate
            ));
        }

        csv
    }
}

/// Individual tool statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStat {
    pub name: String,
    pub total_invocations: u64,
    pub total_duration_ms: u64,
    pub average_duration_ms: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub failure_count: u64,
    pub success_rate: f64,
}

impl ToolStat {
    pub fn from(invocation: &ToolInvocation) -> Self {
        let duration = invocation.duration_ms.unwrap_or(0);
        Self {
            name: invocation.name.clone(),
            total_invocations: 1,
            total_duration_ms: duration,
            average_duration_ms: duration,
            min_duration_ms: duration,
            max_duration_ms: duration,
            failure_count: 0,
            success_rate: 100.0,
        }
    }

    pub fn increment(&mut self, invocation: &ToolInvocation) {
        self.total_invocations += 1;
        if let Some(duration) = invocation.duration_ms {
            self.total_duration_ms += duration;
            self.average_duration_ms = self.total_duration_ms / self.total_invocations;
            self.min_duration_ms = self.min_duration_ms.min(duration);
            self.max_duration_ms = self.max_duration_ms.max(duration);
        }
        self.success_rate = ((self.total_invocations - self.failure_count) as f64
            / self.total_invocations as f64)
            * 100.0;
    }
}

/// Time range for filtering analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Replay event for session replay functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub content: serde_json::Value,
    pub metadata: HashMap<String, String>,
}

/// Session index for efficient querying with database backend
pub struct SessionIndex {
    db: Option<std::sync::Arc<super::persistence::AnalyticsDatabase>>,
    cache: HashMap<String, Session>,
}

impl SessionIndex {
    pub fn new() -> Self {
        Self {
            db: None,
            cache: HashMap::new(),
        }
    }

    /// Create index with database backend
    pub fn with_database(db: std::sync::Arc<super::persistence::AnalyticsDatabase>) -> Self {
        Self {
            db: Some(db),
            cache: HashMap::new(),
        }
    }

    pub async fn add_event(&mut self, session_id: &str, event: SessionEvent) -> anyhow::Result<()> {
        // Update in-memory cache
        if let Some(session) = self.cache.get_mut(session_id) {
            session.events.push(event);
        } else {
            // Create new session if not exists
            let session = Session {
                session_id: session_id.to_string(),
                project_path: String::new(),
                jsonl_path: String::new(),
                started_at: event.timestamp(),
                completed_at: None,
                model: None,
                events: vec![event],
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cache_tokens: 0,
                tool_invocations: Vec::new(),
            };
            self.cache.insert(session_id.to_string(), session.clone());

            // Persist to database if available
            if let Some(db) = &self.db {
                db.upsert_session(&session).await?;
            }
        }
        Ok(())
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Session> {
        // Check cache first
        if let Some(session) = self.cache.get(session_id) {
            return Ok(session.clone());
        }

        // Fall back to database
        if let Some(db) = &self.db {
            if let Some(session) = db.get_session(session_id).await? {
                return Ok(session);
            }
        }

        Err(anyhow::anyhow!("Session {} not found", session_id))
    }

    pub async fn query_sessions(&self, time_range: TimeRange) -> anyhow::Result<Vec<Session>> {
        // If database is available, use it
        if let Some(db) = &self.db {
            return db.query_sessions(time_range.start, time_range.end).await;
        }

        // Otherwise use cache
        Ok(self
            .cache
            .values()
            .filter(|s| s.started_at >= time_range.start && s.started_at <= time_range.end)
            .cloned()
            .collect())
    }

    /// Persist all cached sessions to database
    pub async fn flush_to_database(&self) -> anyhow::Result<()> {
        if let Some(db) = &self.db {
            for session in self.cache.values() {
                db.upsert_session(session).await?;
            }
        }
        Ok(())
    }

    /// Insert session directly into cache (for testing)
    #[cfg(test)]
    pub fn insert_test_session(&mut self, id: String, session: Session) {
        self.cache.insert(id, session);
    }
}

/// Claude pricing model for cost calculation
pub struct PricingModel {
    pub input_price_per_million: f64,
    pub output_price_per_million: f64,
    pub cache_price_per_million: f64,
}

impl Default for PricingModel {
    fn default() -> Self {
        // Default Claude Sonnet 3.5 pricing (as of 2025)
        Self {
            input_price_per_million: 3.0,
            output_price_per_million: 15.0,
            cache_price_per_million: 0.375,
        }
    }
}

impl PricingModel {
    pub fn calculate_cost(&self, input: u64, output: u64, cache: u64) -> f64 {
        let input_cost = (input as f64 / 1_000_000.0) * self.input_price_per_million;
        let output_cost = (output as f64 / 1_000_000.0) * self.output_price_per_million;
        let cache_cost = (cache as f64 / 1_000_000.0) * self.cache_price_per_million;
        input_cost + output_cost + cache_cost
    }
}
