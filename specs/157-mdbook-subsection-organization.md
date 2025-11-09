---
number: 157
title: mdBook Subsection Organization for Large Chapters
category: optimization
priority: medium
status: draft
dependencies: [154, 156]
created: 2025-01-11
---

# Specification 157: mdBook Subsection Organization for Large Chapters

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 154 (mdBook Version Selector UI), Spec 156 (Version-Aware Book Workflow)

## Context

The current book documentation workflow (`book-docs-drift.yml`) generates comprehensive single-file chapters that can become very large (e.g., `mapreduce.md` is 1038 lines). This creates several problems:

1. **Poor User Experience**: Large chapters are difficult to navigate and overwhelming for readers
2. **Inconsistent Structure**: Our books look flat compared to well-organized books like the Rust Book, which uses numbered subsections (1.1, 1.2, etc.)
3. **Maintenance Challenges**: Large files are harder to review, update, and maintain
4. **Limited Granularity**: Cannot track drift at subsection level, only entire chapters

The workflow currently treats each chapter as an atomic unit:
- `prodigy-chapters.json` defines one file path per chapter
- Map phase processes one chapter per agent
- No concept of subsections or nested structure exists
- `SUMMARY.md` shows flat list without hierarchy

### Comparison: Current vs Desired Structure

**Current (Flat)**:
```markdown
# User Guide
- [MapReduce Workflows](mapreduce.md)  # 1038 lines!
- [Command Types](commands.md)         # 398 lines
```

**Desired (Hierarchical)**:
```markdown
# User Guide
- [MapReduce Workflows](mapreduce/index.md)
  - [Quick Start](mapreduce/index.md#quick-start)
  - [Complete Structure](mapreduce/structure.md)
  - [Checkpoint and Resume](mapreduce/checkpoint.md)
  - [Dead Letter Queue](mapreduce/dlq.md)
  - [Performance Tuning](mapreduce/performance.md)
  - [Examples](mapreduce/examples.md)
```

## Objective

Implement automatic subsection organization for large mdBook chapters to improve readability, maintainability, and user experience by splitting monolithic chapters into logical subsections while preserving links, cross-references, and workflow automation.

## Requirements

### Functional Requirements

1. **Automatic Chapter Splitting**
   - Detect chapters exceeding size/complexity thresholds
   - Split chapters into logical subsections based on H2 headings
   - Create subdirectory structure with index.md and subsection files
   - Preserve all content, code examples, and formatting

2. **SUMMARY.md Management**
   - Update SUMMARY.md with nested structure
   - Maintain proper indentation for subsections
   - Preserve chapter ordering and organization
   - Support both numbered and unnumbered sections

3. **Link Preservation**
   - Update internal cross-references when files move
   - Fix anchor links to subsections
   - Preserve external links unchanged
   - Validate all links after reorganization

4. **Workflow Integration**
   - Add subsection organization to book-docs-drift.yml reduce phase
   - Make splitting configurable via thresholds
   - Support dry-run mode for preview
   - Commit changes with descriptive messages

5. **Reversibility**
   - Support merging subsections back into single file if needed
   - Preserve git history through file moves
   - Allow manual override of automatic decisions

### Non-Functional Requirements

1. **Performance**: Complete splitting for typical book (<50 chapters) in <30 seconds
2. **Safety**: Never lose content or break mdbook build
3. **Idempotency**: Running multiple times produces same result
4. **Configurability**: Thresholds and splitting strategy customizable

## Acceptance Criteria

