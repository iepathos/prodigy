---
number: 61
title: Consistent CLI Output Formatting
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-09-06
---

# Specification 61: Consistent CLI Output Formatting

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The current CLI output implementation exhibits several formatting inconsistencies that affect user experience and readability:

1. **Inconsistent Icon Placement**: Output messages display icons with varying spacing patterns, sometimes with extra spaces after the icon (`ℹ️  ` with two spaces) and sometimes with embedded icons in formatted strings leading to alignment issues.

2. **Mixed Formatting Approaches**: Different parts of the codebase format messages differently:
   - `ProgressDisplayImpl` in `display.rs` uses `"ℹ️  {message}"` format with two spaces
   - `orchestrator.rs` embeds icons directly in messages like `"📝 Adding..."` with single space
   - Some messages include icons in the format string while others add them in the display layer

3. **Inconsistent Message Prefixes**: Various message types use different patterns:
   - Info messages: `ℹ️  ` (with two spaces)
   - Progress messages: `🔄 ` (with single space)
   - Custom formatted messages: `📝 Adding`, `📋 Total inputs`, etc.

4. **Duplicated Icons**: When messages with embedded icons are passed to display methods that add their own icons, users see duplicate icons like:
   ```
   ℹ️  📝 Adding 1 direct arguments from --args
   ```

5. **Alignment Issues**: The inconsistent spacing creates visual misalignment in the output, making it harder to scan and read.

Example of current problematic output:
```
ℹ️  Created worktree at: /Users/glen/.prodigy/worktrees/...
ℹ️  Processing workflow with arguments or file patterns
ℹ️  📝 Adding 1 direct arguments from --args
ℹ️  📋 Total inputs to process: 1
ℹ️
🔄 Processing input 1/1: 73
ℹ️  Executing workflow: args-workflow (max 1 iterations)
🔄 Starting iteration 1/1
🔄 Executing step 1/3: claude: /prodigy-implement-spec $ARG
🔄 Running validation (Claude): /prodigy-validate-spec 73 --output .prodigy/validation-result.json
⚠️  Validation incomplete: 91.0% complete (threshold: 100.0%)
ℹ️  Attempting to complete implementation (attempt 1/3)
🔄 Running recovery step: claude: /prodigy-complete-spec $ARG --gaps ${validation.gaps}
```

## Objective

Establish and implement a consistent, clean CLI output formatting system that:
1. Eliminates duplicate icons and inconsistent spacing
2. Provides clear visual hierarchy through consistent formatting
3. Separates presentation concerns from business logic
4. Maintains readability across all output types

## Requirements

### Functional Requirements

1. **Consistent Icon System**
   - Define a single source of truth for all message type icons
   - Ensure icons are only added at the display layer, never in message content
   - Standardize spacing after icons (single space consistently)
   - Create icon categories: info, warning, error, progress, success, action

2. **Message Formatting Rules**
   - All display methods receive plain text messages without icons
   - Icons are added consistently by the display implementation
   - Message prefixes for context (e.g., step numbers) use consistent patterns
   - Multi-line messages maintain proper indentation

3. **Semantic Message Types**
   - Extend message types to include semantic categories:
     - `info`: General information
     - `action`: User-initiated actions (e.g., "Adding arguments")
     - `metric`: Quantitative information (e.g., "Total inputs: 5")
     - `progress`: Ongoing operations
     - `status`: State changes or checkpoints

4. **Clean Message Interface**
   - Refactor all code that calls display methods to pass clean messages
   - Remove embedded icons from format strings
   - Ensure consistent verb tense in progress messages

### Non-Functional Requirements

1. **Backward Compatibility**
   - Changes must not break existing CI/CD pipelines that parse output
   - Maintain same information content in messages
   - Preserve verbosity level behavior

2. **Performance**
   - No measurable performance impact on output generation
   - Efficient string formatting without excessive allocations

3. **Maintainability**
   - Centralized icon and formatting configuration
   - Clear separation between content and presentation
   - Easy to add new message types or modify formatting

## Acceptance Criteria

