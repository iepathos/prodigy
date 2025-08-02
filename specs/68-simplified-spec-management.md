# Specification 68: Simplified Spec Management System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current specification management system uses a manually maintained SPEC_INDEX.md file that tracks all specifications (draft, completed, deprecated). Analysis reveals significant maintenance issues:

- **Stale data**: Index shows 44 active specs but only 5 spec files exist
- **Missing entries**: Spec 65 exists but isn't in the index
- **Wrong status**: Spec 66 marked as "draft" but was implemented
- **High maintenance**: Manual updates required for every status change
- **Unclear source of truth**: Conflicts between index and reality

The current system's complexity exceeds its value, leading to inaccuracies that reduce trust in the specification system.

## Objective

Replace the manual SPEC_INDEX.md system with a simpler, file-based approach where:
- Only unimplemented specs exist as files
- Completed specs are removed after implementation
- Git history tracks what was implemented when
- Commands work directly with spec files instead of an index

## Requirements

### Functional Requirements
- Remove dependency on SPEC_INDEX.md from all commands
- Keep spec files only for unimplemented specifications
- Use consistent frontmatter in spec files for metadata
- Generate spec lists dynamically from existing files
- Track implementation history through git commits
- Update affected Claude commands to use new system

### Non-Functional Requirements
- Zero manual index maintenance
- Always accurate spec status
- Faster command execution (no index parsing)
- Cleaner repository structure
- Easier onboarding for new contributors

## Acceptance Criteria

- [ ] Commands no longer read or write SPEC_INDEX.md
- [ ] `/mmm-add-spec` creates spec files with embedded metadata
- [ ] `/mmm-implement-spec` works by reading spec files directly
- [ ] `/mmm-list-specs` (new) shows all unimplemented specs from files
- [ ] Completed specs are deleted as part of implementation workflow
- [ ] Migration plan executed for existing specs
- [ ] Documentation updated to reflect new workflow
- [ ] Git history preserves record of completed specifications

## Technical Details

### Implementation Approach

1. **Spec File Format Enhancement**
   ```markdown
   ---
   number: 68
   title: Simplified Spec Management System
   category: foundation
   priority: high
   status: draft
   dependencies: []
   created: 2024-01-15
   ---
   
   # Specification 68: Simplified Spec Management System
   
   [rest of spec content]
   ```

2. **Command Updates**
   - `/mmm-add-spec`: Generate next number by scanning existing files
   - `/mmm-implement-spec`: Read spec directly from file
   - `/mmm-list-specs`: Scan specs/ directory and parse frontmatter

3. **Workflow Changes**
   - When spec is implemented, delete the spec file
   - Commit message includes spec content for history
   - Use git tags or commits to track implementation

### Architecture Changes
- Remove all code that reads/writes SPEC_INDEX.md
- Add frontmatter parser for spec files
- Add directory scanner for spec discovery

### Data Structures
```rust
struct SpecMetadata {
    number: u32,
    title: String,
    category: String,
    priority: String,
    status: String,
    dependencies: Vec<u32>,
    created: String,
}

struct Spec {
    metadata: SpecMetadata,
    content: String,
}
```

### APIs and Interfaces
- `list_specs() -> Vec<SpecMetadata>`: Scan and parse all spec files
- `read_spec(number: u32) -> Option<Spec>`: Read specific spec file
- `next_spec_number() -> u32`: Find highest number + 1
- `delete_spec(number: u32)`: Remove spec file after implementation

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `/mmm-add-spec` command
  - `/mmm-implement-spec` command
  - `/mmm-batch-implement` command (if it exists)
  - Any other commands referencing SPEC_INDEX.md
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**: 
  - Frontmatter parsing
  - Spec number generation
  - Directory scanning
- **Integration Tests**: 
  - Full workflow: create → implement → delete
  - Command execution without SPEC_INDEX.md
- **Migration Tests**: 
  - Verify all existing specs properly migrated
  - Ensure no data loss during transition

## Documentation Requirements

- **Code Documentation**: Document new spec file format
- **User Documentation**: Update README with new workflow
- **Command Documentation**: Update all affected command descriptions
- **Migration Guide**: Document transition process for existing specs

## Implementation Notes

### Migration Strategy

1. **Phase 1: Prepare**
   - Create frontmatter for existing 5 spec files
   - Archive SPEC_INDEX.md to SPEC_INDEX.archive.md
   
2. **Phase 2: Update Commands**
   - Modify commands to use file-based approach
   - Add fallback to read archived index if needed
   
3. **Phase 3: Cleanup**
   - Remove completed spec references
   - Delete SPEC_INDEX.archive.md after verification
   
### Benefits Over Current System

1. **Accuracy**: File existence = spec status (no sync issues)
2. **Simplicity**: No manual maintenance required
3. **Speed**: Direct file access instead of parsing large index
4. **Trust**: What you see is what exists
5. **History**: Git log shows complete implementation history

### Example Workflow

```bash
# Create new spec
mmm cook specs --map "mmm-add-spec" --args "Add new feature X"
# Creates: specs/69-add-new-feature-x.md

# List all pending specs
mmm cook specs --map "mmm-list-specs"
# Output: 45, 61, 65, 66, 67, 68, 69

# Implement spec
mmm cook specs --map "mmm-implement-spec" --args "68"
# Implements spec and deletes specs/68-simplified-spec-management.md

# View implementation history
git log --grep="implement spec 68"
```

## Migration and Compatibility

### Breaking Changes
- SPEC_INDEX.md will no longer be maintained
- Commands expecting index file will need updates
- Existing automation relying on index structure will break

### Migration Path
1. Run one-time migration to add frontmatter to existing specs
2. Update all commands to use new system
3. Archive SPEC_INDEX.md with final state
4. Document completed specs in git history

### Backward Compatibility
- Archived SPEC_INDEX.md preserves historical information
- Git history maintains complete audit trail
- No loss of information, just change in storage