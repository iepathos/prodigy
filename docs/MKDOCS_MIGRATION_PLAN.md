# MkDocs Material Migration Plan

This document outlines the strategy for creating a parallel MkDocs Material documentation system alongside the existing mdbook setup.

## Overview

**Goal**: Create a Material MkDocs workflow that mirrors the functionality of the existing `book-docs-drift.yml` workflow but targets MkDocs Material instead of mdbook.

**Strategy**: Parallel systems that can coexist, allowing for gradual migration or dual documentation output.

## Architecture Comparison

### Current (mdbook)
```
book/
├── src/                    # Source markdown files
│   ├── SUMMARY.md         # Navigation structure (markdown)
│   ├── intro.md
│   ├── workflow-basics/
│   └── mapreduce/
├── book.toml              # Configuration (TOML)
└── book/                  # Build output

Workflow: workflows/book-docs-drift.yml
Config: .prodigy/book-config.json
Chapters: workflows/data/prodigy-chapters.json
```

### Proposed (MkDocs Material)
```
docs/                      # Source markdown files
├── index.md              # Home page
├── getting-started/
├── workflow-basics/
└── mapreduce/
mkdocs.yml                # Configuration + Navigation (YAML)
site/                     # Build output

Workflow: workflows/mkdocs-docs-drift.yml
Config: .prodigy/mkdocs-config.json
Chapters: workflows/data/mkdocs-chapters.json
```

## Key Differences

### 1. Navigation Structure
- **mdbook**: Uses `SUMMARY.md` (markdown list)
- **MkDocs**: Uses `nav:` section in `mkdocs.yml` (YAML structure)

**Impact**: Commands must parse and update YAML instead of markdown.

### 2. File Organization
- **mdbook**: Chapters can be files or directories with subsections
- **MkDocs**: Pages can be grouped in sections (nested navigation)

**Impact**: Similar structure, but navigation is defined differently.

### 3. Configuration Format
- **mdbook**: `book.toml` (TOML)
- **MkDocs**: `mkdocs.yml` (YAML with Material theme options)

**Impact**: Different parsing logic for configuration.

### 4. Build Validation
- **mdbook**: `mdbook build`
- **MkDocs**: `mkdocs build --strict`

**Impact**: Different error messages and validation patterns.

### 5. Markdown Features
**Material MkDocs offers:**
- Admonitions (info, warning, danger boxes)
- Content tabs
- Code annotations
- Dark mode by default
- Better search
- Git integration features

**Impact**: Drift detection should recognize MkDocs-specific syntax.

## New Claude Commands Required

### Essential Commands (Must Have)

#### 1. `/prodigy-detect-mkdocs-gaps`
**Purpose**: Detect documentation gaps and create missing MkDocs pages.

**Key Differences from mdbook version**:
- Parse `mkdocs.yml` instead of `SUMMARY.md`
- Update `nav:` section in YAML format
- Create pages in `docs/` instead of `book/src/`
- Generate `mkdocs-chapters.json` with section structure

**Logic Changes**:
```bash
# Instead of updating SUMMARY.md:
# - [New Chapter](new-chapter.md)

# Update mkdocs.yml nav:
nav:
  - Section Name:
      - New Chapter: section/new-chapter.md
```

#### 2. `/prodigy-analyze-mkdocs-drift`
**Purpose**: Analyze a MkDocs page for drift against codebase.

**Key Differences**:
- Read from `docs/` directory
- Parse MkDocs Material-specific syntax:
  - Admonitions: `!!! note` instead of blockquotes
  - Tabs: `=== "Tab Title"`
  - Code blocks with line highlighting
- Use `mkdocs-chapters.json` for structure

**Similar to mdbook version**:
- Feature comparison logic remains the same
- Drift detection patterns are similar
- JSON output format can be identical

#### 3. `/prodigy-fix-mkdocs-drift`
**Purpose**: Fix drift in a MkDocs page.

**Key Differences**:
- Use MkDocs Material syntax for enhancements:
  ```markdown
  !!! warning "Important Change"
      This configuration option has changed.

  === "YAML"
      ```yaml
      config: value
      ```

  === "TOML"
      ```toml
      config = "value"
      ```
  ```
- Update cross-references using MkDocs format
- Preserve Material theme features

#### 4. `/prodigy-validate-mkdocs-holistically`
**Purpose**: Perform holistic validation of MkDocs documentation.

**Validation Steps**:
1. Run `mkdocs build --strict` to catch errors
2. Validate `mkdocs.yml` structure
3. Check for broken internal links
4. Verify all pages are in navigation
5. Check for orphaned pages
6. Validate Material theme configuration

#### 5. `/prodigy-fix-mkdocs-build-errors`
**Purpose**: Fix MkDocs build errors.

**Common Errors to Handle**:
- Broken links: `WARNING - Doc file 'page.md' contains a link to 'missing.md'`
- Missing pages in nav
- Invalid YAML syntax in `mkdocs.yml`
- Invalid markdown extensions
- Theme configuration errors

