//! Performance benchmarks for git operations with large repositories
//!
//! These benchmarks validate the performance characteristics of GitOperationsService
//! when working with repositories containing thousands of commits and files.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use prodigy::cook::execution::mapreduce::resources::git_operations::{
    GitOperationsConfig, GitOperationsService,
};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

/// Create a test repository with specified number of commits and files
fn create_large_test_repo(commits: usize, files_per_commit: usize) -> (TempDir, git2::Repository) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo = git2::Repository::init(temp_dir.path()).expect("Failed to init repository");

    // Configure git user for commits
    let mut config = repo.config().expect("Failed to get config");
    config
        .set_str("user.name", "Benchmark User")
        .expect("Failed to set user name");
    config
        .set_str("user.email", "bench@example.com")
        .expect("Failed to set user email");

    let sig = git2::Signature::now("Benchmark User", "bench@example.com")
        .expect("Failed to create signature");

    // Create initial commit to establish HEAD
    let initial_file = temp_dir.path().join("README.md");
    fs::write(&initial_file, "# Benchmark Repository").expect("Failed to write README");

    let mut index = repo.index().expect("Failed to get index");
    index
        .add_path(Path::new("README.md"))
        .expect("Failed to add README to index");
    index.write().expect("Failed to write index");

    let tree_id = index.write_tree().expect("Failed to write tree");
    let tree = repo.find_tree(tree_id).expect("Failed to find tree");

    repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
        .expect("Failed to create initial commit");

    // Create specified number of commits with files
    for commit_idx in 0..commits {
        for file_idx in 0..files_per_commit {
            let filename = format!("file_{:04}_{:03}.txt", commit_idx, file_idx);
            let filepath = temp_dir.path().join(&filename);
            let content = format!("Content for commit {} file {}", commit_idx, file_idx);
            fs::write(&filepath, content).expect("Failed to write file");
        }

        let mut index = repo.index().expect("Failed to get index");
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .expect("Failed to add files to index");
        index.write().expect("Failed to write index");

        let tree_id = index.write_tree().expect("Failed to write tree");
        let tree = repo.find_tree(tree_id).expect("Failed to find tree");

        let head = repo.head().expect("Failed to get HEAD");
        let parent_commit = head.peel_to_commit().expect("Failed to get parent commit");

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            &format!("Commit #{}", commit_idx),
            &tree,
            &[&parent_commit],
        )
        .expect("Failed to create commit");
    }

    (temp_dir, repo)
}

fn bench_get_commits_small_repo(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (temp_dir, _repo) = create_large_test_repo(10, 5);

    c.bench_function("get_commits_small_10", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig::default();
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });
}

fn bench_get_commits_medium_repo(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (temp_dir, _repo) = create_large_test_repo(100, 10);

    c.bench_function("get_commits_medium_100", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig::default();
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });
}

fn bench_get_commits_large_repo(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (temp_dir, _repo) = create_large_test_repo(500, 5);

    c.bench_function("get_commits_large_500", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig {
                max_commits: 500,
                ..Default::default()
            };
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });
}

fn bench_get_modified_files(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("get_modified_files");

    for size in [10, 50, 100, 500].iter() {
        let (temp_dir, _repo) = create_large_test_repo(*size / 10, 10);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _size| {
            b.to_async(&runtime).iter(|| async {
                let config = GitOperationsConfig::default();
                let mut service = GitOperationsService::new(config);
                let files = service
                    .get_worktree_modified_files(temp_dir.path(), None)
                    .await
                    .expect("Failed to get modified files");
                black_box(files);
            });
        });
    }

    group.finish();
}

fn bench_get_merge_info(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("get_merge_git_info");

    for commits in [10, 50, 100].iter() {
        let (temp_dir, _repo) = create_large_test_repo(*commits, 10);

        group.bench_with_input(
            BenchmarkId::from_parameter(commits),
            commits,
            |b, _commits| {
                b.to_async(&runtime).iter(|| async {
                    let config = GitOperationsConfig {
                        max_commits: 100,
                        max_files: 500,
                        ..Default::default()
                    };
                    let mut service = GitOperationsService::new(config);
                    let merge_info = service
                        .get_merge_git_info(temp_dir.path(), "main")
                        .await
                        .expect("Failed to get merge info");
                    black_box(merge_info);
                });
            },
        );
    }

    group.finish();
}

fn bench_config_variations(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (temp_dir, _repo) = create_large_test_repo(100, 10);

    let mut group = c.benchmark_group("config_variations");

    // Benchmark with different configurations
    group.bench_function("default_config", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig::default();
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });

    group.bench_function("limited_config", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig {
                max_commits: 50,
                max_files: 100,
                include_diffs: false,
                ..Default::default()
            };
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });

    group.bench_function("no_cache_config", |b| {
        b.to_async(&runtime).iter(|| async {
            let config = GitOperationsConfig {
                enable_caching: false,
                ..Default::default()
            };
            let mut service = GitOperationsService::new(config);
            let commits = service
                .get_worktree_commits(temp_dir.path(), None, None)
                .await
                .expect("Failed to get commits");
            black_box(commits);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_get_commits_small_repo,
    bench_get_commits_medium_repo,
    bench_get_commits_large_repo,
    bench_get_modified_files,
    bench_get_merge_info,
    bench_config_variations
);

criterion_main!(benches);
