---
number: 159
title: AI-Driven Section Grouping for Cross-Project Consistency
category: optimization
priority: low
status: draft
dependencies: [157, 158]
created: 2025-01-11
---

# Specification 159: AI-Driven Section Grouping for Cross-Project Consistency

**Category**: optimization
**Priority**: low
**Status**: draft
**Dependencies**: Spec 157 (mdBook Subsection Organization), Spec 158 (Subsection-Aware Drift Detection)

## Context

After implementing Specs 157 and 158, we can automatically split chapters into subsections and detect drift at subsection granularity. However, subsection organization is still **mechanical**:

- **Spec 157**: Splits based on H2 headings (structural, not semantic)
- **Spec 158**: Defines subsections manually in chapters.json (requires human judgment)

This creates inconsistencies when generating documentation for multiple projects:

### Problem: Inconsistent Organization Across Projects

**Prodigy's MapReduce Chapter**:
```
- Quick Start
- Complete Structure
- Environment Variables
- Backoff Strategies
- Setup Phase
- Checkpoint and Resume
- Dead Letter Queue
- Performance Tuning
```

**Debtmap's Debt Analysis Chapter** (hypothetical):
```
- Overview
- Installation
- Running Analysis
- Configuration Options
- Output Formats
- Troubleshooting
- Advanced Features
- Examples
```

**Ripgrep Integration Chapter** (hypothetical):
```
- Introduction
- Basic Usage
- Search Patterns
- File Filtering
- Performance Tips
- Examples and Recipes
```

Notice:
- Inconsistent naming: "Quick Start" vs "Overview" vs "Introduction"
- Different ordering: Some put examples early, others late
- Varying granularity: Some merge related topics, others split them
- No standard structure for similar chapter types

### Problem: Mechanical Splitting Lacks Semantic Understanding

**Current Approach** (Spec 157): Each H2 → separate subsection
```markdown
## Quick Start
## Complete Structure
## Environment Variables in Configuration
## Backoff Strategies
## Error Collection Strategies
```

**Result**: 5 subsections (too granular, lost thematic grouping)

**Better Semantic Grouping**:
```markdown
## Getting Started
   - Quick Start
   - Complete Structure

## Configuration
   - Environment Variables
   - Backoff Strategies
   - Error Collection

## Advanced Topics
   - (other sections)
```

**Result**: 3 subsections with logical grouping (better UX)

### Problem: No Cross-Project Learning

When generating docs for a new project, we start from scratch:
- No templates for common chapter types
- No learned best practices from other projects
- No consistency in how similar features are documented
- Each project's book feels different

**What We Want**:
- All "workflow basics" chapters have same structure across projects
- All "configuration" chapters organized consistently
- All "CLI reference" chapters follow same pattern
- Learned patterns from existing books applied to new projects

## Objective

Use AI/LLM to analyze documentation content semantically and create **consistent, intelligent subsection groupings** across multiple projects by:

1. Learning optimal subsection patterns from existing high-quality documentation
2. Grouping related sections by semantic similarity (not just H2 headings)
3. Applying consistent naming and organization across similar chapter types
4. Maintaining project-specific flexibility while enforcing cross-project standards
5. Continuously improving organization based on user feedback and book analytics

This ensures documentation books for different projects (Prodigy, Debtmap, Ripgrep, etc.) have consistent, professional organization.

## Requirements

### Functional Requirements

1. **Semantic Section Analysis**
   - Parse chapter content to understand topics and relationships
   - Use embeddings or topic modeling to cluster related content
   - Identify semantic boundaries (not just structural H2s)
   - Detect related concepts that should be grouped together

2. **Cross-Project Template Learning**
   - Analyze existing high-quality documentation (Rust Book, mdBook Guide, etc.)
   - Extract common organizational patterns for chapter types
   - Build templates for standard chapter types:
     - Getting Started / Quick Start
     - Installation and Setup
     - Configuration Reference
     - CLI Reference
     - API Documentation
     - Advanced Features
     - Troubleshooting
     - Examples and Tutorials
   - Store templates in `.prodigy/book-templates/`

3. **Intelligent Subsection Grouping**
   - Propose subsection groupings based on semantic analysis
   - Suggest subsection titles that reflect grouped content
   - Maintain consistent naming across projects (e.g., always "Quick Start" not "Getting Started")
   - Balance granularity (avoid too many tiny subsections)
   - Provide rationale for grouping decisions

