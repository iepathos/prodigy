//! Test fixture for integrated checkpoint and resume testing

use anyhow::Result;
use std::path::PathBuf;
use tempfile::TempDir;

/// Comprehensive test fixture for checkpoint/resume tests
pub struct TestFixture {
    /// Temporary directory (automatically cleaned up on drop)
    pub temp_dir: TempDir,
    /// Path to the workflow file
    pub workflow_path: Option<PathBuf>,
    /// Path to the checkpoint directory
    pub checkpoint_dir: PathBuf,
    /// Session ID for testing
    pub session_id: String,
}

impl TestFixture {
    /// Create a new test fixture with default setup
    pub async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        tokio::fs::create_dir_all(&checkpoint_dir).await?;

        let session_id = format!("test-session-{}", uuid::Uuid::new_v4());

        Ok(Self {
            temp_dir,
            workflow_path: None,
            checkpoint_dir,
            session_id,
        })
    }

    /// Create a test fixture with a specific session ID
    pub async fn with_session_id(session_id: impl Into<String>) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let checkpoint_dir = temp_dir.path().join("checkpoints");
        tokio::fs::create_dir_all(&checkpoint_dir).await?;

        Ok(Self {
            temp_dir,
            workflow_path: None,
            checkpoint_dir,
            session_id: session_id.into(),
        })
    }

    /// Get the temporary directory path
    pub fn temp_path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }

    /// Set the workflow path
    pub fn set_workflow_path(&mut self, path: PathBuf) {
        self.workflow_path = Some(path);
    }

    /// Get the checkpoint file path for this session
    pub fn checkpoint_path(&self) -> PathBuf {
        self.checkpoint_dir
            .join(format!("{}.json", self.session_id))
    }

    /// Create a simple workflow file
    pub async fn create_simple_workflow(&mut self) -> Result<PathBuf> {
        let path = super::workflow_helpers::create_simple_workflow(self.temp_path()).await?;
        self.workflow_path = Some(path.clone());
        Ok(path)
    }

    /// Create a failing workflow
    pub async fn create_failing_workflow(&mut self, fail_at_step: usize) -> Result<PathBuf> {
        let path =
            super::workflow_helpers::create_failing_workflow(self.temp_path(), fail_at_step)
                .await?;
        self.workflow_path = Some(path.clone());
        Ok(path)
    }

    /// Create a MapReduce workflow
    pub async fn create_mapreduce_workflow(
        &mut self,
        num_items: usize,
    ) -> Result<(PathBuf, PathBuf)> {
        let (workflow_path, items_path) =
            super::workflow_helpers::create_mapreduce_workflow(self.temp_path(), num_items)
                .await?;
        self.workflow_path = Some(workflow_path.clone());
        Ok((workflow_path, items_path))
    }

    /// Corrupt the checkpoint file
    pub async fn corrupt_checkpoint(&self) -> Result<()> {
        super::checkpoint_helpers::corrupt_checkpoint_file(&self.checkpoint_path()).await
    }

    /// Remove the workflow file
    pub async fn remove_workflow_file(&self) -> Result<()> {
        if let Some(ref path) = self.workflow_path {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// Modify the workflow file to change its hash
    pub async fn modify_workflow(&self) -> Result<()> {
        if let Some(ref path) = self.workflow_path {
            super::workflow_helpers::modify_workflow_file(path).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fixture_creation() -> Result<()> {
        let fixture = TestFixture::new().await?;

        assert!(fixture.temp_path().exists());
        assert!(fixture.checkpoint_dir.exists());
        assert!(!fixture.session_id.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_fixture_with_workflow() -> Result<()> {
        let mut fixture = TestFixture::new().await?;
        let workflow_path = fixture.create_simple_workflow().await?;

        assert!(workflow_path.exists());
        assert_eq!(fixture.workflow_path, Some(workflow_path));

        Ok(())
    }
}
