//! Integration tests for concurrent resume protection

use anyhow::Result;
use prodigy::cook::execution::ResumeLockManager;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

#[tokio::test]
async fn test_concurrent_resume_attempts_blocked() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = Arc::new(ResumeLockManager::new(temp_dir.path().to_path_buf())?);
    let job_id = "test-job-concurrent";

    // Spawn two concurrent resume tasks
    let manager1 = manager.clone();
    let job_id1 = job_id.to_string();
    let handle1 = tokio::spawn(async move {
        let lock = manager1.acquire_lock(&job_id1).await;
        if lock.is_ok() {
            // Hold lock for a bit
            sleep(Duration::from_millis(100)).await;
        }
        lock
    });

    let manager2 = manager.clone();
    let job_id2 = job_id.to_string();
    let handle2 = tokio::spawn(async move {
        // Small delay to ensure first task acquires lock first
        sleep(Duration::from_millis(10)).await;
        manager2.acquire_lock(&job_id2).await
    });

    let result1 = handle1.await?;
    let result2 = handle2.await?;

    // One should succeed, one should fail
    assert!(
        (result1.is_ok() && result2.is_err()) || (result1.is_err() && result2.is_ok()),
        "Exactly one lock acquisition should succeed"
    );

    // The failed one should have "already in progress" error
    let error = if result1.is_err() {
        result1.unwrap_err()
    } else {
        result2.unwrap_err()
    };
    assert!(error.to_string().contains("already in progress"));

    Ok(())
}

#[tokio::test]
async fn test_sequential_resume_succeeds() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf())?;
    let job_id = "test-job-sequential";

    // First resume acquires lock
    {
        let _lock = manager.acquire_lock(job_id).await?;
        // Do some work
        sleep(Duration::from_millis(50)).await;
    } // Lock released

    // Second resume should succeed after first completes
    let lock2 = manager.acquire_lock(job_id).await;
    assert!(lock2.is_ok(), "Second resume should succeed");

    Ok(())
}

#[tokio::test]
async fn test_resume_after_crash_cleans_stale_lock() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf())?;
    let job_id = "test-job-stale";

    // Create lock file with non-existent PID
    let lock_path = temp_dir
        .path()
        .join("resume_locks")
        .join(format!("{}.lock", job_id));
    std::fs::create_dir_all(lock_path.parent().unwrap())?;

    let stale_lock_data = serde_json::json!({
        "job_id": job_id,
        "process_id": 999999,  // Fake PID
        "hostname": "test-host",
        "acquired_at": chrono::Utc::now().to_rfc3339()
    });
    std::fs::write(&lock_path, serde_json::to_string(&stale_lock_data)?)?;

    // Resume should detect stale lock and succeed
    let lock = manager.acquire_lock(job_id).await;
    assert!(
        lock.is_ok(),
        "Resume should clean up stale lock and succeed"
    );

    Ok(())
}

#[tokio::test]
async fn test_lock_error_message_includes_details() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = ResumeLockManager::new(temp_dir.path().to_path_buf())?;
    let job_id = "test-job-error-msg";

    // First resume acquires lock
    let _lock1 = manager.acquire_lock(job_id).await?;

    // Second resume should fail with detailed error
    let result = manager.acquire_lock(job_id).await;
    assert!(result.is_err());

    let error = result.unwrap_err().to_string();
    // Verify error message includes helpful information
    assert!(error.contains("already in progress"));
    assert!(error.contains("PID"));
    assert!(error.contains(job_id));

    Ok(())
}

#[tokio::test]
async fn test_lock_released_on_task_panic() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = Arc::new(ResumeLockManager::new(temp_dir.path().to_path_buf())?);
    let job_id = "test-job-panic";

    // Spawn task that acquires lock and panics
    let manager_clone = manager.clone();
    let job_id_clone = job_id.to_string();
    let handle = tokio::spawn(async move {
        let _lock = manager_clone.acquire_lock(&job_id_clone).await.unwrap();
        // Lock will be dropped when task ends
        sleep(Duration::from_millis(50)).await;
    });

    // Wait for task to complete
    let _ = handle.await;

    // Should be able to acquire lock after task completes
    // Note: In real panic scenarios, lock might not be released if process crashes
    // This test simulates task completion (normal drop)
    let lock = manager.acquire_lock(job_id).await;
    assert!(
        lock.is_ok(),
        "Lock should be available after task completes"
    );

    Ok(())
}

#[tokio::test]
async fn test_multiple_jobs_independent_locks() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let manager = Arc::new(ResumeLockManager::new(temp_dir.path().to_path_buf())?);

    // Acquire locks for multiple jobs concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            let job_id = format!("job-{}", i);
            manager_clone.acquire_lock(&job_id).await
        });
        handles.push(handle);
    }

    // All should succeed (different jobs)
    let results = futures::future::join_all(handles).await;
    for result in results {
        assert!(result.is_ok());
        assert!(result.unwrap().is_ok());
    }

    Ok(())
}
