---
number: 45
title: Context Window Management
category: optimization
priority: high
status: draft
dependencies: [44, 11]
created: 2024-01-15
---

# Specification 45: Context Window Management

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [Spec 44: Context-Aware Project Understanding, Spec 11: Simple State Management]

## Context

Currently, Prodigy provides the same static context to Claude for every iteration - typically PROJECT.md, ARCHITECTURE.md, and some basic project information. This one-size-fits-all approach is inefficient and limits Claude's effectiveness, especially in longer improvement sessions. As iterations progress, Claude needs different context: the files currently being worked on, related test files, recent changes, and learned patterns.

Effective context window management is crucial for maximizing Claude's capabilities within token limits while ensuring relevant information is always available. This becomes even more critical as projects grow and improvement sessions run for many iterations.

## Objective

Implement a smart context window management system that dynamically selects and prioritizes the most relevant information for each iteration, maximizing Claude's effectiveness while staying within token limits.

## Requirements

### Functional Requirements

1. **Core Context Management**
   - Always include essential project files (PROJECT.md, ARCHITECTURE.md)
   - Identify and include key interfaces and type definitions
   - Prioritize files based on project structure and importance
   - Maintain a stable core context across iterations

2. **Dynamic Context Selection**
   - Track files changed in recent iterations (last 3-5)
   - Automatically include test files for code being modified
   - Include dependencies and dependents of current focus files
   - Adapt context based on current workflow phase (review/implement/lint)

3. **Memory Context Integration**
   - Include relevant iteration history and learned patterns
   - Add recent failures and successful solutions
   - Include project-specific constraints and quirks
   - Prioritize memory items by relevance to current task

4. **Token Budget Management**
   - Calculate token usage for each context component
   - Prioritize context elements when approaching limits
   - Implement smart truncation strategies
   - Reserve tokens for Claude's response

5. **Context Caching**
   - Cache tokenized context components for efficiency
   - Invalidate cache when files change
   - Pre-compute common context combinations
   - Store context metadata for analysis

### Non-Functional Requirements

- **Performance**: Context selection must complete in < 2 seconds
- **Token Efficiency**: Maximize information density within token limits
- **Adaptability**: Context should evolve with the improvement session
- **Transparency**: Clear logging of what context is included and why
- **Configurability**: Allow users to customize context priorities

## Acceptance Criteria

- [ ] Core context files are always included unless they exceed token budget
- [ ] Recently modified files are automatically added to context
- [ ] Test files are included when their corresponding source files are in context
- [ ] Memory items are filtered by relevance to current iteration
- [ ] Token usage is tracked and stays within Claude's limits
- [ ] Context selection logs show clear reasoning for inclusions/exclusions
- [ ] Performance meets < 2 second requirement for context building
- [ ] Context quality improves Claude's success rate measurably

## Technical Details

### Implementation Approach

1. **Context Manager Architecture**
   ```rust
   pub struct ContextManager {
       core_selector: CoreContextSelector,
       dynamic_selector: DynamicContextSelector,
       memory_selector: MemoryContextSelector,
       token_counter: TokenCounter,
       cache: ContextCache,
   }
   
   pub struct ContextWindow {
       core: Vec<ContextItem>,
       dynamic: Vec<ContextItem>,
       memory: Vec<ContextItem>,
       total_tokens: usize,
   }
   ```

2. **Token Budget Allocation**
   ```rust
   const MAX_CONTEXT_TOKENS: usize = 150_000;  // Claude's limit
   const RESPONSE_RESERVE: usize = 50_000;    // Reserve for response
   
   pub struct TokenBudget {
       core: usize,      // 30% - Essential files
       dynamic: usize,   // 50% - Current work
       memory: usize,    // 20% - Historical context
   }
   ```

3. **Relevance Scoring**
   ```rust
   pub trait RelevanceScorer {
       fn score(&self, item: &ContextItem, iteration: &Iteration) -> f32;
   }
   
   // Factors: recency, import relationships, test coverage, focus area
   ```

### Architecture Changes

- New `context` module in the cook workflow
- Enhanced state to track file access patterns
- Integration with memory system from Spec 44
- Modified Claude command interface to accept dynamic context

### Data Structures

```rust
pub struct ContextItem {
    path: PathBuf,
    content: String,
    item_type: ContextType,
    tokens: usize,
    relevance_score: f32,
    last_modified: DateTime<Utc>,
}

pub enum ContextType {
    CoreDocument,      // PROJECT.md, ARCHITECTURE.md
    SourceCode,        // Active code files
    TestFile,          // Related test files
    Dependency,        // Imported/dependent files
    Memory,            // Historical patterns
    Interface,         // Key types/interfaces
}

pub struct ContextMetrics {
    total_tokens: usize,
    items_included: usize,
    items_excluded: usize,
    selection_time: Duration,
    hit_rate: f32,
}
```

### APIs and Interfaces

```rust
pub trait ContextSelector {
    fn select_context(
        &self,
        current_focus: &[PathBuf],
        iteration_state: &IterationState,
        token_budget: usize,
    ) -> Result<ContextWindow>;
}

pub trait TokenCounter {
    fn count_tokens(&self, text: &str) -> usize;
    fn estimate_tokens(&self, file_path: &Path) -> Result<usize>;
}
```

## Dependencies

- **Prerequisites**: 
  - Spec 44: Context-Aware Project Understanding (for project analysis)
  - Spec 11: Simple State Management (for iteration tracking)
- **Affected Components**: 
  - Claude command execution will use dynamic context
  - Cook workflow will build context before each Claude call
  - State management will track context usage metrics
- **External Dependencies**: 
  - Token counting library (tiktoken or similar)
  - File watching for cache invalidation

## Testing Strategy

- **Unit Tests**: Test each context selector independently
- **Integration Tests**: Verify full context building pipeline
- **Performance Tests**: Ensure < 2 second context building
- **Token Tests**: Verify context stays within limits
- **Effectiveness Tests**: Measure improvement in Claude's success rate

## Documentation Requirements

- **Code Documentation**: Document selection algorithms and heuristics
- **User Documentation**: Explain how context selection works
- **Configuration Guide**: Document context customization options
- **Metrics Guide**: Explain context effectiveness metrics

## Implementation Notes

1. **Incremental Rollout**: Start with simple recency-based selection, add sophistication
2. **Failsafe Mode**: If smart selection fails, fall back to static context
3. **User Override**: Allow manual context specification via config
4. **Learning System**: Track which context combinations work best
5. **Token Estimation**: Use fast approximation, refine with actual counts

## Migration and Compatibility

- **Backward Compatible**: Falls back to current static context approach
- **Progressive Enhancement**: Context selection improves over time
- **Configuration Migration**: Existing configs continue to work
- **Metrics Collection**: Start collecting context effectiveness data immediately