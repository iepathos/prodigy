use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::sync::RwLock;
use tokio::time;

use super::progress_tracker::{AgentProgress, PhaseProgress, ProgressRenderer, WorkflowProgress};

pub enum DisplayMode {
    Rich,
    Simple,
    Json,
    None,
}

pub struct MultiProgressDisplay {
    multi_progress: MultiProgress,
    main_bar: ProgressBar,
    phase_bars: Arc<RwLock<HashMap<String, ProgressBar>>>,
    agent_bars: Arc<RwLock<HashMap<String, ProgressBar>>>,
    log_area: ProgressBar,
    update_interval: Duration,
    mode: DisplayMode,
}

impl MultiProgressDisplay {
    pub fn new(mode: DisplayMode) -> Self {
        let multi_progress = MultiProgress::new();

        // Main workflow progress
        let main_bar = multi_progress.add(ProgressBar::new(100));
        main_bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) | {msg}")
                .unwrap()
                .progress_chars("#>-"),
        );

        // Log area for messages
        let log_area = multi_progress.add(ProgressBar::new_spinner());
        log_area.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.blue} {msg}")
                .unwrap(),
        );

        let display = Self {
            multi_progress,
            main_bar,
            phase_bars: Arc::new(RwLock::new(HashMap::new())),
            agent_bars: Arc::new(RwLock::new(HashMap::new())),
            log_area,
            update_interval: Duration::from_millis(100),
            mode,
        };

        display.start_update_loop();
        display
    }

    pub async fn update_workflow(&self, progress: &WorkflowProgress) {
        if matches!(self.mode, DisplayMode::None) {
            return;
        }

        let percentage = if progress.total_steps > 0 {
            (progress.completed_steps as f64 / progress.total_steps as f64 * 100.0) as u64
        } else {
            0
        };

        self.main_bar.set_position(percentage);

        let eta_str = progress
            .eta
            .map(|d| format!("ETA: {}", format_duration(d)))
            .unwrap_or_else(|| "ETA: calculating...".to_string());

        let msg = format!(
            "{} | {} | CPU: {:.1}% | Mem: {}",
            progress
                .current_phase
                .as_ref()
                .unwrap_or(&"Starting".to_string()),
            eta_str,
            progress.resource_usage.cpu_percent,
            humanize_bytes(progress.resource_usage.memory_bytes)
        );

        self.main_bar.set_message(msg);
    }

    pub async fn add_phase(&self, phase_id: &str, total_items: usize) -> Result<()> {
        if matches!(self.mode, DisplayMode::None) {
            return Ok(());
        }

        let bar = self
            .multi_progress
            .add(ProgressBar::new(total_items as u64));
        bar.set_style(
            ProgressStyle::default_bar()
                .template("  {spinner:.blue} [{bar:30.yellow/red}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );

        let mut phase_bars = self.phase_bars.write().await;
        phase_bars.insert(phase_id.to_string(), bar);
        Ok(())
    }

    pub async fn update_phase(&self, phase: &PhaseProgress) {
        if matches!(self.mode, DisplayMode::None) {
            return;
        }

        let phase_bars = self.phase_bars.read().await;
        if let Some(bar) = phase_bars.get(&phase.name) {
            bar.set_position(phase.processed_items as u64);

            let success_rate = if phase.processed_items > 0 {
                (phase.successful_items as f64 / phase.processed_items as f64 * 100.0) as u32
            } else {
                0
            };

            let msg = format!(
                "Success: {}% | Rate: {:.1}/s | Active: {}",
                success_rate,
                phase.throughput,
                phase.active_agents.len()
            );

            bar.set_message(msg);
        }
    }

    pub async fn add_agent(&self, agent_id: &str) -> Result<()> {
        if matches!(self.mode, DisplayMode::None) {
            return Ok(());
        }

        let bar = self.multi_progress.add(ProgressBar::new_spinner());
        bar.set_style(
            ProgressStyle::default_spinner()
                .template("    {spinner:.green} Agent {prefix}: {msg}")
                .unwrap(),
        );
        bar.set_prefix(agent_id.to_string());

        let mut agent_bars = self.agent_bars.write().await;
        agent_bars.insert(agent_id.to_string(), bar);
        Ok(())
    }

    pub async fn update_agent(&self, agent: &AgentProgress) {
        if matches!(self.mode, DisplayMode::None) {
            return;
        }

        let agent_bars = self.agent_bars.read().await;
        if let Some(bar) = agent_bars.get(&agent.id) {
            let msg = match (&agent.current_item, &agent.current_step) {
                (Some(item), Some(step)) => format!("{} - {}", item, step),
                (Some(item), None) => item.clone(),
                (None, Some(step)) => step.clone(),
                (None, None) => "Idle".to_string(),
            };

            bar.set_message(msg);
        }
    }

    pub async fn log_message(&self, message: &str) {
        if matches!(self.mode, DisplayMode::None) {
            return;
        }

        self.log_area.set_message(message.to_string());
    }

    pub async fn clear(&self) {
        self.multi_progress.clear().ok();
    }

    fn start_update_loop(&self) {
        // MultiProgress in indicatif doesn't need manual ticking
        // It's handled automatically by the progress bars
    }
}

#[async_trait::async_trait]
impl ProgressRenderer for MultiProgressDisplay {
    async fn update_display(
        &self,
        workflow: &WorkflowProgress,
        phases: &HashMap<String, PhaseProgress>,
    ) -> Result<()> {
        self.update_workflow(workflow).await;

        for phase in phases.values() {
            self.update_phase(phase).await;

            for agent in &phase.active_agents {
                self.update_agent(agent).await;
            }
        }

        Ok(())
    }
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn humanize_bytes(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= THRESHOLD && unit_index < UNITS.len() - 1 {
        size /= THRESHOLD;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as usize, UNITS[unit_index])
    } else {
        format!("{:.2} {}", size, UNITS[unit_index])
    }
}

// TODO: Implement JsonProgressRenderer with proper serialization
