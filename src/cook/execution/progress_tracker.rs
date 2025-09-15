use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct WorkflowProgress {
    pub id: String,
    pub name: String,
    pub status: WorkflowStatus,
    pub start_time: Instant,
    pub eta: Option<Duration>,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub current_phase: Option<String>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgressSnapshot {
    pub id: String,
    pub name: String,
    pub status: WorkflowStatus,
    pub elapsed_secs: u64,
    pub eta: Option<Duration>,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub current_phase: Option<String>,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone)]
pub struct PhaseProgress {
    pub name: String,
    pub phase_type: PhaseType,
    pub status: PhaseStatus,
    pub start_time: Instant,
    pub total_items: usize,
    pub processed_items: usize,
    pub successful_items: usize,
    pub failed_items: usize,
    pub active_agents: Vec<AgentProgress>,
    pub throughput: f64,
    pub avg_item_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseProgressSnapshot {
    pub name: String,
    pub phase_type: PhaseType,
    pub status: PhaseStatus,
    pub elapsed_secs: u64,
    pub total_items: usize,
    pub processed_items: usize,
    pub successful_items: usize,
    pub failed_items: usize,
    pub active_agents: Vec<AgentProgressSnapshot>,
    pub throughput: f64,
    pub avg_item_time: Duration,
}

#[derive(Debug, Clone)]
pub struct AgentProgress {
    pub id: String,
    pub worktree: String,
    pub current_item: Option<String>,
    pub status: AgentStatus,
    pub items_processed: usize,
    pub start_time: Instant,
    pub last_update: Instant,
    pub current_step: Option<String>,
    pub memory_usage: usize,
    pub cpu_usage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProgressSnapshot {
    pub id: String,
    pub worktree: String,
    pub current_item: Option<String>,
    pub status: AgentStatus,
    pub items_processed: usize,
    pub elapsed_secs: u64,
    pub last_update_secs: u64,
    pub current_step: Option<String>,
    pub memory_usage: usize,
    pub cpu_usage: f32,
}

#[derive(Debug, Clone)]
pub struct ItemProgress {
    pub id: String,
    pub status: ItemStatus,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub agent_id: Option<String>,
    pub error: Option<String>,
    pub retry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_percent: f32,
    pub memory_bytes: usize,
    pub disk_bytes_written: usize,
    pub disk_bytes_read: usize,
    pub network_bytes_sent: usize,
    pub network_bytes_received: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseType {
    Setup,
    Map,
    Reduce,
    Sequential,
    Parallel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PhaseStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Working,
    Completed,
    Failed,
    Terminated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ItemStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Retrying,
}

pub struct ProgressTracker {
    workflow_progress: Arc<RwLock<WorkflowProgress>>,
    phase_progress: Arc<RwLock<HashMap<String, PhaseProgress>>>,
    item_progress: Arc<RwLock<HashMap<String, ItemProgress>>>,
    metrics: Arc<SystemMetrics>,
    history: Arc<ProgressHistory>,
    renderer: Box<dyn ProgressRenderer>,
}

impl ProgressTracker {
    pub fn new(
        workflow_id: String,
        workflow_name: String,
        renderer: Box<dyn ProgressRenderer>,
    ) -> Self {
        let workflow_progress = WorkflowProgress {
            id: workflow_id,
            name: workflow_name,
            status: WorkflowStatus::Pending,
            start_time: Instant::now(),
            eta: None,
            total_steps: 0,
            completed_steps: 0,
            failed_steps: 0,
            current_phase: None,
            resource_usage: ResourceUsage::default(),
        };

        Self {
            workflow_progress: Arc::new(RwLock::new(workflow_progress)),
            phase_progress: Arc::new(RwLock::new(HashMap::new())),
            item_progress: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(SystemMetrics::new()),
            history: Arc::new(ProgressHistory::new()),
            renderer,
        }
    }

    pub async fn start_workflow(&self, total_steps: usize) -> Result<()> {
        let mut progress = self.workflow_progress.write().await;
        progress.status = WorkflowStatus::Running;
        progress.total_steps = total_steps;
        progress.start_time = Instant::now();
        Ok(())
    }

