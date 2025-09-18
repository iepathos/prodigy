//! Comprehensive tests for the resource management module

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::cook::orchestrator::ExecutionEnvironment;
    use crate::subprocess::{MockProcessRunner, ProcessRunner};
    use crate::worktree::{WorktreeManager, WorktreePool, WorktreeSession};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;

    /// Create a mock execution environment for testing
    fn create_test_env() -> ExecutionEnvironment {
        ExecutionEnvironment {
            working_dir: std::path::PathBuf::from("/tmp/test_project"),
            project_dir: std::path::PathBuf::from("/tmp/test_project"),
            worktree_name: Some("test-worktree".to_string()),
            session_id: "test-session-123".to_string(),
        }
    }

    #[tokio::test]
    async fn test_resource_manager_creation() {
        let manager = ResourceManager::new(None);

        // Verify all components are initialized
        assert!(manager.worktree_pool.is_none());
        assert!(manager.active_sessions.read().await.is_empty());
        assert_eq!(manager.get_metrics().await.active_sessions, 0);
    }

    #[tokio::test]
    async fn test_resource_manager_with_worktree_pool() {
        let mock_runner = MockProcessRunner::new();
        let subprocess = crate::subprocess::SubprocessManager::new(Arc::new(mock_runner) as Arc<dyn ProcessRunner>);
        let worktree_manager = WorktreeManager::new(std::path::PathBuf::from("/tmp"), subprocess)
            .ok()
            .map(Arc::new);
        let config = Default::default();
        let pool = worktree_manager
            .clone()
            .map(|manager| Arc::new(WorktreePool::new(config, manager)));

        let manager = ResourceManager::new(pool);

        assert!(manager.worktree_pool.is_some());
        assert_eq!(manager.get_metrics().await.active_sessions, 0);
    }

    #[tokio::test]
    async fn test_session_registration_and_unregistration() {
        let manager = ResourceManager::new(None);

        // Create a mock worktree session
        let session = WorktreeSession {
            name: "test-worktree".to_string(),
            branch: "test-branch".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            created_at: chrono::Utc::now(),
        };

        // Register session
        manager.register_session("agent-1".to_string(), session.clone()).await;

        // Verify registration
        let active = manager.get_active_sessions().await;
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].0, "agent-1");

        // Check metrics
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.active_sessions, 1);

        // Unregister session
        let unregistered = manager.unregister_session("agent-1").await;
        assert!(unregistered.is_some());

        // Verify unregistration
        let active = manager.get_active_sessions().await;
        assert_eq!(active.len(), 0);

        // Check metrics after unregistration
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.active_sessions, 0);
    }

    #[tokio::test]
    async fn test_cleanup_orphaned_resources() {
        let manager = ResourceManager::new(None);

        // Test with empty list
        manager.cleanup_orphaned_resources(&[]).await;

        // Test with worktree names
        let worktree_names = vec![
            "worktree-1".to_string(),
            "worktree-2".to_string(),
        ];
        manager.cleanup_orphaned_resources(&worktree_names).await;

        // Verify cleanup tasks were registered
        // Note: actual cleanup would require a real worktree manager
    }

    #[tokio::test]
    async fn test_cleanup_all_resources() {
        let manager = ResourceManager::new(None);

        // Register multiple sessions
        for i in 0..3 {
            let session = WorktreeSession {
                name: format!("test-worktree-{}", i),
                branch: "test-branch".to_string(),
                path: std::path::PathBuf::from(format!("/tmp/test-{}", i)),
                created_at: chrono::Utc::now(),
            };
            manager.register_session(format!("agent-{}", i), session).await;
        }

        // Verify sessions are registered
        assert_eq!(manager.get_active_sessions().await.len(), 3);

        // Cleanup all resources
        let result = manager.cleanup_all().await;
        assert!(result.is_ok());

        // Verify all sessions are cleared
        assert_eq!(manager.get_active_sessions().await.len(), 0);
    }

    #[tokio::test]
    async fn test_resource_metrics_tracking() {
        let manager = ResourceManager::new(None);

        // Initial metrics should be zero
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.active_sessions, 0);
        assert_eq!(metrics.total_created, 0);
        assert_eq!(metrics.total_reused, 0);

        // Register sessions and check metrics
        for i in 0..5 {
            let session = WorktreeSession {
                name: format!("test-worktree-{}", i),
                branch: "test-branch".to_string(),
                path: std::path::PathBuf::from(format!("/tmp/test-{}", i)),
                created_at: chrono::Utc::now(),
            };
            manager.register_session(format!("agent-{}", i), session).await;
        }

        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.active_sessions, 5);
    }

    #[tokio::test]
    async fn test_resource_guard_cleanup() {
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();

        {
            let _guard = ResourceGuard::new(42, move |_value| {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            });

            // Guard is in scope, cleanup not yet called
            assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);
        }

        // Guard dropped, cleanup should have been called
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_resource_guard_take() {
        let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let guard = ResourceGuard::new(42, move |_value| {
            counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        });

        // Take the resource, preventing cleanup
        let value = guard.take();
        assert_eq!(value, Some(42));

        // Cleanup should not have been called
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_agent_resource_manager() {
        let agent_manager = AgentResourceManager::new();

        // Create test context
        let context = AgentContext::new(
            "agent-1".to_string(),
            json!({"test": "data"}),
            0,
            WorktreeSession {
                name: "test-worktree".to_string(),
                branch: "test-branch".to_string(),
                path: std::path::PathBuf::from("/tmp/test"),
                created_at: chrono::Utc::now(),
            },
            Default::default(),
        );

        // Register context
        agent_manager.register_context("agent-1".to_string(), context.clone()).await;

        // Verify registration
        let retrieved = agent_manager.get_context("agent-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_id, "agent-1");

        // Get all contexts
        let all_contexts = agent_manager.get_active_contexts().await;
        assert_eq!(all_contexts.len(), 1);

        // Unregister context
        let unregistered = agent_manager.unregister_context("agent-1").await;
        assert!(unregistered.is_some());

        // Verify unregistration
        let retrieved = agent_manager.get_context("agent-1").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_agent_context_initialization() {
        let agent_manager = AgentResourceManager::new();
        let session = WorktreeSession {
            name: "test-worktree".to_string(),
            branch: "test-branch".to_string(),
            path: std::path::PathBuf::from("/tmp/test"),
            created_at: chrono::Utc::now(),
        };

        let item = json!({"key": "value", "number": 42});
        let context = agent_manager.initialize_agent_context(
            "agent-test",
            &item,
            5,
            &session,
            "correlation-123",
        );

        // Verify context variables
        assert_eq!(context.get("agent_id").unwrap(), &json!("agent-test"));
        assert_eq!(context.get("item").unwrap(), &item);
        assert_eq!(context.get("item_index").unwrap(), &json!(5));

        // Verify MapReduce context
        let map_context = context.get("map").unwrap();
        assert!(map_context.is_object());
        assert_eq!(map_context["job_id"], json!("correlation-123"));
        assert_eq!(map_context["agent"]["id"], json!("agent-test"));
        assert_eq!(map_context["agent"]["index"], json!(5));
    }

    #[tokio::test]
    async fn test_cleanup_registry_operations() {
        let registry = CleanupRegistry::new();

        // Create test cleanup task
        struct TestCleanupTask {
            executed: Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait::async_trait]
        impl CleanupTask for TestCleanupTask {
            async fn cleanup(&self) -> MapReduceResult<()> {
                self.executed.store(true, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            }

            fn priority(&self) -> CleanupPriority {
                CleanupPriority::Normal
            }
        }

        let executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let task = Box::new(TestCleanupTask {
            executed: executed.clone(),
        });

        // Register task
        registry.register(task).await;

        // Execute all tasks
        let result = registry.execute_all().await;
        assert!(result.is_ok());

        // Verify task was executed
        assert!(executed.load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    async fn test_resource_manager_stress() {
        let manager = Arc::new(ResourceManager::new(None));
        let mut handles = vec![];

        // Spawn multiple tasks that register/unregister sessions concurrently
        for i in 0..10 {
            let manager_clone = manager.clone();
            let handle = tokio::spawn(async move {
                for j in 0..10 {
                    let session = WorktreeSession {
                        name: format!("test-worktree-{}-{}", i, j),
                        branch: "test-branch".to_string(),
                        path: std::path::PathBuf::from(format!("/tmp/test-{}-{}", i, j)),
                        created_at: chrono::Utc::now(),
                    };
                    let agent_id = format!("agent-{}-{}", i, j);

                    // Register
                    manager_clone.register_session(agent_id.clone(), session).await;

                    // Small delay
                    tokio::time::sleep(Duration::from_millis(1)).await;

                    // Unregister
                    manager_clone.unregister_session(&agent_id).await;
                }
            });
            handles.push(handle);
        }

        // Wait for all tasks with timeout
        for handle in handles {
            let _ = timeout(Duration::from_secs(5), handle).await;
        }

        // Verify all sessions are cleaned up
        assert_eq!(manager.get_active_sessions().await.len(), 0);
    }

    #[tokio::test]
    async fn test_worktree_error_creation() {
        let agent_manager = AgentResourceManager::new();

        let error = agent_manager.create_worktree_error(
            "test-agent",
            "Failed to create worktree".to_string(),
            "correlation-789",
        );

        match error {
            crate::cook::execution::errors::MapReduceError::WorktreeCreationFailed { agent_id, reason, .. } => {
                assert_eq!(agent_id, "test-agent");
                assert_eq!(reason, "Failed to create worktree");
            }
            _ => panic!("Expected WorktreeCreationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_cleanup_task_priority_ordering() {
        let registry = CleanupRegistry::new();
        let execution_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        // Create tasks with different priorities
        struct PriorityTestTask {
            name: String,
            priority: CleanupPriority,
            execution_order: Arc<std::sync::Mutex<Vec<String>>>,
        }

        #[async_trait::async_trait]
        impl CleanupTask for PriorityTestTask {
            async fn cleanup(&self) -> MapReduceResult<()> {
                let mut order = self.execution_order.lock().unwrap();
                order.push(self.name.clone());
                Ok(())
            }

            fn priority(&self) -> CleanupPriority {
                self.priority.clone()
            }
        }

        // Register tasks in mixed order
        let tasks = vec![
            ("normal", CleanupPriority::Normal),
            ("high", CleanupPriority::High),
            ("low", CleanupPriority::Low),
            ("critical", CleanupPriority::Critical),
        ];

        for (name, priority) in tasks {
            let task = Box::new(PriorityTestTask {
                name: name.to_string(),
                priority,
                execution_order: execution_order.clone(),
            });
            registry.register(task).await;
        }

        // Execute all tasks
        registry.execute_all().await.unwrap();

        // Verify execution order (high priority first)
        let order = execution_order.lock().unwrap();
        assert_eq!(order[0], "critical");
        assert_eq!(order[1], "high");
        assert_eq!(order[2], "normal");
        assert_eq!(order[3], "low");
    }
}