4. **Template Application**
   - Detect chapter type based on content and title
   - Apply appropriate template for that chapter type
   - Adapt template to project-specific content
   - Allow manual overrides for special cases

5. **Consistency Enforcement**
   - Validate subsection organization against templates
   - Flag inconsistencies across similar chapters in same project
   - Suggest standardization improvements
   - Generate consistency reports

### Non-Functional Requirements

1. **Quality**: AI-suggested groupings should be as good or better than manual organization
2. **Consistency**: 90% of similar chapters across projects should use same structure
3. **Flexibility**: Allow project-specific customization when needed
4. **Transparency**: Provide clear rationale for grouping decisions
5. **Learning**: Improve templates based on feedback and analytics

## Acceptance Criteria

- [ ] Command `/prodigy-ai-organize-chapter` analyzes chapter semantically
- [ ] Semantic clustering identifies related H2 sections for grouping
- [ ] Subsection groupings proposed with rationale (why these sections together?)
- [ ] Template library created with standard chapter types (5+ templates)
- [ ] Template matching detects chapter type automatically (>90% accuracy)
- [ ] Cross-project consistency enforced (similar chapters use same template)
- [ ] Naming standardization (e.g., "Quick Start" used consistently, not variants)
- [ ] Dry-run mode shows proposed groupings before applying
- [ ] Manual override supported for special cases
- [ ] Template refinement command learns from manual adjustments
- [ ] Consistency report shows deviation from templates across projects
- [ ] Documentation books for 3+ projects use consistent organization

## Technical Details

### Implementation Approach

#### 1. Semantic Section Analysis

**Command**: `/prodigy-ai-analyze-chapter-semantics`

**Input**: Chapter markdown file
**Output**: Semantic section analysis with clustering

**Algorithm**:
```python
# Pseudo-code
def analyze_chapter_semantics(chapter_md):
    # Parse markdown to extract sections
    sections = parse_markdown(chapter_md)

    # Generate embeddings for each section
    embeddings = []
    for section in sections:
        text = section.title + "\n" + section.content
        embedding = generate_embedding(text)  # OpenAI, local model, etc.
        embeddings.append({
            "section": section,
            "embedding": embedding,
            "topics": extract_topics(section.content)
        })

    # Cluster similar sections
    clusters = cluster_embeddings(embeddings, optimal_k=3-8)

    # Name clusters based on common topics
    named_clusters = []
    for cluster in clusters:
        common_topics = find_common_topics(cluster.sections)
        cluster_name = generate_cluster_name(common_topics)
        named_clusters.append({
            "name": cluster_name,
            "sections": cluster.sections,
            "topics": common_topics,
            "rationale": explain_grouping(cluster)
        })

    return named_clusters
```

**Example Output**:
```json
{
  "chapter": "MapReduce Workflows",
  "semantic_clusters": [
    {
      "name": "Getting Started",
      "sections": ["Quick Start", "Complete Structure"],
      "topics": ["basic usage", "workflow syntax", "first steps"],
      "rationale": "Both sections introduce users to basic MapReduce concepts and syntax"
    },
    {
      "name": "Configuration",
      "sections": ["Environment Variables", "Backoff Strategies", "Error Collection"],
      "topics": ["configuration", "settings", "customization"],
      "rationale": "All sections deal with configuring MapReduce behavior"
    },
    {
      "name": "Execution and State Management",
      "sections": ["Checkpoint and Resume", "Dead Letter Queue"],
      "topics": ["runtime", "state", "recovery", "failure handling"],
      "rationale": "Sections cover runtime behavior and state management"
    }
  ]
}
```

#### 2. Template Library

**Template Structure**:
```
.prodigy/book-templates/
├── quick-start.yaml
├── configuration-reference.yaml
├── cli-reference.yaml
├── api-documentation.yaml
├── advanced-features.yaml
├── troubleshooting.yaml
└── examples-tutorials.yaml
```

**Example Template** (`quick-start.yaml`):
```yaml
name: "Quick Start"
chapter_types:
  - "getting started"
  - "quick start"
  - "introduction"
  - "tutorial"

subsection_structure:
  - name: "Overview"
    aliases: ["Introduction", "What is X"]
    topics: ["purpose", "goals", "use cases"]
    order: 1

  - name: "Installation"
    aliases: ["Setup", "Getting Started"]
    topics: ["install", "setup", "dependencies"]
    order: 2

  - name: "First Example"
    aliases: ["Hello World", "Quick Start", "Basic Example"]
    topics: ["simple example", "basic usage"]
    order: 3

  - name: "Next Steps"
    aliases: ["Learn More", "Where to Go", "Additional Resources"]
    topics: ["links", "further reading", "advanced topics"]
    order: 4

naming_rules:
  - prefer: "Quick Start"
    over: ["Getting Started", "Quickstart", "Quick Guide"]
  - prefer: "Installation"
    over: ["Setup", "Installing", "Install Guide"]

validation_rules:
  - required_subsections: ["Overview", "First Example"]
  - optional_subsections: ["Installation", "Next Steps"]
  - max_subsections: 5
```