    pub async fn start_phase(
        &self,
        phase_id: String,
        phase_type: PhaseType,
        total_items: usize,
    ) -> Result<()> {
        let phase = PhaseProgress {
            name: phase_id.clone(),
            phase_type,
            status: PhaseStatus::Running,
            start_time: Instant::now(),
            total_items,
            processed_items: 0,
            successful_items: 0,
            failed_items: 0,
            active_agents: Vec::new(),
            throughput: 0.0,
            avg_item_time: Duration::ZERO,
        };

        let mut phases = self.phase_progress.write().await;
        phases.insert(phase_id.clone(), phase);

        let mut workflow = self.workflow_progress.write().await;
        workflow.current_phase = Some(phase_id);

        self.renderer.update_display(&*workflow, &*phases).await?;
        Ok(())
    }

    pub async fn update_agent(&self, phase_id: &str, agent: AgentProgress) -> Result<()> {
        let mut phases = self.phase_progress.write().await;
        if let Some(phase) = phases.get_mut(phase_id) {
            if let Some(existing) = phase.active_agents.iter_mut().find(|a| a.id == agent.id) {
                *existing = agent;
            } else {
                phase.active_agents.push(agent);
            }

            self.calculate_phase_metrics(phase);
        }
        Ok(())
    }

    pub async fn complete_item(
        &self,
        phase_id: &str,
        item_id: String,
        success: bool,
    ) -> Result<()> {
        let mut phases = self.phase_progress.write().await;
        if let Some(phase) = phases.get_mut(phase_id) {
            phase.processed_items += 1;
            if success {
                phase.successful_items += 1;
            } else {
                phase.failed_items += 1;
            }

            self.calculate_phase_metrics(phase);
        }

        let mut items = self.item_progress.write().await;
        if let Some(item) = items.get_mut(&item_id) {
            item.status = if success {
                ItemStatus::Completed
            } else {
                ItemStatus::Failed
            };
            item.end_time = Some(Instant::now());
        }

        self.update_eta().await?;
        Ok(())
    }

    pub async fn complete_phase(&self, phase_id: &str, success: bool) -> Result<()> {
        let mut phases = self.phase_progress.write().await;
        if let Some(phase) = phases.get_mut(phase_id) {
            phase.status = if success {
                PhaseStatus::Completed
            } else {
                PhaseStatus::Failed
            };
        }

        let mut workflow = self.workflow_progress.write().await;
        if success {
            workflow.completed_steps += 1;
        } else {
            workflow.failed_steps += 1;
        }

        Ok(())
    }

    pub async fn update_resource_usage(&self) -> Result<()> {
        let usage = self.metrics.collect_metrics().await?;
        let mut workflow = self.workflow_progress.write().await;
        workflow.resource_usage = usage;
        Ok(())
    }

    pub async fn snapshot(&self) -> ProgressSnapshot {
        let workflow = self.workflow_progress.read().await.clone();
        let phases = self.phase_progress.read().await.clone();
        let items = self.item_progress.read().await.clone();

        ProgressSnapshot {
            workflow,
            phases,
            items,
            timestamp: Instant::now(),
        }
    }

