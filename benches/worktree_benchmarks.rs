//! Performance benchmarks for worktree listing operations

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use prodigy::subprocess::SubprocessManager;
use prodigy::worktree::{WorktreeManager, WorktreeStatus};
use std::hint::black_box;
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

/// Create test worktree states for benchmarking as JSON
fn create_test_worktree_states_json(count: usize) -> Vec<serde_json::Value> {
    (0..count)
        .map(|i| {
            serde_json::json!({
                "session_id": format!("session-{}", i),
                "status": match i % 4 {
                    0 => "in_progress",
                    1 => "completed",
                    2 => "failed",
                    _ => "interrupted",
                },
                "branch": format!("feature-{}", i),
                "worktree_name": format!("session-{}", i),
                "created_at": (chrono::Utc::now() - chrono::Duration::hours(i as i64)).to_rfc3339(),
                "updated_at": (chrono::Utc::now() - chrono::Duration::minutes((i * 5) as i64)).to_rfc3339(),
                "error": if i % 4 == 2 {
                    Some(format!("Error in session {}", i))
                } else {
                    None
                },
                "stats": {
                    "files_changed": (i % 20) as u32,
                    "commits": (i % 10) as u32,
                    "last_commit_sha": None as Option<String>
                },
                "iterations": { "completed": 0, "max": 5 },
                "merged": false,
                "merged_at": null,
                "merge_prompt_shown": false,
                "merge_prompt_response": null,
                "interrupted_at": null,
                "interruption_type": null,
                "last_checkpoint": null,
                "resumable": false
            })
        })
        .collect()
}

/// Setup test environment with worktree states
async fn setup_test_worktrees(
    manager: &WorktreeManager,
    count: usize,
) -> anyhow::Result<()> {
    let metadata_dir = manager.base_dir.join(".metadata");
    std::fs::create_dir_all(&metadata_dir)?;

    let states = create_test_worktree_states_json(count);

    for state in states {
        // Save state file
        let session_id = state["session_id"].as_str().unwrap();
        let state_file = metadata_dir.join(format!("{}.json", session_id));
        std::fs::write(&state_file, serde_json::to_string(&state)?)?;

        // Create worktree directory
        let wt_dir = manager.base_dir.join(session_id);
        std::fs::create_dir_all(&wt_dir)?;

        // Create session state with workflow info for some sessions
        if session_id.ends_with('0') || session_id.ends_with('5') {
            let prodigy_dir = wt_dir.join(".prodigy");
            std::fs::create_dir_all(&prodigy_dir)?;

            let session_state = serde_json::json!({
                "session_id": session_id,
                "workflow_state": {
                    "workflow_path": format!("workflows/{}.yaml", session_id),
                    "input_args": vec!["arg1", "arg2"],
                    "current_step": 5,
                    "completed_steps": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
                }
            });

            let session_state_file = prodigy_dir.join("session_state.json");
            std::fs::write(&session_state_file, serde_json::to_string(&session_state)?)?;
        }
    }

    Ok(())
}

fn bench_list_sessions(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("worktree_list_sessions");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for count in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("list_basic", count),
            count,
            |b, &count| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let subprocess = SubprocessManager::production();
                        let manager = WorktreeManager::new(
                            temp_dir.path().to_path_buf(),
                            subprocess,
                        ).unwrap();

                        rt.block_on(setup_test_worktrees(&manager, count)).unwrap();
                        (manager, temp_dir)
                    },
                    |(manager, _temp_dir)| async move {
                        black_box(manager.list_sessions().await.unwrap())
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_list_detailed(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("worktree_list_detailed");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    // Benchmark with 100 sessions to verify sub-500ms requirement
    group.bench_function("list_detailed_100_sessions", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let temp_dir = TempDir::new().unwrap();
                let subprocess = SubprocessManager::production();
                let manager = WorktreeManager::new(
                    temp_dir.path().to_path_buf(),
                    subprocess,
                ).unwrap();

                rt.block_on(setup_test_worktrees(&manager, 100)).unwrap();
                (manager, temp_dir)
            },
            |(manager, _temp_dir)| async move {
                black_box(manager.list_detailed().await.unwrap())
            },
            BatchSize::SmallInput,
        )
    });

    // Test with different session counts
    for count in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("varying_counts", count),
            count,
            |b, &count| {
                b.to_async(&rt).iter_batched(
                    || {
                        let temp_dir = TempDir::new().unwrap();
                        let subprocess = SubprocessManager::production();
                        let manager = WorktreeManager::new(
                            temp_dir.path().to_path_buf(),
                            subprocess,
                        ).unwrap();

                        rt.block_on(setup_test_worktrees(&manager, count)).unwrap();
                        (manager, temp_dir)
                    },
                    |(manager, _temp_dir)| async move {
                        black_box(manager.list_detailed().await.unwrap())
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_display_formatting(c: &mut Criterion) {
    use prodigy::worktree::display::{EnhancedSessionInfo, DetailedWorktreeList, WorktreeSummary, SessionDisplay};
    use std::path::PathBuf;

    let mut group = c.benchmark_group("display_formatting");

    // Create test data
    let sessions: Vec<EnhancedSessionInfo> = (0..100)
        .map(|i| EnhancedSessionInfo {
            session_id: format!("session-{}", i),
            status: match i % 4 {
                0 => WorktreeStatus::InProgress,
                1 => WorktreeStatus::Completed,
                2 => WorktreeStatus::Failed,
                _ => WorktreeStatus::Interrupted,
            },
            workflow_path: Some(PathBuf::from(format!("workflow-{}.yaml", i))),
            workflow_args: vec!["arg1".to_string(), "arg2".to_string()],
            started_at: chrono::Utc::now() - chrono::Duration::hours(i as i64),
            last_activity: chrono::Utc::now() - chrono::Duration::minutes((i * 5) as i64),
            current_step: (i % 10) as usize,
            total_steps: Some(10),
            error_summary: if i % 4 == 2 {
                Some(format!("Error in session {}", i))
            } else {
                None
            },
            branch_name: format!("feature-{}", i),
            parent_branch: Some("main".to_string()),
            worktree_path: PathBuf::from(format!("/tmp/worktree-{}", i)),
            files_changed: (i % 20) as u32,
            commits: (i % 10) as u32,
            items_processed: if i % 3 == 0 { Some((i * 10) as u32) } else { None },
            total_items: if i % 3 == 0 { Some(1000) } else { None },
        })
        .collect();

    let list = DetailedWorktreeList {
        sessions: sessions.clone(),
        summary: WorktreeSummary {
            total: 100,
            in_progress: 25,
            interrupted: 25,
            failed: 25,
            completed: 25,
        },
    };

    group.bench_function("format_default_100_sessions", |b| {
        b.iter(|| {
            black_box(list.format_default())
        })
    });

    group.bench_function("format_verbose_100_sessions", |b| {
        b.iter(|| {
            black_box(list.format_verbose())
        })
    });

    group.bench_function("format_json_100_sessions", |b| {
        b.iter(|| {
            black_box(list.format_json())
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_list_sessions,
    bench_list_detailed,
    bench_display_formatting
);
criterion_main!(benches);