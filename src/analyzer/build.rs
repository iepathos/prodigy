//! Build tool detection and analysis

use anyhow::Result;
use std::collections::HashMap;

use super::structure::ProjectStructure;

/// Dependency information
#[derive(Debug, Clone)]
pub struct Dependency {
    pub name: String,
    pub version: String,
}

/// Build tools
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildTool {
    Cargo,   // Rust
    Npm,     // JavaScript
    Yarn,    // JavaScript
    Pnpm,    // JavaScript
    Poetry,  // Python
    Pip,     // Python
    Maven,   // Java
    Gradle,  // Java/Kotlin
    Dotnet,  // C#
    Go,      // Go
    Bundler, // Ruby
    SwiftPM, // Swift
}

impl std::fmt::Display for BuildTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildTool::Cargo => write!(f, "Cargo"),
            BuildTool::Npm => write!(f, "npm"),
            BuildTool::Yarn => write!(f, "Yarn"),
            BuildTool::Pnpm => write!(f, "pnpm"),
            BuildTool::Poetry => write!(f, "Poetry"),
            BuildTool::Pip => write!(f, "pip"),
            BuildTool::Maven => write!(f, "Maven"),
            BuildTool::Gradle => write!(f, "Gradle"),
            BuildTool::Dotnet => write!(f, ".NET"),
            BuildTool::Go => write!(f, "Go"),
            BuildTool::Bundler => write!(f, "Bundler"),
            BuildTool::SwiftPM => write!(f, "Swift Package Manager"),
        }
    }
}

/// Build information
#[derive(Debug, Clone)]
pub struct BuildInfo {
    pub tool: BuildTool,
    pub scripts: HashMap<String, String>,
    pub dependencies: Vec<Dependency>,
    pub dev_dependencies: Vec<Dependency>,
}

/// Build analyzer
pub struct BuildAnalyzer;

impl BuildAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub async fn analyze(&self, structure: &ProjectStructure) -> Result<Option<BuildInfo>> {
        // Detect build tool
        let tool = match self.detect_build_tool(structure).await? {
            Some(tool) => tool,
            None => return Ok(None),
        };

        // Analyze based on tool
        let build_info = match tool {
            BuildTool::Cargo => self.analyze_cargo(structure).await?,
            BuildTool::Npm | BuildTool::Yarn | BuildTool::Pnpm => {
                self.analyze_node(structure, tool).await?
            }
            BuildTool::Poetry | BuildTool::Pip => self.analyze_python(structure, tool).await?,
            BuildTool::Maven => self.analyze_maven(structure).await?,
            BuildTool::Gradle => self.analyze_gradle(structure).await?,
            BuildTool::Go => self.analyze_go(structure).await?,
            BuildTool::Dotnet => self.analyze_dotnet(structure).await?,
            BuildTool::Bundler => self.analyze_bundler(structure).await?,
            BuildTool::SwiftPM => self.analyze_swift(structure).await?,
        };