- [ ] Command `/prodigy-auto-organize-chapters` created and functional
- [ ] Chapters >400 lines with 6+ H2 sections automatically split into subsections
- [ ] Subdirectory structure created with index.md containing overview
- [ ] SUMMARY.md updated with properly indented subsection hierarchy
- [ ] All internal links and cross-references work after reorganization
- [ ] mdbook build succeeds after splitting without warnings
- [ ] Workflow updated with reduce phase step for automatic organization
- [ ] Dry-run mode allows previewing changes without applying them
- [ ] Git commits include moved files properly (git mv detection)
- [ ] Documentation updated with splitting strategy and configuration options
- [ ] Large existing chapters (mapreduce.md, commands.md) successfully split
- [ ] Subsection organization respects manually created structure (doesn't overwrite)

## Technical Details

### Implementation Approach

#### 1. Create `/prodigy-auto-organize-chapters` Command

**Command Parameters**:
```bash
/prodigy-auto-organize-chapters \
  --book-dir book \
  --min-h2-sections 6 \
  --min-lines 400 \
  --dry-run false \
  --preserve-index-sections 2
```

**Algorithm**:
1. Scan all markdown files in `book/src/`
2. Parse each file to extract:
   - Total line count
   - H2 heading count and content
   - H3+ nesting under each H2
   - Cross-references and links
3. Identify chapters exceeding thresholds
4. For each chapter to split:
   - Create subdirectory: `book/src/{chapter-name}/`
   - Create index.md with intro + quick start (first N H2 sections)
   - Create subsection files for remaining H2 sections
   - Update cross-references and links
   - Update SUMMARY.md with nested structure
5. Validate all changes:
   - Run mdbook build
   - Check for broken links
   - Verify no lost content

#### 2. Splitting Strategy

**Threshold Decision Matrix**:

| Criteria | Action |
|----------|--------|
| <300 lines | Keep as single file |
| 300-600 lines, <6 H2s | Keep as single file |
| >400 lines, 6+ H2s | **Split into subsections** |
| >600 lines, any H2s | **Split into subsections** |
| >800 lines | **Always split** |

**Subsection Grouping**:
- **Index.md**: Overview + Quick Start (first 1-2 H2 sections)
- **Individual subsections**: Each major H2 becomes its own file
- **Combined subsections**: Small related H2s (<50 lines) grouped together
- **Preserve nesting**: H3/H4 under H2 stay in same subsection file

**Example Split: mapreduce.md**:
```
Original: book/src/mapreduce.md (1038 lines, 20+ H2 sections)

After split:
book/src/mapreduce/
├── index.md              # Quick Start + Complete Structure (~150 lines)
├── environment.md        # Environment Variables (~50 lines)
├── backoff.md            # Backoff Strategies (~60 lines)
├── setup-phase.md        # Setup Phase (Advanced) (~80 lines)
├── checkpoint-resume.md  # Checkpoint and Resume (~100 lines)
├── dlq.md                # Dead Letter Queue (~80 lines)
├── performance.md        # Performance Tuning (~120 lines)
├── examples.md           # Real-World Use Cases (~150 lines)
└── troubleshooting.md    # Common Pitfalls + Troubleshooting (~100 lines)
```

#### 3. SUMMARY.md Update Algorithm

**Before**:
```markdown
- [MapReduce Workflows](mapreduce.md)
```

**After**:
```markdown
- [MapReduce Workflows](mapreduce/index.md)
  - [Environment Variables](mapreduce/environment.md)
  - [Backoff Strategies](mapreduce/backoff.md)
  - [Setup Phase](mapreduce/setup-phase.md)
  - [Checkpoint and Resume](mapreduce/checkpoint-resume.md)
  - [Dead Letter Queue](mapreduce/dlq.md)
  - [Performance Tuning](mapreduce/performance.md)
  - [Real-World Use Cases](mapreduce/examples.md)
  - [Troubleshooting](mapreduce/troubleshooting.md)
```

**Algorithm**:
1. Parse SUMMARY.md to identify line for split chapter
2. Replace single line with nested structure
3. Indent subsections with 2 spaces
4. Preserve surrounding structure
5. Validate markdown syntax

#### 4. Link Preservation

**Cross-Reference Update Strategy**:

1. **Internal links to moved content**:
   - Before: `[See MapReduce](mapreduce.md#checkpoint-resume)`
   - After: `[See MapReduce](mapreduce/checkpoint-resume.md)`

2. **Anchor links within same file**:
   - Before: `[See below](#setup-phase)` (in mapreduce.md)
   - After: `[See Setup Phase](setup-phase.md)` (in mapreduce/index.md)

3. **Links from other chapters**:
   - Scan all markdown files for references
   - Update paths to reflect new structure
   - Validate all links resolve

**Link Update Algorithm**:
```rust
// Pseudo-code
for each markdown file in book:
    for each link in file:
        if link points to split chapter:
            if link has anchor:
                determine which subsection contains anchor
                update link to subsection file
            else:
                update link to index.md
```

### Architecture Changes

**New Command Structure**:
```
.claude/commands/
└── prodigy-auto-organize-chapters.md
```

**Modified Files**:
```
workflows/book-docs-drift.yml  # Add reduce phase step
```

**Generated Structure**:
```
book/src/
├── {chapter}/
│   ├── index.md           # Chapter overview
│   ├── subsection-1.md
│   ├── subsection-2.md
│   └── ...
└── SUMMARY.md             # Updated with nested structure
```

### Data Structures

**Chapter Analysis Result**:
```rust
struct ChapterAnalysis {
    file_path: PathBuf,
    line_count: usize,
    h2_sections: Vec<Section>,
    should_split: bool,
    split_strategy: SplitStrategy,
}

struct Section {
    title: String,
    level: u8,              // 2 for H2, 3 for H3, etc.
    start_line: usize,
    end_line: usize,
    content: String,
    subsections: Vec<Section>,
}

enum SplitStrategy {
    NoSplit,
    IndexPlusSubsections {
        index_sections: Vec<usize>,  // Which H2s go in index.md
        subsection_files: Vec<SubsectionFile>,
    },
}

struct SubsectionFile {
    filename: String,
    title: String,
    sections: Vec<Section>,  // H2s included in this file
}
```

**Link Update Mapping**:
```rust
struct LinkMapping {
    old_path: String,
    new_path: String,
    anchor_mappings: HashMap<String, String>,
}
```

### APIs and Interfaces

**Command Interface**:
```yaml
- claude: "/prodigy-auto-organize-chapters --book-dir book --min-h2-sections 6 --min-lines 400 --dry-run false"
  commit_required: true
```

**Configuration Options** (future enhancement):
```yaml
# .prodigy/book-config.json
{
  "subsection_organization": {
    "enabled": true,
    "min_h2_sections": 6,
    "min_lines": 400,
    "force_split_lines": 800,
    "preserve_index_sections": 2,
    "group_small_subsections": true,
    "small_subsection_threshold": 50
  }
}
```

## Dependencies

### Prerequisites
- **Spec 154**: mdBook version selector UI (book infrastructure must exist)
- **Spec 156**: Version-aware book workflow (workflow automation foundation)

### Affected Components
- `.claude/commands/prodigy-auto-organize-chapters.md` (new)
- `workflows/book-docs-drift.yml` (modified)
- `book/src/SUMMARY.md` (modified by command)
- All large chapter files in `book/src/` (split into subdirectories)

### External Dependencies
- mdbook: Must support nested directory structure (already does)
- Markdown parser: For extracting sections and links
- Git: For proper file move tracking

## Testing Strategy

### Unit Tests
- Parse markdown files and extract H2 sections correctly
- Identify chapters exceeding thresholds
- Generate correct subsection file structure
- Update SUMMARY.md with proper indentation
- Update cross-references and links accurately

### Integration Tests
1. **Split Large Chapter Test**:
   - Start with single 1000-line file with 10 H2 sections
   - Run organization command
   - Verify subdirectory created with index.md + subsections
   - Verify SUMMARY.md updated correctly
   - Verify mdbook builds successfully

2. **Link Preservation Test**:
   - Create chapter with internal links and cross-references
   - Split chapter into subsections
   - Verify all links resolve correctly
   - Run mdbook build and check for broken links

3. **Idempotency Test**:
   - Run organization command twice
   - Verify second run detects already-split chapters
   - Verify no duplicate directories or broken structure

4. **Dry-Run Test**:
   - Run with --dry-run true
   - Verify preview output shows planned changes
   - Verify no files actually modified

### Performance Tests
- Organize book with 50 chapters in <30 seconds
- Split 1000-line chapter with 20 sections in <5 seconds
- Update 100 cross-references in <2 seconds

### User Acceptance
- Generate books from multiple projects (prodigy, ripgrep)
- Verify subsection structure improves navigation
- Confirm no content lost during splitting
- Validate links work in generated HTML

## Documentation Requirements

### Code Documentation
- Document splitting algorithm in command file
- Explain threshold decision matrix
- Document link update strategy
- Include examples of subsection grouping

### User Documentation
- Add section to automated-documentation.md explaining subsection organization
- Document configuration options and thresholds
- Provide examples of split vs non-split chapters
- Explain how to manually adjust subsection structure

### Architecture Updates
- Update CLAUDE.md with command description
- Document subsection organization in book workflow section
- Explain when and why splitting occurs

## Implementation Notes

### Splitting Heuristics

**Preserve in Index.md**:
- Title and introduction
- Quick Start section
- Overview or "What is X" section
- First 1-2 H2 sections (configurable)

**Extract to Subsections**:
- Configuration details
- Advanced features
- Troubleshooting
- Examples and use cases
- Reference material

**Grouping Small Sections**:
- If H2 section is <50 lines, consider combining with related section
- Group related troubleshooting items
- Combine setup + configuration if both small

### Link Update Edge Cases

**Anchor Links Within Same Subsection**:
- Keep as-is (don't need path update)
- Example: `#setup` in setup-phase.md → no change

**Links to Index Overview**:
- Links to chapter without anchor → point to index.md
- Example: `[MapReduce](mapreduce.md)` → `[MapReduce](mapreduce/index.md)`

**External Links**:
- Never modify (preserve as-is)
- Example: `https://docs.rs/...` → no change

### Git Best Practices

**File Moves**:
- Use `git mv` semantics by:
  1. Reading original file
  2. Deleting original
  3. Creating new files in subdirectory
- Git will detect similarity and preserve history

**Commit Message**:
```
docs: organize {chapter-name} into subsections

Split large chapter into logical subsections:
- index.md: Overview and quick start
- {subsection-1}.md: {description}
- {subsection-2}.md: {description}

Updated SUMMARY.md with nested structure and fixed cross-references.
```

### Error Handling

**mdbook Build Failure**:
- Detect build errors after splitting
- Provide detailed error message
- Offer rollback option
- Log problematic files

**Broken Links**:
- Validate all links before committing
- Report which links would break
- Offer to fix automatically or abort

**Duplicate Subsections**:
- Detect existing subsection structure
- Skip chapters already organized
- Warn if manual structure conflicts with automatic

## Migration and Compatibility

### Backward Compatibility

**Existing Books**:
- Support both flat and nested chapter structures
- Don't break existing SUMMARY.md formats
- Preserve manually created subsections

**Version Support**:
- Works with mdbook 0.4.x and later
- No special mdbook features required
- Standard markdown file structure

### Migration Path

**Phase 1: Immediate (Spec 157)**:
- Implement automatic splitting command
- Add to book-docs-drift.yml reduce phase
- Split existing large chapters in prodigy book

**Phase 2: Future Enhancement (Spec 158)**:
- Add subsection support to `prodigy-chapters.json` schema
- Update drift detection to analyze subsections separately
- Enable subsection-level drift fixes

**Phase 3: Advanced Features (Future)**:
- Automatic subsection generation during gap detection
- Configurable splitting strategies per project
- AI-driven section grouping and organization

### Breaking Changes

**None**: This is purely additive functionality that enhances existing chapters without breaking compatibility.

### Migration Steps

1. **For Existing Books**:
   ```bash
   # Run organization once to split large chapters
   cd book
   /prodigy-auto-organize-chapters --book-dir . --dry-run false
   git add -A
   git commit -m "docs: organize large chapters into subsections"
   ```

2. **For New Books**:
   - Organization runs automatically in reduce phase
   - No manual intervention needed

3. **Manual Overrides**:
   - If you've manually organized a chapter, command skips it
   - Detect by checking for existing subdirectory with index.md

## Success Metrics

- **Readability**: Average chapter length reduced from 500+ to <300 lines
- **Navigation**: Users can find specific subsections without searching entire chapter
- **Maintenance**: Subsection-level updates easier than monolithic chapter edits
- **Consistency**: Book structure matches industry standards (Rust Book, mdBook Guide)
- **Automation**: No manual intervention needed for subsection organization

## Future Enhancements

### Spec 158: Subsection-Aware Drift Detection
- Update `prodigy-chapters.json` to support subsections
- Analyze drift at subsection granularity
- Fix subsections independently in map phase

### Spec 159: Smart Subsection Grouping
- Use AI to determine optimal subsection boundaries
- Analyze content semantics, not just H2 headings
- Suggest logical groupings based on topic clustering

### Spec 160: Interactive Subsection Management
- CLI tool to preview and adjust subsection structure
- Interactive mode to approve/reject splits
- Visual diff of before/after organization
