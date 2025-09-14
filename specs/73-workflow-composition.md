---
number: 73
title: Workflow Composition and Reusability
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 73: Workflow Composition and Reusability

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

While not explicitly shown in the whitepaper, the complexity of the example workflows suggests a need for composition and reusability. Large workflows like the "Modernize JavaScript codebase" example with multiple phases would benefit from being able to compose smaller, reusable workflow components.

## Objective

Implement workflow composition capabilities that enable building complex workflows from reusable components, supporting workflow imports, templates, and parameterization for maximum reusability and maintainability.

## Requirements

### Functional Requirements
- Import workflows from other files
- Define reusable workflow templates
- Parameterize workflows with inputs
- Compose workflows from sub-workflows
- Override template values at runtime
- Share common configurations
- Workflow libraries and registries
- Conditional workflow inclusion
- Workflow inheritance and extension

### Non-Functional Requirements
- Minimal overhead for composition
- Clear error messages for circular dependencies
- Efficient template resolution
- Type-safe parameter passing

## Acceptance Criteria

- [ ] `import: ./common/workflows.yml` imports workflows
- [ ] `extends: base-workflow` inherits from base
- [ ] `template: refactor-module` uses template
- [ ] Parameters passed via `with:` block
- [ ] Override templates with `override:` block
- [ ] Shared configs via `defaults:` section
- [ ] Workflow registry for common patterns
- [ ] Circular dependency detection
- [ ] Clear composition error messages
- [ ] Documentation generation from workflows

## Technical Details

### Implementation Approach