        Ok(Some(build_info))
    }

    async fn detect_build_tool(&self, structure: &ProjectStructure) -> Result<Option<BuildTool>> {
        for config_file in &structure.config_files {
            if let Some(file_name) = config_file.path.file_name() {
                let name = file_name.to_string_lossy();

                // Check for build files
                match name.as_ref() {
                    "Cargo.toml" => return Ok(Some(BuildTool::Cargo)),
                    "package.json" => {
                        // Check for yarn.lock or pnpm-lock.yaml
                        if structure.root.join("yarn.lock").exists() {
                            return Ok(Some(BuildTool::Yarn));
                        } else if structure.root.join("pnpm-lock.yaml").exists() {
                            return Ok(Some(BuildTool::Pnpm));
                        } else {
                            return Ok(Some(BuildTool::Npm));
                        }
                    }
                    "pyproject.toml" => {
                        // Check if it's a Poetry project
                        let content = tokio::fs::read_to_string(&config_file.path).await?;
                        if content.contains("[tool.poetry]") {
                            return Ok(Some(BuildTool::Poetry));
                        }
                    }
                    "requirements.txt" | "setup.py" => return Ok(Some(BuildTool::Pip)),
                    "pom.xml" => return Ok(Some(BuildTool::Maven)),
                    "build.gradle" | "build.gradle.kts" => return Ok(Some(BuildTool::Gradle)),
                    "go.mod" => return Ok(Some(BuildTool::Go)),
                    "Gemfile" => return Ok(Some(BuildTool::Bundler)),
                    "Package.swift" => return Ok(Some(BuildTool::SwiftPM)),
                    _ => {}
                }

                // Check for .csproj files
                if name.ends_with(".csproj") {
                    return Ok(Some(BuildTool::Dotnet));
                }
            }
        }

        Ok(None)
    }

    async fn analyze_cargo(&self, structure: &ProjectStructure) -> Result<BuildInfo> {
        let mut scripts = HashMap::new();
        let mut dependencies = Vec::new();
        let mut dev_dependencies = Vec::new();

        // Find Cargo.toml
        for config_file in &structure.config_files {
            if config_file
                .path
                .file_name()
                .map(|n| n == "Cargo.toml")
                .unwrap_or(false)
            {
                let content = tokio::fs::read_to_string(&config_file.path).await?;

                // Parse TOML (simplified parsing)
                let toml: toml::Value = toml::from_str(&content)?;

                // Extract dependencies
                if let Some(deps) = toml.get("dependencies").and_then(|v| v.as_table()) {
                    for (name, value) in deps {
                        let version = extract_version(value);
                        dependencies.push(Dependency {
                            name: name.clone(),
                            version,
                        });
                    }
                }

                // Extract dev dependencies
                if let Some(deps) = toml.get("dev-dependencies").and_then(|v| v.as_table()) {
                    for (name, value) in deps {
                        let version = extract_version(value);
                        dev_dependencies.push(Dependency {
                            name: name.clone(),
                            version,
                        });
                    }
                }

                // Common cargo scripts
                scripts.insert("build".to_string(), "cargo build".to_string());
                scripts.insert("test".to_string(), "cargo test".to_string());
                scripts.insert("fmt".to_string(), "cargo fmt".to_string());
                scripts.insert("clippy".to_string(), "cargo clippy".to_string());

                break;
            }
        }

        Ok(BuildInfo {
            tool: BuildTool::Cargo,
            scripts,
            dependencies,
            dev_dependencies,
        })
    }

    async fn analyze_node(
        &self,
        structure: &ProjectStructure,
        tool: BuildTool,
    ) -> Result<BuildInfo> {
        let mut scripts = HashMap::new();
        let mut dependencies = Vec::new();
        let mut dev_dependencies = Vec::new();

        // Find package.json
        for config_file in &structure.config_files {
            if config_file
                .path
                .file_name()
                .map(|n| n == "package.json")
                .unwrap_or(false)
            {
                let content = tokio::fs::read_to_string(&config_file.path).await?;

                // Parse JSON
                let json: serde_json::Value = serde_json::from_str(&content)?;

                // Extract scripts
                if let Some(scripts_obj) = json.get("scripts").and_then(|v| v.as_object()) {
                    for (name, value) in scripts_obj {
                        if let Some(script) = value.as_str() {
                            scripts.insert(name.clone(), script.to_string());
                        }
                    }
                }

                // Extract dependencies
                if let Some(deps) = json.get("dependencies").and_then(|v| v.as_object()) {
                    for (name, value) in deps {
                        if let Some(version) = value.as_str() {
                            dependencies.push(Dependency {
                                name: name.clone(),
                                version: version.to_string(),
                            });
                        }
                    }
                }

                // Extract dev dependencies
                if let Some(deps) = json.get("devDependencies").and_then(|v| v.as_object()) {
                    for (name, value) in deps {
                        if let Some(version) = value.as_str() {
                            dev_dependencies.push(Dependency {
                                name: name.clone(),
                                version: version.to_string(),
                            });
                        }
                    }
                }

                break;
            }
        }

        Ok(BuildInfo {
            tool,
            scripts,
            dependencies,
            dev_dependencies,
        })
    }

    async fn analyze_python(
        &self,
        structure: &ProjectStructure,
        tool: BuildTool,
    ) -> Result<BuildInfo> {
        let mut scripts = HashMap::new();
        let mut dependencies = Vec::new();
        let dev_dependencies = Vec::new();

        if tool == BuildTool::Poetry {
            // Find pyproject.toml
            for config_file in &structure.config_files {
                if config_file
                    .path
                    .file_name()
                    .map(|n| n == "pyproject.toml")
                    .unwrap_or(false)
                {
                    let content = tokio::fs::read_to_string(&config_file.path).await?;
                    let toml: toml::Value = toml::from_str(&content)?;

                    // Extract dependencies from Poetry section
                    if let Some(poetry) = toml.get("tool").and_then(|t| t.get("poetry")) {
                        if let Some(deps) = poetry.get("dependencies").and_then(|v| v.as_table()) {
                            for (name, value) in deps {
                                if name != "python" {
                                    let version = extract_version(value);
                                    dependencies.push(Dependency {
                                        name: name.clone(),
                                        version,
                                    });
                                }
                            }
                        }
                    }

                    // Common poetry scripts
                    scripts.insert("install".to_string(), "poetry install".to_string());
                    scripts.insert("test".to_string(), "poetry run pytest".to_string());

                    break;
                }
            }
        } else {
            // Find requirements.txt
            for config_file in &structure.config_files {
                if config_file
                    .path
                    .file_name()
                    .map(|n| n == "requirements.txt")
                    .unwrap_or(false)
                {
                    let content = tokio::fs::read_to_string(&config_file.path).await?;

                    for line in content.lines() {
                        let line = line.trim();
                        if !line.is_empty() && !line.starts_with('#') {
                            // Parse requirement (simplified)
                            if let Some((name, version)) = parse_requirement(line) {
                                dependencies.push(Dependency { name, version });
                            }
                        }
                    }

                    // Common pip scripts
                    scripts.insert(
                        "install".to_string(),
                        "pip install -r requirements.txt".to_string(),
                    );

                    break;
                }
            }
        }

        Ok(BuildInfo {
            tool,
            scripts,
            dependencies,
            dev_dependencies,
        })
    }

    async fn analyze_maven(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement Maven analysis
        Ok(BuildInfo {
            tool: BuildTool::Maven,
            scripts: HashMap::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }

    async fn analyze_gradle(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement Gradle analysis
        Ok(BuildInfo {
            tool: BuildTool::Gradle,
            scripts: HashMap::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }

    async fn analyze_go(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement Go analysis
        let mut scripts = HashMap::new();
        scripts.insert("build".to_string(), "go build".to_string());
        scripts.insert("test".to_string(), "go test ./...".to_string());
        scripts.insert("fmt".to_string(), "go fmt ./...".to_string());

        Ok(BuildInfo {
            tool: BuildTool::Go,
            scripts,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }

    async fn analyze_dotnet(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement .NET analysis
        let mut scripts = HashMap::new();
        scripts.insert("build".to_string(), "dotnet build".to_string());
        scripts.insert("test".to_string(), "dotnet test".to_string());

        Ok(BuildInfo {
            tool: BuildTool::Dotnet,
            scripts,
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }

    async fn analyze_bundler(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement Bundler analysis
        Ok(BuildInfo {
            tool: BuildTool::Bundler,
            scripts: HashMap::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }

    async fn analyze_swift(&self, _structure: &ProjectStructure) -> Result<BuildInfo> {
        // TODO: Implement Swift Package Manager analysis
        Ok(BuildInfo {
            tool: BuildTool::SwiftPM,
            scripts: HashMap::new(),
            dependencies: Vec::new(),
            dev_dependencies: Vec::new(),
        })
    }
}

impl Default for BuildAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_version(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        toml::Value::Table(t) => t
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("*")
            .to_string(),
        _ => "*".to_string(),
    }
}

fn parse_requirement(line: &str) -> Option<(String, String)> {
    // Simple parsing of requirements.txt lines
    if let Some(pos) = line.find("==") {
        let name = line[..pos].trim().to_string();
        let version = line[pos + 2..].trim().to_string();
        Some((name, version))
    } else if let Some(pos) = line.find(">=") {
        let name = line[..pos].trim().to_string();
        let version = format!(">={}", line[pos + 2..].trim());
        Some((name, version))
    } else {
        Some((line.to_string(), "*".to_string()))
    }
}