**Template Matching**:
```python
def match_chapter_to_template(chapter):
    # Extract chapter metadata
    title = chapter.title.lower()
    topics = extract_topics(chapter.content)
    section_titles = [s.title.lower() for s in chapter.sections]

    # Score each template
    template_scores = {}
    for template in load_all_templates():
        score = 0

        # Match by title keywords
        for chapter_type in template.chapter_types:
            if chapter_type in title:
                score += 10

        # Match by topics
        for topic in topics:
            if topic in template.topics:
                score += 2

        # Match by section titles
        for section in section_titles:
            for subsection in template.subsection_structure:
                if section in subsection.aliases:
                    score += 5

        template_scores[template.name] = score

    # Return best matching template
    best_template = max(template_scores, key=template_scores.get)
    confidence = template_scores[best_template] / sum(template_scores.values())

    return {
        "template": best_template,
        "confidence": confidence,
        "all_scores": template_scores
    }
```

#### 3. Intelligent Subsection Grouping

**Command**: `/prodigy-ai-organize-chapter`

**Parameters**:
```bash
/prodigy-ai-organize-chapter \
  --chapter-file book/src/mapreduce.md \
  --strategy semantic \
  --max-subsections 8 \
  --template auto \
  --dry-run true
```

**Algorithm**:
```python
def ai_organize_chapter(chapter_file, strategy, max_subsections, template):
    # Step 1: Analyze chapter semantics
    semantic_analysis = analyze_chapter_semantics(chapter_file)

    # Step 2: Match to template (if auto)
    if template == "auto":
        template_match = match_chapter_to_template(chapter_file)
        template = load_template(template_match.template)
    else:
        template = load_template(template)

    # Step 3: Apply template to semantic clusters
    subsections = []
    for cluster in semantic_analysis.clusters:
        # Find best matching template subsection
        best_match = find_best_template_match(cluster, template)

        subsections.append({
            "title": best_match.preferred_name,  # Use standardized name
            "sections": cluster.sections,
            "topics": cluster.topics,
            "rationale": cluster.rationale,
            "template_match": best_match.name,
            "order": best_match.order
        })

    # Step 4: Sort by template order
    subsections.sort(key=lambda s: s.order)

    # Step 5: Validate against template rules
    validation = validate_subsections(subsections, template)

    # Step 6: Generate grouping proposal
    proposal = {
        "chapter": chapter_file,
        "template_used": template.name,
        "subsections": subsections,
        "validation": validation,
        "changes_required": generate_changes(chapter_file, subsections)
    }

    return proposal
```

**Example Proposal Output**:
```json
{
  "chapter": "book/src/mapreduce.md",
  "template_used": "Advanced Features",
  "template_confidence": 0.85,
  "proposed_subsections": [
    {
      "title": "Getting Started",
      "sections": ["Quick Start", "Complete Structure"],
      "file": "book/src/mapreduce/getting-started.md",
      "rationale": "Introductory content for new users",
      "template_match": "Overview",
      "order": 1
    },
    {
      "title": "Configuration",
      "sections": ["Environment Variables", "Backoff Strategies", "Error Collection"],
      "file": "book/src/mapreduce/configuration.md",
      "rationale": "All configuration-related topics grouped together",
      "template_match": "Configuration Options",
      "order": 2
    },
    {
      "title": "State Management",
      "sections": ["Checkpoint and Resume", "Dead Letter Queue"],
      "file": "book/src/mapreduce/state-management.md",
      "rationale": "Runtime state and recovery mechanisms",
      "template_match": "Runtime Behavior",
      "order": 3
    },
    {
      "title": "Performance Tuning",
      "sections": ["Performance Tuning"],
      "file": "book/src/mapreduce/performance.md",
      "rationale": "Optimization and tuning guidance",
      "template_match": "Optimization",
      "order": 4
    },
    {
      "title": "Examples",
      "sections": ["Real-World Use Cases", "Troubleshooting"],
      "file": "book/src/mapreduce/examples.md",
      "rationale": "Practical examples and common issues",
      "template_match": "Examples and Troubleshooting",
      "order": 5
    }
  ],
  "validation": {
    "issues": [],
    "warnings": [
      "Template suggests 'Next Steps' subsection but chapter has none"
    ]
  }
}
```