1. **Workflow Composition Structure**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ComposableWorkflow {
       /// Base workflow configuration
       #[serde(flatten)]
       pub config: WorkflowConfig,

       /// Import other workflow files
       #[serde(skip_serializing_if = "Option::is_none")]
       pub imports: Option<Vec<WorkflowImport>>,

       /// Extend from base workflow
       #[serde(skip_serializing_if = "Option::is_none")]
       pub extends: Option<String>,

       /// Use workflow template
       #[serde(skip_serializing_if = "Option::is_none")]
       pub template: Option<WorkflowTemplate>,

       /// Define parameters
       #[serde(skip_serializing_if = "Option::is_none")]
       pub parameters: Option<ParameterDefinitions>,

       /// Default values
       #[serde(skip_serializing_if = "Option::is_none")]
       pub defaults: Option<HashMap<String, Value>>,

       /// Sub-workflows
       #[serde(skip_serializing_if = "Option::is_none")]
       pub workflows: Option<HashMap<String, SubWorkflow>>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowImport {
       pub path: PathBuf,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub alias: Option<String>,
       #[serde(default)]
       pub selective: Vec<String>, // Import specific workflows
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowTemplate {
       pub name: String,
       pub source: TemplateSource,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub with: Option<HashMap<String, Value>>,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub override_field: Option<HashMap<String, Value>>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ParameterDefinitions {
       pub required: Vec<Parameter>,
       pub optional: Vec<Parameter>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct Parameter {
       pub name: String,
       pub type_hint: ParameterType,
       pub description: String,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub default: Option<Value>,
       #[serde(skip_serializing_if = "Option::is_none")]
       pub validation: Option<String>,
   }
   ```

2. **Workflow Composer**:
   ```rust
   pub struct WorkflowComposer {
       loader: WorkflowLoader,
       template_registry: TemplateRegistry,
       resolver: DependencyResolver,
   }

   impl WorkflowComposer {
       pub async fn compose(
           &self,
           source: &Path,
           params: HashMap<String, Value>,
       ) -> Result<ComposedWorkflow> {
           // Load base workflow
           let mut workflow = self.loader.load(source).await?;

           // Process imports
           if let Some(imports) = &workflow.imports {
               self.process_imports(&mut workflow, imports).await?;
           }

           // Apply inheritance
           if let Some(base_name) = &workflow.extends {
               self.apply_inheritance(&mut workflow, base_name).await?;
           }

           // Apply template
           if let Some(template) = &workflow.template {
               self.apply_template(&mut workflow, template).await?;
           }

           // Validate and apply parameters
           self.apply_parameters(&mut workflow, params)?;

           // Resolve sub-workflows
           if let Some(sub_workflows) = &workflow.workflows {
               self.resolve_sub_workflows(&mut workflow, sub_workflows).await?;
           }

           // Apply defaults
           if let Some(defaults) = &workflow.defaults {
               self.apply_defaults(&mut workflow, defaults)?;
           }

           // Validate final composition
           self.validate_composition(&workflow)?;

           Ok(ComposedWorkflow {
               workflow,
               metadata: self.generate_metadata(&workflow),
           })
       }

       async fn process_imports(
           &self,
           workflow: &mut ComposableWorkflow,
           imports: &[WorkflowImport],
       ) -> Result<()> {
           for import in imports {
               let imported = self.loader.load(&import.path).await?;

               if let Some(alias) = &import.alias {
                   // Import with alias
                   self.register_aliased(workflow, alias, imported)?;
               } else if !import.selective.is_empty() {
                   // Selective import
                   self.import_selective(workflow, imported, &import.selective)?;
               } else {
                   // Import all
                   self.merge_workflows(workflow, imported)?;
               }
           }

           Ok(())
       }

       async fn apply_template(
           &self,
           workflow: &mut ComposableWorkflow,
           template: &WorkflowTemplate,
       ) -> Result<()> {
           // Load template
           let base_template = self.template_registry
               .get(&template.name)
               .await?;

           // Apply parameters
           let mut instantiated = base_template.clone();
           if let Some(params) = &template.with {
               self.apply_template_params(&mut instantiated, params)?;
           }

           // Apply overrides
           if let Some(overrides) = &template.override_field {
               self.apply_overrides(&mut instantiated, overrides)?;
           }

           // Merge with current workflow
           self.merge_workflows(workflow, instantiated)?;

           Ok(())
       }

       fn validate_composition(&self, workflow: &ComposableWorkflow) -> Result<()> {
           // Check for circular dependencies
           self.resolver.check_circular_deps(workflow)?;

           // Validate parameter usage
           self.validate_parameters(workflow)?;

           // Check sub-workflow references
           self.validate_sub_workflows(workflow)?;

           Ok(())
       }
   }
   ```

3. **Sub-Workflow Execution**:
   ```rust
   pub struct SubWorkflowExecutor {
       composer: WorkflowComposer,
       executor: WorkflowExecutor,
   }

   impl SubWorkflowExecutor {
       pub async fn execute_sub_workflow(
           &self,
           parent_context: &ExecutionContext,
           sub_workflow: &SubWorkflow,
       ) -> Result<SubWorkflowResult> {
           // Create isolated context
           let mut sub_context = self.create_sub_context(
               parent_context,
               &sub_workflow.inputs,
           )?;

           // Compose sub-workflow
           let composed = self.composer.compose(
               &sub_workflow.source,
               sub_workflow.parameters.clone(),
           ).await?;

           // Execute
           let result = self.executor.execute(
               &composed.workflow,
               &mut sub_context,
           ).await?;

           // Extract outputs
           let outputs = self.extract_outputs(
               &sub_context,
               &sub_workflow.outputs,
           )?;

           // Merge back to parent context
           self.merge_contexts(parent_context, &sub_context, &outputs)?;

           Ok(SubWorkflowResult {
               success: result.success,
               outputs,
               duration: result.duration,
           })
       }
   }
   ```

4. **Template Registry**:
   ```rust
   pub struct TemplateRegistry {
       templates: Arc<RwLock<HashMap<String, WorkflowTemplate>>>,
       storage: Box<dyn TemplateStorage>,
   }

   impl TemplateRegistry {
       pub async fn register_template(
           &self,
           name: String,
           template: WorkflowTemplate,
       ) -> Result<()> {
           // Validate template
           self.validate_template(&template)?;

           // Store template
           self.storage.store(&name, &template).await?;

           // Cache in memory
           self.templates.write().await.insert(name, template);

           Ok(())
       }

       pub async fn get(&self, name: &str) -> Result<WorkflowTemplate> {
           // Check cache
           if let Some(template) = self.templates.read().await.get(name) {
               return Ok(template.clone());
           }

           // Load from storage
           let template = self.storage.load(name).await?;

           // Cache for future use
           self.templates.write().await.insert(
               name.to_string(),
               template.clone(),
           );

           Ok(template)
       }
   }
   ```

### Architecture Changes
- Add `WorkflowComposer` component
- Add `TemplateRegistry` for reusable templates
- Enhance workflow loader with import resolution
- Add sub-workflow execution support
- Implement parameter validation system

### Data Structures
```yaml
# Base workflow template (templates/refactor-base.yml)
name: refactor-base
parameters:
  required:
    - name: target_file
      type: string
      description: "File to refactor"
  optional:
    - name: style
      type: string
      default: "functional"

tasks:
  - name: "Analyze ${target_file}"
    claude: "/analyze ${target_file}"
    capture: analysis

  - name: "Refactor"
    claude: "/refactor ${target_file} --style ${style}"
    validate: "npm test ${target_file}"

---
# Composed workflow using template
name: refactor-modules
imports:
  - path: ./templates/common.yml
    alias: common

template:
  name: refactor-base
  with:
    style: "modular"

workflows:
  process_each:
    source: ./templates/refactor-base.yml
    foreach: "find . -name '*.js'"
    parallel: 5
    inputs:
      target_file: "${item}"
    outputs:
      - refactored_file

tasks:
  - name: "Setup"
    use: common.setup_environment

  - name: "Process modules"
    execute: process_each

  - name: "Validate all"
    extends: common.validation
    override:
      command: "npm test"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/config/workflow.rs` - Composition syntax
  - `src/cook/workflow/` - Composition logic
  - `src/cook/execution/` - Sub-workflow execution
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Template parameter substitution
  - Import resolution
  - Circular dependency detection
  - Override application
- **Integration Tests**:
  - End-to-end composition
  - Sub-workflow execution
  - Template instantiation
  - Cross-file imports
- **Validation Tests**:
  - Invalid compositions
  - Missing parameters
  - Type mismatches
  - Circular references

## Documentation Requirements

- **Code Documentation**: Document composition patterns
- **User Documentation**:
  - Composition guide
  - Template creation
  - Parameter usage
  - Best practices
- **Architecture Updates**: Add composition to workflow architecture

## Implementation Notes

- Use lazy loading for imports
- Cache composed workflows
- Validate at composition time, not runtime
- Support local and remote template sources
- Future: Package manager for workflow templates

## Migration and Compatibility

- Simple workflows work without composition
- Gradual adoption of composition features
- Backwards compatible with existing workflows
- Clear migration path from duplicated to composed