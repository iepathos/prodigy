# mdBook Chapter Templates

This directory contains templates for organizing mdBook chapters consistently across projects.

## Available Templates

### quick-start.yaml
For introductory chapters that help users get started quickly.

**Typical subsections:**
- Overview
- Installation
- First Example
- Next Steps

**Use when:** Chapter introduces users to the project, provides installation steps, and shows a simple example.

### configuration-reference.yaml
For chapters documenting configuration options and settings.

**Typical subsections:**
- Overview
- Configuration Options
- Environment Variables
- Configuration Files
- Examples

**Use when:** Chapter describes how to configure the software, environment variables, config files.

### cli-reference.yaml
For command-line interface documentation.

**Typical subsections:**
- Overview
- Global Options
- Commands
- Examples

**Use when:** Chapter documents CLI commands, flags, and usage.

### advanced-features.yaml
For chapters covering complex features or workflows.

**Typical subsections:**
- Getting Started
- Configuration
- State Management
- Performance
- Examples

**Use when:** Chapter covers sophisticated features like MapReduce, workflows, or advanced capabilities.

### troubleshooting.yaml
For debugging and problem-solving chapters.

**Typical subsections:**
- Overview
- Common Issues
- Error Messages
- Debugging Tools
- Getting Help

**Use when:** Chapter helps users debug problems, understand errors, and get support.

### examples-tutorials.yaml
For chapters with code examples and tutorials.

**Typical subsections:**
- Overview
- Basic Examples
- Advanced Examples
- Tutorials
- Recipes

**Use when:** Chapter provides practical examples, walkthroughs, or recipes.

### api-documentation.yaml
For API reference documentation.

**Typical subsections:**
- Overview
- Core Types
- API Methods
- Error Handling
- Examples

**Use when:** Chapter documents APIs, libraries, or programmatic interfaces.

## Template Structure

Each template is a YAML file with:

```yaml
name: "Template Name"

# Keywords that identify this chapter type
chapter_types:
  - "keyword1"
  - "keyword2"

# Standard subsection structure
subsection_structure:
  - name: "Subsection Name"
    aliases:
      - "Alternative Name 1"
      - "Alternative Name 2"
    topics:
      - "topic1"
      - "topic2"
    order: 1
    required: true/false

# Naming standardization rules
naming_rules:
  - prefer: "Preferred Name"
    over:
      - "Alternative 1"
      - "Alternative 2"

# Validation constraints
validation_rules:
  required_subsections:
    - "Subsection Name"
  optional_subsections:
    - "Optional Name"
  max_subsections: 8
```

## Using Templates

### Automatic Template Detection

```bash
/prodigy-ai-organize-chapter \
  --chapter-file book/src/mapreduce.md \
  --template auto
```

The command analyzes the chapter and selects the best matching template.

### Manual Template Selection

```bash
/prodigy-ai-organize-chapter \
  --chapter-file book/src/configuration.md \
  --template configuration-reference
```

### Analyzing Cross-Project Consistency

```bash
/prodigy-analyze-book-consistency \
  --books "prodigy:book,debtmap:../debtmap/book" \
  --output consistency-report.json
```

## Creating Custom Templates

1. Copy an existing template as a starting point
2. Modify the structure to match your needs
3. Save with a descriptive name (e.g., `my-custom-template.yaml`)
4. Use with `--template my-custom-template`

## Best Practices

1. **Use consistent naming:** Follow template naming conventions across projects
2. **Respect required subsections:** Templates define minimum required content
3. **Follow ordering:** Template order reflects logical reading progression
4. **Add project-specific subsections:** Templates are guidelines, not strict requirements
5. **Review consistency reports:** Regular consistency checks help maintain quality

## Template Evolution

Templates improve over time based on:
- User feedback
- Documentation analytics
- Cross-project learning
- Manual adjustments

Refinements are applied through the `/prodigy-refine-template` command (planned).

## Related Commands

- `/prodigy-ai-analyze-chapter-semantics` - Analyze chapter structure
- `/prodigy-ai-organize-chapter` - Apply template-based organization
- `/prodigy-analyze-book-consistency` - Check cross-project consistency
- `/prodigy-auto-organize-chapters` - Batch organize multiple chapters