#### 4. Cross-Project Consistency Enforcement

**Command**: `/prodigy-analyze-book-consistency`

**Parameters**:
```bash
/prodigy-analyze-book-consistency \
  --books "prodigy:book,debtmap:../debtmap/book,ripgrep:../ripgrep-book/book" \
  --output consistency-report.json
```

**Algorithm**:
```python
def analyze_cross_project_consistency(books):
    # Load all books
    all_chapters = {}
    for project, book_dir in books.items():
        chapters = load_book_chapters(book_dir)
        all_chapters[project] = chapters

    # Group chapters by type
    chapter_types = group_chapters_by_type(all_chapters)

    # Analyze consistency for each type
    consistency_report = {}
    for chapter_type, chapters in chapter_types.items():
        # Compare subsection structures
        structures = [extract_structure(ch) for ch in chapters]

        # Find common pattern
        common_pattern = find_common_pattern(structures)

        # Identify deviations
        deviations = []
        for project, structure in zip(chapters, structures):
            diff = compare_structures(structure, common_pattern)
            if diff:
                deviations.append({
                    "project": project.project,
                    "chapter": project.title,
                    "differences": diff
                })

        consistency_report[chapter_type] = {
            "common_pattern": common_pattern,
            "deviations": deviations,
            "consistency_score": calculate_consistency_score(structures)
        }

    return consistency_report
```

**Example Consistency Report**:
```json
{
  "chapter_type": "Configuration Reference",
  "projects_analyzed": ["prodigy", "debtmap", "ripgrep"],
  "common_pattern": {
    "subsections": ["Overview", "Configuration Options", "Environment Variables", "Examples"],
    "naming": "Configuration"
  },
  "deviations": [
    {
      "project": "debtmap",
      "chapter": "Configuration",
      "differences": [
        {
          "type": "naming",
          "expected": "Quick Start",
          "actual": "Getting Started",
          "suggestion": "Rename to 'Quick Start' for consistency"
        },
        {
          "type": "missing_subsection",
          "expected": "Environment Variables",
          "suggestion": "Add 'Environment Variables' subsection"
        }
      ]
    }
  ],
  "consistency_score": 0.78,
  "recommendations": [
    "Standardize 'Getting Started' to 'Quick Start' across all projects",
    "Add 'Environment Variables' subsection to debtmap Configuration chapter"
  ]
}
```

#### 5. Template Learning and Refinement

**Command**: `/prodigy-refine-template`

**Use Case**: Learn from manual adjustments to improve templates

**Algorithm**:
```python
def refine_template_from_manual_edits(chapter, manual_structure):
    # Load chapter's original template
    template_match = match_chapter_to_template(chapter)
    template = load_template(template_match.template)

    # Compare manual structure to template
    differences = compare_structures(manual_structure, template)

    # Analyze differences for patterns
    if len(differences) >= 3:  # Significant deviation
        # User might have found better organization
        proposed_changes = {
            "template": template.name,
            "suggested_updates": []
        }

        for diff in differences:
            if diff.type == "subsection_renamed":
                proposed_changes.suggested_updates.append({
                    "type": "add_naming_alias",
                    "subsection": diff.subsection,
                    "new_alias": diff.new_name,
                    "rationale": "User preferred this naming"
                })
            elif diff.type == "subsection_reordered":
                proposed_changes.suggested_updates.append({
                    "type": "adjust_order",
                    "subsection": diff.subsection,
                    "new_order": diff.new_order,
                    "rationale": "User found this order more logical"
                })

        # Store proposed changes for template maintainer review
        save_template_refinement_proposal(proposed_changes)

    return proposed_changes
```

### Architecture Changes

**New Commands**:
```
.claude/commands/
├── prodigy-ai-analyze-chapter-semantics.md  # NEW
├── prodigy-ai-organize-chapter.md           # NEW
├── prodigy-analyze-book-consistency.md      # NEW
└── prodigy-refine-template.md               # NEW
```