    pub async fn serializable_snapshot(&self) -> SerializableProgressSnapshot {
        let workflow = self.workflow_progress.read().await.clone();
        let phases = self.phase_progress.read().await.clone();

        let workflow_snapshot = WorkflowProgressSnapshot {
            id: workflow.id.clone(),
            name: workflow.name.clone(),
            status: workflow.status.clone(),
            elapsed_secs: workflow.start_time.elapsed().as_secs(),
            eta: workflow.eta,
            total_steps: workflow.total_steps,
            completed_steps: workflow.completed_steps,
            failed_steps: workflow.failed_steps,
            current_phase: workflow.current_phase.clone(),
            resource_usage: workflow.resource_usage.clone(),
        };

        let phase_snapshots: HashMap<String, PhaseProgressSnapshot> = phases
            .into_iter()
            .map(|(k, v)| {
                let snapshot = PhaseProgressSnapshot {
                    name: v.name.clone(),
                    phase_type: v.phase_type.clone(),
                    status: v.status.clone(),
                    elapsed_secs: v.start_time.elapsed().as_secs(),
                    total_items: v.total_items,
                    processed_items: v.processed_items,
                    successful_items: v.successful_items,
                    failed_items: v.failed_items,
                    active_agents: v
                        .active_agents
                        .iter()
                        .map(|a| AgentProgressSnapshot {
                            id: a.id.clone(),
                            worktree: a.worktree.clone(),
                            current_item: a.current_item.clone(),
                            status: a.status.clone(),
                            items_processed: a.items_processed,
                            elapsed_secs: a.start_time.elapsed().as_secs(),
                            last_update_secs: a.last_update.elapsed().as_secs(),
                            current_step: a.current_step.clone(),
                            memory_usage: a.memory_usage,
                            cpu_usage: a.cpu_usage,
                        })
                        .collect(),
                    throughput: v.throughput,
                    avg_item_time: v.avg_item_time,
                };
                (k, snapshot)
            })
            .collect();

        SerializableProgressSnapshot {
            workflow: workflow_snapshot,
            phases: phase_snapshots,
            timestamp_secs: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    async fn update_eta(&self) -> Result<()> {
        let workflow = self.workflow_progress.read().await;
        let phases = self.phase_progress.read().await;

        let eta = self
            .history
            .calculate_eta(workflow.completed_steps, workflow.total_steps, &phases)
            .await;

        drop(workflow);
        drop(phases);

        let mut workflow = self.workflow_progress.write().await;
        workflow.eta = eta;
        Ok(())
    }

    fn calculate_phase_metrics(&self, phase: &mut PhaseProgress) {
        if phase.processed_items == 0 {
            return;
        }

        let elapsed = phase.start_time.elapsed();
        phase.throughput = phase.processed_items as f64 / elapsed.as_secs_f64();
        phase.avg_item_time = elapsed / phase.processed_items as u32;
    }
}

impl Default for ResourceUsage {
    fn default() -> Self {
        Self {
            cpu_percent: 0.0,
            memory_bytes: 0,
            disk_bytes_written: 0,
            disk_bytes_read: 0,
            network_bytes_sent: 0,
            network_bytes_received: 0,
        }
    }
}

#[async_trait::async_trait]
pub trait ProgressRenderer: Send + Sync {
    async fn update_display(
        &self,
        workflow: &WorkflowProgress,
        phases: &HashMap<String, PhaseProgress>,
    ) -> Result<()>;
}

pub struct SystemMetrics {
    system: Arc<RwLock<System>>,
}

impl SystemMetrics {
    pub fn new() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            system: Arc::new(RwLock::new(system)),
        }
    }

    pub async fn collect_metrics(&self) -> Result<ResourceUsage> {
        let mut system = self.system.write().await;

        // Refresh system data
        system.refresh_all();

        // Calculate CPU usage
        let cpu_percent = system.global_cpu_usage();

        // Calculate memory usage
        let used_memory = system.used_memory();

        // For now, use placeholder values for disk and network I/O
        // as the sysinfo API doesn't provide these directly
        let disk_bytes_read = 0usize;
        let disk_bytes_written = 0usize;
        let network_bytes_sent = 0usize;
        let network_bytes_received = 0usize;

        Ok(ResourceUsage {
            cpu_percent,
            memory_bytes: used_memory as usize,
            disk_bytes_written,
            disk_bytes_read,
            network_bytes_sent,
            network_bytes_received,
        })
    }
}

pub struct ProgressHistory {
    history: RwLock<VecDeque<HistoryPoint>>,
    window_size: usize,
}

#[derive(Clone)]
struct HistoryPoint {
    timestamp: Instant,
    items_completed: usize,
    phases_completed: usize,
}

impl ProgressHistory {
    pub fn new() -> Self {
        Self {
            history: RwLock::new(VecDeque::new()),
            window_size: 20,
        }
    }

    pub async fn calculate_eta(
        &self,
        completed_steps: usize,
        total_steps: usize,
        _phases: &HashMap<String, PhaseProgress>,
    ) -> Option<Duration> {
        let mut history = self.history.write().await;

        let now = Instant::now();
        history.push_back(HistoryPoint {
            timestamp: now,
            items_completed: completed_steps,
            phases_completed: completed_steps,
        });

        while history.len() > self.window_size {
            history.pop_front();
        }

        if history.len() < 2 {
            return None;
        }

        let first = &history[0];
        let elapsed = now.duration_since(first.timestamp);
        let items_done = completed_steps.saturating_sub(first.items_completed);

        if items_done == 0 {
            return None;
        }

        let rate = items_done as f64 / elapsed.as_secs_f64();
        let remaining = total_steps.saturating_sub(completed_steps);

        if rate > 0.0 {
            Some(Duration::from_secs_f64(remaining as f64 / rate))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSnapshot {
    pub workflow: WorkflowProgress,
    pub phases: HashMap<String, PhaseProgress>,
    pub items: HashMap<String, ItemProgress>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableProgressSnapshot {
    pub workflow: WorkflowProgressSnapshot,
    pub phases: HashMap<String, PhaseProgressSnapshot>,
    pub timestamp_secs: u64,
}
