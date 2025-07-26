//! Adapter to use simple_state with the improve command

use anyhow::Result;
use chrono::Utc;

use crate::analyzer::AnalyzerResult;
use crate::simple_state::{
    CacheManager, Improvement, ProjectAnalysis, SessionRecord, StateManager,
};

use super::session::ImproveState;

/// Adapter for using simple state with improve command
pub struct StateAdapter {
    state_mgr: StateManager,
    cache_mgr: CacheManager,
}

impl StateAdapter {
    /// Create a new state adapter
    pub fn new() -> Result<Self> {
        Ok(Self {
            state_mgr: StateManager::new()?,
            cache_mgr: CacheManager::new()?,
        })
    }

    /// Save the current state
    pub fn save(&self) -> Result<()> {
        self.state_mgr.save()?;
        Ok(())
    }

    /// Get cached project analysis
    pub fn get_cached_analysis(&self) -> Result<Option<AnalyzerResult>> {
        // Check if we have a cached analysis
        if let Some(_analysis) = self.cache_mgr.get::<ProjectAnalysis>("project_analysis")? {
            // For now, return None as we'd need to convert ProjectAnalysis to AnalyzerResult
            // In a real implementation, we'd store the full AnalyzerResult
            Ok(None)
        } else {
            Ok(None)
        }
    }

    /// Cache project analysis
    pub fn cache_analysis(&self, result: &AnalyzerResult) -> Result<()> {
        let analysis = ProjectAnalysis {
            language: result.language.to_string(),
            framework: result.framework.as_ref().map(|f| f.to_string()),
            health_score: result.health_score,
            focus_areas: result
                .focus_areas
                .primary
                .iter()
                .map(|a| a.to_string())
                .collect(),
            analyzed_at: Utc::now(),
        };

        self.cache_mgr.set("project_analysis", &analysis)?;
        Ok(())
    }

    /// Start a new improvement session
    pub fn start_session(&mut self, initial_score: f32) -> SessionRecord {
        let session = SessionRecord::new(initial_score);
        self.state_mgr.state_mut().sessions.active = Some(session.session_id.clone());
        session
    }

    /// Complete an improvement session
    pub fn complete_session(&mut self, mut session: SessionRecord, final_score: f32) -> Result<()> {
        session.complete(final_score);

        // Record session
        self.state_mgr.record_session(session)?;
        self.state_mgr.state_mut().sessions.active = None;

        Ok(())
    }

    /// Add an improvement to the current session
    pub fn add_improvement(
        &mut self,
        session: &mut SessionRecord,
        file: String,
        description: String,
    ) {
        let improvement = Improvement {
            file: file.clone(),
            description,
        };

        session.improvements.push(improvement);

        // Track changed file
        if !session.files_changed.contains(&file) {
            session.files_changed.push(file);
        }
    }

    /// Get current state for improve session
    pub fn get_improve_state(&self) -> ImproveState {
        let state = self.state_mgr.state();

        ImproveState {
            last_run: state.last_run.unwrap_or_else(Utc::now),
            current_score: state.current_score,
            improvement_history: Vec::new(), // Not directly compatible
        }
    }

    /// Update session metrics
    pub fn update_metrics(
        &mut self,
        session: &mut SessionRecord,
        claude_calls: u32,
        tokens_used: u32,
    ) {
        session.metrics.claude_calls += claude_calls;
        session.metrics.tokens_used += tokens_used;
    }

    /// Get history of sessions
    pub fn get_history(&self, date: Option<&str>) -> Result<Vec<SessionRecord>> {
        self.state_mgr.get_history(date)
    }

    /// Check if this is the first run
    pub fn is_first_run(&self) -> bool {
        self.state_mgr.state().stats.total_runs == 0
    }

    /// Get current score
    pub fn current_score(&self) -> f32 {
        self.state_mgr.state().current_score
    }
}