### Nice-to-Have Commands

#### 6. `/prodigy-convert-mdbook-to-mkdocs`
**Purpose**: One-time migration from mdbook to MkDocs.

**Tasks**:
1. Convert `SUMMARY.md` to `mkdocs.yml` nav structure
2. Copy `book/src/` to `docs/`
3. Transform mdbook-specific syntax to MkDocs Material:
   - Convert blockquote notes to admonitions
   - Update code block annotations
   - Fix cross-references
4. Generate initial `mkdocs.yml` from `book.toml`
5. Create `mkdocs-chapters.json` from `prodigy-chapters.json`

#### 7. `/prodigy-sync-mkdocs-nav`
**Purpose**: Synchronize `mkdocs.yml` navigation with actual file structure.

**Tasks**:
1. Scan `docs/` directory
2. Find all `.md` files
3. Compare against `nav:` section in `mkdocs.yml`
4. Add missing pages to navigation
5. Remove references to deleted pages
6. Maintain section structure

#### 8. `/prodigy-analyze-features-for-mkdocs`
**Purpose**: Analyze codebase features for MkDocs documentation.

**Differences**:
- Output to `.prodigy/mkdocs-analysis/features.json`
- Use MkDocs-specific paths and structure
- Otherwise identical to mdbook version

## Implementation Phases

### Phase 1: Foundation (Week 1)
**Goal**: Create basic workflow infrastructure

**Tasks**:
1. ✅ Create `workflows/mkdocs-docs-drift.yml`
2. ✅ Create `.prodigy/mkdocs-config.json`
3. ✅ Create `workflows/data/mkdocs-chapters.json`
4. ✅ Create example `mkdocs.yml`
5. Create initial `docs/` directory structure
6. Set up Material theme

**Deliverables**:
- Working MkDocs site (even if minimal)
- Build pipeline (`mkdocs build` works)
- Basic navigation structure

### Phase 2: Core Commands (Week 2)
**Goal**: Implement essential drift detection commands

**Tasks**:
1. Create `/prodigy-detect-mkdocs-gaps`
   - Parse `mkdocs.yml` navigation
   - Update YAML structure
   - Generate stub pages in `docs/`

2. Create `/prodigy-analyze-mkdocs-drift`
   - Read MkDocs pages
   - Parse Material syntax
   - Compare against features

3. Create `/prodigy-fix-mkdocs-drift`
   - Fix drift using Material syntax
   - Update cross-references
   - Preserve theme features

**Deliverables**:
- Working gap detection
- Working drift analysis
- Working drift fixing

### Phase 3: Validation (Week 3)
**Goal**: Implement validation and error handling

**Tasks**:
1. Create `/prodigy-validate-mkdocs-holistically`
2. Create `/prodigy-fix-mkdocs-build-errors`
3. Add validation to workflow reduce phase
4. Test with real Prodigy codebase

**Deliverables**:
- Complete validation pipeline
- Error recovery mechanisms
- Production-ready workflow

### Phase 4: Enhancement (Week 4)
**Goal**: Add nice-to-have features

**Tasks**:
1. Create `/prodigy-convert-mdbook-to-mkdocs` (if migrating)
2. Create `/prodigy-sync-mkdocs-nav`
3. Add MkDocs-specific features:
   - Search optimization
   - Dark mode testing
   - Mobile responsiveness checks
4. Documentation for using both systems

**Deliverables**:
- Migration tools (if needed)
- Enhanced workflow features
- Complete documentation

## Workflow Comparison

### mdbook Workflow
```yaml
setup:
  - Analyze features → features.json
  - Detect gaps → Update SUMMARY.md

map:
  - Analyze chapter drift
  - Fix chapter drift

reduce:
  - mdbook build
  - Validate holistically
```

### MkDocs Workflow
```yaml
setup:
  - Analyze features → features.json
  - Detect gaps → Update mkdocs.yml nav

map:
  - Analyze page drift
  - Fix page drift

reduce:
  - mkdocs build --strict
  - Validate holistically
```

**Key Insight**: The workflow structure is nearly identical. The main differences are:
1. YAML manipulation instead of markdown
2. Different build commands
3. Different markdown syntax (Material-specific)

## Data Structure Changes

### Chapter Definition Format

**mdbook (prodigy-chapters.json)**:
```json
{
  "chapters": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "type": "multi-subsection",
      "index_file": "book/src/workflow-basics/index.md",
      "subsections": [...]
    }
  ]
}
```

**MkDocs (mkdocs-chapters.json)**:
```json
{
  "pages": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "type": "section",
      "pages": [
        {
          "id": "workflow-structure",
          "title": "Workflow Structure",
          "file": "docs/workflow-basics/workflow-structure.md"
        }
      ]
    }
  ]
}
```

**Key Changes**:
- `chapters` → `pages`
- `subsections` → nested `pages`
- `type: "multi-subsection"` → `type: "section"`
- `index_file` → implicit `index.md` in section

