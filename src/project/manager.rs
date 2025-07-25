use super::{get_global_mmm_dir, Project, ProjectMetadata};
use crate::{Error, Result};
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
            return Err(Error::Project(format!("Project '{}' already exists", name)));
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
        let content = toml::to_string_pretty(&project)
            .map_err(|e| Error::Config(e.to_string()))?;
        
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
        
        Ok(())
    }
    
    pub fn switch_project(&mut self, name: &str) -> Result<()> {
        if !self.projects.contains_key(name) {
            return Err(Error::Project(format!("Project '{}' not found", name)));
        }
        
        self.current_project = Some(name.to_string());
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
            return Err(Error::Project(format!("Project '{}' not found", name)));
        }
        
        let registry_path = self.global_dir.join("projects").join(format!("{}.toml", name));
        if registry_path.exists() {
            fs::remove_file(&registry_path).await?;
        }
        
        self.projects.remove(name);
        if self.current_project.as_ref() == Some(&name.to_string()) {
            self.current_project = None;
        }
        
        Ok(())
    }
}