**Template Library**:
```
.prodigy/book-templates/
├── quick-start.yaml
├── configuration-reference.yaml
├── cli-reference.yaml
├── api-documentation.yaml
├── advanced-features.yaml
├── troubleshooting.yaml
└── examples-tutorials.yaml
```

**Workflow Integration** (`book-docs-drift.yml`):
```yaml
reduce:
  # Existing steps...
  - shell: "cd book && mdbook build"

  # NEW: AI-driven organization (replaces Spec 157's mechanical split)
  - claude: "/prodigy-ai-organize-chapters --book-dir $BOOK_DIR --strategy semantic --apply true"
    commit_required: true

  # NEW: Consistency check (optional)
  - claude: "/prodigy-analyze-book-consistency --book-dir $BOOK_DIR --report consistency-report.json"
```

### Data Structures

**Template Definition**:
```rust
#[derive(Serialize, Deserialize)]
struct ChapterTemplate {
    name: String,
    chapter_types: Vec<String>,
    subsection_structure: Vec<SubsectionTemplate>,
    naming_rules: Vec<NamingRule>,
    validation_rules: ValidationRules,
}

#[derive(Serialize, Deserialize)]
struct SubsectionTemplate {
    name: String,
    aliases: Vec<String>,
    topics: Vec<String>,
    order: usize,
    required: bool,
}

#[derive(Serialize, Deserialize)]
struct NamingRule {
    prefer: String,
    over: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct ValidationRules {
    required_subsections: Vec<String>,
    optional_subsections: Vec<String>,
    max_subsections: usize,
}
```

**Semantic Analysis Result**:
```rust
#[derive(Serialize, Deserialize)]
struct SemanticAnalysis {
    chapter: String,
    semantic_clusters: Vec<SemanticCluster>,
    embedding_model: String,
    analysis_date: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct SemanticCluster {
    name: String,
    sections: Vec<String>,
    topics: Vec<String>,
    rationale: String,
    confidence_score: f64,
}
```

### APIs and Interfaces

**AI Organization API**:
```bash
# Analyze semantics only (no changes)
/prodigy-ai-analyze-chapter-semantics \
  --chapter-file book/src/mapreduce.md \
  --output semantics.json

# Propose organization (dry-run)
/prodigy-ai-organize-chapter \
  --chapter-file book/src/mapreduce.md \
  --strategy semantic \
  --template auto \
  --dry-run true \
  --output proposal.json

# Apply organization
/prodigy-ai-organize-chapter \
  --chapter-file book/src/mapreduce.md \
  --strategy semantic \
  --template advanced-features \
  --apply true

# Check consistency across projects
/prodigy-analyze-book-consistency \
  --books "prodigy:book,debtmap:../debtmap/book" \
  --output consistency-report.json
```

## Dependencies

### Prerequisites
- **Spec 157**: mdBook Subsection Organization (file structure)
- **Spec 158**: Subsection-Aware Drift Detection (subsection definitions)

### Affected Components
- `.claude/commands/` (4 new commands)
- `.prodigy/book-templates/` (new template library)
- `workflows/book-docs-drift.yml` (updated reduce phase)

### External Dependencies
- **Embedding Model**: OpenAI embeddings API or local model (sentence-transformers)
- **Clustering**: scikit-learn, HDBSCAN, or similar
- **Topic Modeling**: LDA, NMF, or LLM-based topic extraction
- **YAML Parser**: For template definitions

## Testing Strategy

### Unit Tests
- Semantic clustering produces coherent groups
- Template matching identifies correct chapter type (>90% accuracy)
- Subsection naming follows template rules
- Consistency analysis detects deviations accurately

### Integration Tests

1. **Semantic Grouping Quality**:
   - Take MapReduce chapter with 15 H2 sections
   - Run semantic analysis
   - Verify clusters are thematically coherent
   - Compare to manual grouping (human baseline)

2. **Template Application**:
   - Create "Configuration" chapter
   - Match to configuration-reference template
   - Verify subsection names standardized
   - Verify order follows template

3. **Cross-Project Consistency**:
   - Generate books for 3 projects
   - Analyze consistency
   - Verify similar chapters use same structure
   - Identify and fix deviations

4. **Template Learning**:
   - Manually adjust AI-organized chapter
   - Run template refinement
   - Verify proposed template updates make sense
   - Apply updates and re-organize

### Performance Tests
- Semantic analysis completes in <30 seconds for 1000-line chapter
- Template matching completes in <5 seconds
- Consistency analysis for 3 projects completes in <1 minute

