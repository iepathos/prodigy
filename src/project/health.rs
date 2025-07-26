use super::Project;
use anyhow::Result;
use std::path::Path;
use tokio::fs;

#[derive(Debug, Clone)]
pub struct ProjectHealth {
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Passing,
    Warning,
    Failing,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Critical, // Blocks execution
    Major,    // Degrades functionality
    Minor,    // Cosmetic issues
}

impl ProjectHealth {
    pub async fn check(project: &Project) -> Result<Self> {
        let mut checks = Vec::new();

        // Check configuration file validity
        checks.push(check_config_file(&project.path).await);

        // Check specification directory
        checks.push(check_spec_directory(&project.path).await);

        // Check state database integrity
        checks.push(check_state_database(&project.path).await);

        // Check file permissions
        checks.push(check_permissions(&project.path).await);

        // Check mmm directory structure
        checks.push(check_mmm_structure(&project.path).await);

        Ok(Self { checks })
    }

    pub fn is_healthy(&self) -> bool {
        self.checks
            .iter()
            .all(|c| c.status != HealthStatus::Failing || c.severity != Severity::Critical)
    }

    pub fn has_warnings(&self) -> bool {
        self.checks
            .iter()
            .any(|c| c.status == HealthStatus::Warning)
    }
}

async fn check_config_file(project_path: &Path) -> HealthCheck {
    let config_path = project_path.join(".mmm").join("config.toml");

    match fs::read_to_string(&config_path).await {
        Ok(content) => match toml::from_str::<toml::Value>(&content) {
            Ok(_) => HealthCheck {
                name: "Configuration file".to_string(),
                status: HealthStatus::Passing,
                message: Some("Valid TOML configuration".to_string()),
                severity: Severity::Critical,
            },
            Err(e) => HealthCheck {
                name: "Configuration file".to_string(),
                status: HealthStatus::Failing,
                message: Some(format!("Invalid TOML: {e}")),
                severity: Severity::Critical,
            },
        },
        Err(_) => HealthCheck {
            name: "Configuration file".to_string(),
            status: HealthStatus::Warning,
            message: Some("Configuration file not found".to_string()),
            severity: Severity::Major,
        },
    }
}

async fn check_spec_directory(project_path: &Path) -> HealthCheck {
    let spec_dir = project_path.join("specs");

    match fs::metadata(&spec_dir).await {
        Ok(metadata) if metadata.is_dir() => match fs::read_dir(&spec_dir).await {
            Ok(mut entries) => {
                let mut spec_count = 0;
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.path().extension().and_then(|s| s.to_str()) == Some("md") {
                        spec_count += 1;
                    }
                }

                HealthCheck {
                    name: "Specification directory".to_string(),
                    status: if spec_count > 0 {
                        HealthStatus::Passing
                    } else {
                        HealthStatus::Warning
                    },
                    message: Some(format!("Found {spec_count} specifications")),
                    severity: Severity::Major,
                }
            }
            Err(e) => HealthCheck {
                name: "Specification directory".to_string(),
                status: HealthStatus::Warning,
                message: Some(format!("Cannot read directory: {e}")),
                severity: Severity::Major,
            },
        },
        _ => HealthCheck {
            name: "Specification directory".to_string(),
            status: HealthStatus::Warning,
            message: Some("Specification directory not found".to_string()),
            severity: Severity::Major,
        },
    }
}

async fn check_state_database(project_path: &Path) -> HealthCheck {
    let db_path = project_path.join(".mmm").join("state.db");

    match fs::metadata(&db_path).await {
        Ok(metadata) => {
            let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

            let (status, message) = if size_mb > 100.0 {
                (
                    HealthStatus::Warning,
                    format!("Database is large ({size_mb:.1} MB)"),
                )
            } else {
                (
                    HealthStatus::Passing,
                    format!("Database size: {size_mb:.1} MB"),
                )
            };

            HealthCheck {
                name: "State database".to_string(),
                status,
                message: Some(message),
                severity: Severity::Major,
            }
        }
        Err(_) => HealthCheck {
            name: "State database".to_string(),
            status: HealthStatus::Warning,
            message: Some("Database not found (will be created on first use)".to_string()),
            severity: Severity::Minor,
        },
    }
}

async fn check_permissions(project_path: &Path) -> HealthCheck {
    let mmm_dir = project_path.join(".mmm");

    match fs::metadata(&mmm_dir).await {
        Ok(metadata) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = metadata.permissions().mode();
                let is_writable = mode & 0o200 != 0;

                if is_writable {
                    HealthCheck {
                        name: "File permissions".to_string(),
                        status: HealthStatus::Passing,
                        message: Some("Directory is writable".to_string()),
                        severity: Severity::Critical,
                    }
                } else {
                    HealthCheck {
                        name: "File permissions".to_string(),
                        status: HealthStatus::Failing,
                        message: Some("Directory is not writable".to_string()),
                        severity: Severity::Critical,
                    }
                }
            }

            #[cfg(not(unix))]
            {
                HealthCheck {
                    name: "File permissions".to_string(),
                    status: HealthStatus::Passing,
                    message: Some("Permission check not available on this platform".to_string()),
                    severity: Severity::Minor,
                }
            }
        }
        Err(_) => HealthCheck {
            name: "File permissions".to_string(),
            status: HealthStatus::Warning,
            message: Some(".mmm directory not found".to_string()),
            severity: Severity::Major,
        },
    }
}

async fn check_mmm_structure(project_path: &Path) -> HealthCheck {
    let mmm_dir = project_path.join(".mmm");
    let expected_files = vec!["config.toml"];
    let expected_dirs = vec!["cache", "logs"];

    let mut missing_files = Vec::new();
    let mut missing_dirs = Vec::new();

    for file in expected_files {
        if !mmm_dir.join(file).exists() {
            missing_files.push(file);
        }
    }

    for dir in expected_dirs {
        if !mmm_dir.join(dir).exists() {
            missing_dirs.push(dir);
        }
    }

    if missing_files.is_empty() && missing_dirs.is_empty() {
        HealthCheck {
            name: "Project structure".to_string(),
            status: HealthStatus::Passing,
            message: Some("All expected files and directories present".to_string()),
            severity: Severity::Minor,
        }
    } else {
        let mut message_parts = Vec::new();
        if !missing_files.is_empty() {
            message_parts.push(format!("Missing files: {}", missing_files.join(", ")));
        }
        if !missing_dirs.is_empty() {
            message_parts.push(format!("Missing directories: {}", missing_dirs.join(", ")));
        }

        HealthCheck {
            name: "Project structure".to_string(),
            status: HealthStatus::Warning,
            message: Some(message_parts.join("; ")),
            severity: Severity::Minor,
        }
    }
}
