use super::{get_global_mmm_dir, Project};
use crate::{Error, Result};
use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

pub struct ProjectManager {
    global_dir: PathBuf,
    projects: HashMap<String, Project>,
    current_project: Option<String>,
}

impl ProjectManager {
    pub async fn new() -> Result<Self> {
        let global_dir = get_global_mmm_dir()?;
        fs::create_dir_all(&global_dir).await?;

        let mut manager = Self {
            global_dir,
            projects: HashMap::new(),
            current_project: None,
        };

        manager.load_projects().await?;
        Ok(manager)
    }

    pub async fn create_project(&mut self, name: &str, path: &Path) -> Result<&Project> {
        if self.projects.contains_key(name) {
            return Err(Error::Project(format!("Project '{name}' already exists")));
        }

        let project = Project::new(name.to_string(), path.to_path_buf());
        project.init_structure().await?;

        self.register_project(project).await?;
        Ok(self.projects.get(name).unwrap())
    }

    pub async fn register_project(&mut self, project: Project) -> Result<()> {
        let registry_path = self.global_dir.join("projects");
        fs::create_dir_all(&registry_path).await?;

        let project_file = registry_path.join(format!("{}.toml", project.name));
        let content = toml::to_string_pretty(&project).map_err(|e| Error::Config(e.to_string()))?;

        fs::write(&project_file, content).await?;
        self.projects.insert(project.name.clone(), project);

        Ok(())
    }

    pub async fn load_projects(&mut self) -> Result<()> {
        let registry_path = self.global_dir.join("projects");
        if !registry_path.exists() {
            return Ok(());
        }

        let mut entries = fs::read_dir(&registry_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                let content = fs::read_to_string(&path).await?;
                if let Ok(project) = toml::from_str::<Project>(&content) {
                    self.projects.insert(project.name.clone(), project);
                }
            }
        }

        // Load current project from state file
        let state_file = self.global_dir.join("current_project");
        if state_file.exists() {
            if let Ok(current) = fs::read_to_string(&state_file).await {
                self.current_project = Some(current.trim().to_string());
            }
        }

        Ok(())
    }

    pub async fn switch_project(&mut self, name: &str) -> Result<()> {
        if !self.projects.contains_key(name) {
            return Err(Error::Project(format!("Project '{name}' not found")));
        }

        // Update last accessed time
        if let Some(project) = self.projects.get_mut(name) {
            project.last_accessed = Utc::now();
        }

        // Save project after updating
        if let Some(project) = self.projects.get(name) {
            self.save_project(project).await?;
        }

        self.current_project = Some(name.to_string());

        // Save current project state
        let state_file = self.global_dir.join("current_project");
        fs::write(&state_file, name).await?;

        Ok(())
    }

    pub fn current_project(&self) -> Option<&Project> {
        self.current_project
            .as_ref()
            .and_then(|name| self.projects.get(name))
    }

    pub fn list_projects(&self) -> Vec<&Project> {
        self.projects.values().collect()
    }

    pub async fn remove_project(&mut self, name: &str) -> Result<()> {
        if !self.projects.contains_key(name) {
            return Err(Error::Project(format!("Project '{name}' not found")));
        }

        let registry_path = self
            .global_dir
            .join("projects")
            .join(format!("{name}.toml"));
        if registry_path.exists() {
            fs::remove_file(&registry_path).await?;
        }

        self.projects.remove(name);
        if self.current_project.as_ref() == Some(&name.to_string()) {
            self.current_project = None;
        }

        Ok(())
    }

    pub fn get_project(&self, name: &str) -> Result<&Project> {
        self.projects
            .get(name)
            .ok_or_else(|| Error::Project(format!("Project '{name}' not found")))
    }

    pub async fn clone_project(&mut self, source: &str, destination: &str) -> Result<()> {
        let source_project = self.get_project(source)?.clone();

        let mut new_project = source_project;
        new_project.name = destination.to_string();
        new_project.created = Utc::now();
        new_project.last_accessed = Utc::now();

        // TODO: Clone project files if needed

        self.register_project(new_project).await?;
        Ok(())
    }

    pub async fn archive_project(&mut self, name: &str) -> Result<()> {
        if let Some(project) = self.projects.get_mut(name) {
            project.archived = true;
        } else {
            return Err(Error::Project(format!("Project '{name}' not found")));
        }

        if let Some(project) = self.projects.get(name) {
            self.save_project(project).await?;
        }
        Ok(())
    }

    pub async fn unarchive_project(&mut self, name: &str) -> Result<()> {
        if let Some(project) = self.projects.get_mut(name) {
            project.archived = false;
        } else {
            return Err(Error::Project(format!("Project '{name}' not found")));
        }

        if let Some(project) = self.projects.get(name) {
            self.save_project(project).await?;
        }
        Ok(())
    }

    pub async fn delete_project(&mut self, name: &str) -> Result<()> {
        self.remove_project(name).await
    }

    async fn save_project(&self, project: &Project) -> Result<()> {
        let registry_path = self.global_dir.join("projects");
        fs::create_dir_all(&registry_path).await?;

        let project_file = registry_path.join(format!("{}.toml", project.name));
        let content = toml::to_string_pretty(project).map_err(|e| Error::Config(e.to_string()))?;

        fs::write(&project_file, content).await?;
        Ok(())
    }
}