- [ ] All display methods in `ProgressDisplay` trait use consistent icon spacing (single space)
- [ ] No message strings in the codebase contain icon characters directly
- [ ] All formatted messages in `orchestrator.rs` pass plain text to display methods
- [ ] New `DisplayMessageType` enum implemented for semantic message categorization
- [ ] Icon configuration centralized in a single location
- [ ] All progress messages use consistent verb tense (present continuous)
- [ ] Step and iteration counters use consistent formatting patterns
- [ ] No duplicate icons appear in any output scenario
- [ ] Output alignment is visually consistent across all message types
- [ ] Test suite validates formatting consistency
- [ ] Documentation updated with formatting guidelines

## Technical Details

### Implementation Approach

1. **Phase 1: Define Message Type System**
   ```rust
   pub enum DisplayMessageType {
       Info,
       Warning,
       Error,
       Progress,
       Success,
       Action,    // User-initiated actions
       Metric,    // Quantitative information
       Status,    // State changes
   }
   ```

2. **Phase 2: Centralize Icon Configuration**
   ```rust
   pub struct IconConfig {
       info: &'static str,      // "ℹ️"
       warning: &'static str,    // "⚠️"
       error: &'static str,      // "❌"
       progress: &'static str,   // "🔄"
       success: &'static str,    // "✅"
       action: &'static str,     // "📝"
       metric: &'static str,     // "📊"
       status: &'static str,     // "📋"
   }
   ```

3. **Phase 3: Update Display Implementation**
   - Modify `ProgressDisplayImpl` to use consistent spacing
   - Add methods for new message types
   - Ensure all icons are added at display layer only

4. **Phase 4: Refactor Message Callers**
   - Remove embedded icons from all format strings
   - Update orchestrator.rs to use appropriate message types
   - Ensure consistent message content formatting

### Architecture Changes

- Add `DisplayMessageType` enum to `interaction/mod.rs`
- Extend `UserInteraction` trait with typed message methods
- Create `IconConfig` structure for centralized icon management
- Add formatting utilities for consistent message structure

### APIs and Interfaces

```rust
pub trait UserInteraction {
    // Existing methods remain
    fn display_info(&self, message: &str);
    fn display_warning(&self, message: &str);
    
    // New typed methods
    fn display_action(&self, message: &str);
    fn display_metric(&self, label: &str, value: &str);
    fn display_status(&self, message: &str);
}
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/interaction/display.rs`
  - `src/cook/interaction/mod.rs`
  - `src/cook/orchestrator.rs`
  - `src/cook/workflow/executor.rs`
  - All components using `UserInteraction` trait

## Testing Strategy

- **Unit Tests**: 
  - Verify icon spacing consistency
  - Test message type categorization
  - Validate formatting utilities
  
- **Integration Tests**: 
  - End-to-end output formatting verification
  - Multi-line message alignment tests
  - Verbosity level filtering validation

- **Visual Tests**: 
  - Manual verification of output appearance
  - Screenshot comparison for regression testing

## Documentation Requirements

- **Code Documentation**: 
  - Document message type guidelines in trait definitions
  - Add examples of proper message formatting
  
- **User Documentation**: 
  - Update CLI output examples in README
  - Document output format for parsing tools

- **Developer Guidelines**: 
  - Add CONTRIBUTING.md section on message formatting
  - Create style guide for consistent message content

## Implementation Notes

1. **Icon Removal Pattern**: When refactoring, search for string literals containing emoji/icon characters and replace with plain text equivalents.

2. **Message Verb Tense**: 
   - Progress messages: Present continuous ("Processing...", "Running...")
   - Completed actions: Past tense ("Created", "Added")
   - Status updates: Present tense ("Ready", "Complete")

3. **Special Cases**:
   - Empty info messages (`ℹ️` alone) should be replaced with proper separators
   - Multi-line messages need consistent indentation after icons
   - Spinner messages should follow same formatting rules

4. **Testing Approach**: Create snapshot tests for common output scenarios to prevent regression.

## Migration and Compatibility

- No breaking changes to public APIs
- Output parsers may need updates if they rely on specific icon patterns
- Consider adding a `--plain` flag for icon-free output in future iteration
- Maintain backward compatibility for at least one major version