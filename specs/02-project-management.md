# Feature: Project Management

## Objective
Implement comprehensive project management capabilities that allow users to efficiently manage multiple Claude-assisted projects with different configurations, workflows, and team collaborations.

## Acceptance Criteria
- [ ] Create, list, and switch between projects
- [ ] Import existing projects with auto-detection
- [ ] Project templates for common scenarios
- [ ] Project cloning and forking
- [ ] Project archival and restoration
- [ ] Multi-project operations (bulk actions)
- [ ] Project health checks and validation
- [ ] Project migration between versions

## Technical Details

### CLI Commands
```bash
# Project lifecycle
mmm project new <name> [--template <template>] [--path <path>]
mmm project init [--name <name>]  # Initialize in current directory
mmm project list [--format json|table]
mmm project info [<name>]
mmm project switch <name>
mmm project clone <source> <destination>
mmm project archive <name>
mmm project unarchive <name>
mmm project delete <name> [--force]

# Project configuration
mmm project config get <key>
mmm project config set <key> <value>
mmm project config list

# Project templates
mmm template list
mmm template create <name> --from-project <project>
mmm template install <url>
mmm template remove <name>
```

### Project Templates

Built-in templates:
1. **web-app**: Full-stack web application
2. **cli-tool**: Command-line application
3. **library**: Reusable library/package
4. **api-service**: REST/GraphQL API
5. **data-pipeline**: Data processing pipeline
6. **ml-model**: Machine learning project

Template structure:
```yaml
# template.yaml
name: web-app
description: Full-stack web application template
version: 1.0.0
author: mmm-community

variables:
  - name: project_name
    description: Name of the project
    required: true
  - name: framework
    description: Web framework to use
    default: "react"
    choices: ["react", "vue", "angular", "svelte"]

structure:
  - path: specs/
    files:
      - authentication.md
      - database-schema.md
      - api-design.md
      - frontend-architecture.md
  - path: .mmm/
    files:
      - config.toml
      - workflows/
        - development.yaml
        - testing.yaml
        - deployment.yaml

config:
  default_command: "/implement-spec"
  phases:
    - planning
    - implementation
    - testing
    - review
```

### Project Registry

Global project registry at `~/.mmm/projects/`:
```toml
# project-name.toml
[project]
name = "my-awesome-app"
path = "/Users/username/projects/my-awesome-app"
created = 2024-01-15T10:30:00Z
last_accessed = 2024-01-20T15:45:00Z
template = "web-app"
version = "0.1.0"

[metadata]
description = "A revolutionary web application"
tags = ["web", "saas", "ai"]
team = ["alice", "bob"]
repository = "https://github.com/user/my-awesome-app"

[stats]
total_specs = 25
completed_specs = 18
total_iterations = 156
success_rate = 0.92
```

### Project Health Checks

Health check system to validate project state:
- Configuration file validity
- Specification syntax checking
- State database integrity
- File permissions
- Dependencies availability
- Claude CLI connectivity

```rust
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub severity: Severity,
}

pub enum HealthStatus {
    Passing,
    Warning,
    Failing,
}

pub enum Severity {
    Critical,  // Blocks execution
    Major,     // Degrades functionality  
    Minor,     // Cosmetic issues
}
```

### Multi-Project Operations

Bulk operations across projects:
```bash
# Run specs across multiple projects
mmm multi run --projects project1,project2 --spec "authentication"

# Sync configurations
mmm multi sync-config --source template --targets all

# Generate reports
mmm multi report --format html --output report.html

# Batch updates
mmm multi update --component claude-integration
```

### Project Migration

Version migration system:
```rust
pub trait Migration {
    fn version(&self) -> Version;
    fn up(&self, project: &mut Project) -> Result<()>;
    fn down(&self, project: &mut Project) -> Result<()>;
}

// Example migration
struct V1ToV2Migration;
impl Migration for V1ToV2Migration {
    fn version(&self) -> Version {
        Version::new(2, 0, 0)
    }
    
    fn up(&self, project: &mut Project) -> Result<()> {
        // Convert old state format to new database
        // Update configuration schema
        // Migrate spec frontmatter
    }
}
```

### Project Context

Automatic project detection and context switching:
- Detect .mmm directory in current path
- Remember last active project
- Project-specific shell aliases
- IDE integration markers