## Command Implementation Strategy

### Option 1: Duplicate and Modify (Recommended)
**Approach**: Copy existing mdbook commands and modify for MkDocs.

**Pros**:
- Fastest implementation
- Independent evolution
- No risk to existing workflow
- Clear separation of concerns

**Cons**:
- Code duplication
- Maintenance of two similar codebases

**Recommendation**: Start with this approach for rapid prototyping.

### Option 2: Parameterized Commands
**Approach**: Create generic commands that accept `--format mdbook|mkdocs`.

**Pros**:
- Single codebase
- Shared logic
- Easier maintenance

**Cons**:
- More complex implementation
- Risk of breaking existing workflow
- Higher initial effort

**Recommendation**: Consider for Phase 4 refactoring.

### Option 3: Shared Library with Format Adapters
**Approach**: Extract common logic into library, create format adapters.

**Pros**:
- Best code reuse
- Clean architecture
- Testable components

**Cons**:
- Significant refactoring needed
- Higher complexity
- Longer development time

**Recommendation**: Future enhancement after both systems are stable.

## Testing Strategy

### Unit Testing
- YAML parsing and updating
- Navigation structure generation
- Material syntax detection
- Feature comparison logic

### Integration Testing
- Full workflow execution
- Gap detection → drift fixing → validation
- Build success verification
- Cross-reference integrity

### Validation Testing
- Run on real Prodigy codebase
- Compare mdbook vs MkDocs output
- Verify feature parity
- Test error handling

## Migration Considerations

### Coexistence Strategy
Both systems can run simultaneously:
- `workflows/book-docs-drift.yml` → mdbook
- `workflows/mkdocs-docs-drift.yml` → MkDocs Material

**Shared Resources**:
- Same features.json from codebase analysis
- Same feature detection logic
- Same validation criteria

**Separate Resources**:
- Different output directories (`book/` vs `docs/`)
- Different chapter definitions
- Different configuration files

### Content Synchronization
**Option 1**: Manual sync (recommended initially)
- Run both workflows independently
- Content diverges naturally over time
- Choose preferred format for each section

**Option 2**: Shared source with converters
- Keep source in neutral format
- Generate both outputs
- More complex but maintains consistency

**Option 3**: Convert on demand
- Use `/prodigy-convert-mdbook-to-mkdocs` when needed
- Periodic synchronization
- Accept some drift between formats

## Dependencies

### Required Tools
```bash
# MkDocs Material
pip install mkdocs-material

# Optional plugins
pip install mkdocs-git-revision-date-localized-plugin
pip install mkdocs-minify-plugin
pip install mkdocs-redirects
```

### Configuration Files
- `mkdocs.yml` - Main configuration
- `.prodigy/mkdocs-config.json` - Workflow configuration
- `workflows/data/mkdocs-chapters.json` - Chapter definitions
- `requirements.txt` - Python dependencies for MkDocs

## Next Steps

1. **Review this plan** with team/stakeholders
2. **Decide on strategy**:
   - Parallel systems (both mdbook + MkDocs)
   - Full migration to MkDocs
   - Experimental MkDocs only
3. **Start Phase 1**: Create foundation
4. **Implement core commands** following the phase plan
5. **Test with real content** from Prodigy
6. **Iterate and refine** based on results

## Questions to Answer

1. **Should we maintain both formats long-term?**
   - Pros: Flexibility, different audiences
   - Cons: Maintenance burden

2. **Should we convert existing mdbook content?**
   - Use `/prodigy-convert-mdbook-to-mkdocs`?
   - Start fresh with MkDocs?
   - Maintain both independently?

3. **What features are must-haves in MkDocs?**
   - Admonitions?
   - Tabs?
   - Dark mode?
   - Versioning (mike)?

4. **How to handle versioned docs?**
   - MkDocs supports versioning with `mike`
   - mdbook has different versioning approach
   - Need unified strategy?

## Resources

- [MkDocs Material Documentation](https://squidfunk.github.io/mkdocs-material/)
- [MkDocs Documentation](https://www.mkdocs.org/)
- [Material Theme Reference](https://squidfunk.github.io/mkdocs-material/reference/)
- [MkDocs Plugins](https://github.com/mkdocs/mkdocs/wiki/MkDocs-Plugins)
- [Mike (Versioning)](https://github.com/jimporter/mike)

## Conclusion

Creating a MkDocs Material workflow is highly feasible using the existing mdbook workflow as a template. The core logic remains similar; the main changes are in:

1. **Navigation format** (YAML vs Markdown)
2. **Build commands** (`mkdocs` vs `mdbook`)
3. **Markdown syntax** (Material features)

The recommended approach is to create parallel workflows initially, allowing both systems to coexist. This provides flexibility and reduces risk while exploring MkDocs Material's capabilities.

**Total Effort Estimate**: 3-4 weeks for a production-ready MkDocs workflow with all essential features.