### User Acceptance
- AI-organized chapters rated as good or better than manual organization
- Consistency across projects improves user experience
- Template library covers 90% of common chapter types

## Documentation Requirements

### Code Documentation
- Document semantic analysis algorithm
- Explain template matching scoring
- Provide template creation guide
- Document consistency analysis metrics

### User Documentation
- Add "AI-Driven Organization" section to automated-documentation.md
- Provide template customization guide
- Explain how to override AI suggestions
- Document consistency enforcement benefits

### Template Documentation
- Document each template with examples
- Explain when to use each template
- Provide customization guidelines
- Include best practices for template design

## Implementation Notes

### Embedding Model Selection

**Options**:
1. **OpenAI Embeddings** (text-embedding-ada-002):
   - Pros: High quality, easy API
   - Cons: Cost, requires API key, external dependency

2. **Local Model** (sentence-transformers):
   - Pros: Free, no API limits, privacy
   - Cons: Requires local inference, slower

3. **Hybrid**: Use local for development, OpenAI for production

**Recommendation**: Start with OpenAI for quality, add local model option later.

### Clustering Strategy

**Algorithm**: HDBSCAN or K-Means
- HDBSCAN: Finds optimal number of clusters automatically
- K-Means: Requires specifying cluster count (3-8 typical)

**Tuning**:
- Too few clusters: Overly broad groupings
- Too many clusters: Fragmented, too many subsections

**Approach**: Start with K-Means (K=5), tune based on feedback.

### Template Evolution

**Bootstrap Phase**:
1. Create initial templates by analyzing high-quality docs:
   - Rust Book
   - mdBook Guide
   - Python docs
   - Prodigy docs (as they mature)

2. Extract patterns manually:
   - Common subsection names
   - Typical ordering
   - Required vs optional sections

**Continuous Improvement**:
1. Track template application success rate
2. Collect user feedback on AI organization
3. Learn from manual adjustments
4. Refine templates quarterly

### Cross-Project Learning

**Centralized Template Store**:
- Templates stored in prodigy repo (`.prodigy/book-templates/`)
- Shared across all projects using prodigy workflows
- Version controlled for reproducibility

**Project-Specific Overrides**:
- Projects can override templates locally
- Store overrides in `.prodigy/book-config.json`:
  ```json
  {
    "template_overrides": {
      "quick-start": "./custom-templates/my-quick-start.yaml"
    }
  }
  ```

## Migration and Compatibility

### Backward Compatibility

**Opt-In Feature**:
- AI organization is optional (triggered by command)
- Spec 157's mechanical splitting still works
- Can use AI for some chapters, manual for others

**No Breaking Changes**:
- Existing subsections not affected
- Templates applied only when requested
- Manual organization always takes precedence

### Migration Path

**Phase 1: Template Creation** (Weeks 1-2)
- Analyze existing high-quality documentation
- Create initial template library (5-7 templates)
- Test templates on prodigy book

**Phase 2: Single Project Validation** (Weeks 3-4)
- Apply AI organization to prodigy book
- Validate quality vs manual organization
- Refine templates based on results

**Phase 3: Multi-Project Rollout** (Weeks 5-8)
- Apply to debtmap, ripgrep books
- Analyze consistency across projects
- Enforce standardization where beneficial

**Phase 4: Continuous Improvement** (Ongoing)
- Collect feedback from users
- Refine templates based on usage
- Add new templates for new chapter types
- Improve clustering and matching algorithms

## Success Metrics

- **Organization Quality**: AI groupings rated ≥4/5 by users (vs manual baseline)
- **Consistency**: 90% of similar chapters across projects use same structure
- **Template Coverage**: 90% of chapters match a template
- **User Satisfaction**: Readers report improved navigation and clarity
- **Maintenance**: 50% reduction in manual reorganization effort

## Future Enhancements

### Multi-Language Support
- Generate consistent organization across different languages
- Translate subsection names while preserving structure
- Maintain cross-project consistency in all languages

### Visual Organization Tools
- Interactive UI for reviewing AI proposals
- Drag-and-drop subsection reordering
- Visual diff of before/after organization
- Template editor for custom templates

### Analytics-Driven Optimization
- Track which subsections users visit most
- Identify navigation pain points
- Optimize organization based on usage patterns
- A/B test different subsection structures

### Integration with Spec 158
- Use AI groupings to define subsection schema
- Populate `feature_mapping` automatically
- Suggest subsection topics based on semantic analysis
- Fully automated subsection definition generation
