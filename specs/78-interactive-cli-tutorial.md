---
number: 78
title: Interactive CLI Tutorial
category: foundation
priority: medium
status: draft
dependencies: [75]
created: 2025-01-17
---

# Specification 78: Interactive CLI Tutorial

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: [75 - Interactive Examples Directory]

## Context

New users benefit from interactive, hands-on tutorials that guide them through features step-by-step. Static documentation requires users to context-switch between reading and doing. An interactive tutorial within the CLI provides immediate feedback and progressive learning.

## Objective

Create an interactive in-CLI tutorial system that guides users through Prodigy's features with hands-on exercises, real-time feedback, and progressive skill building, similar to `vimtutor` or `git tutorial`.

## Requirements

### Functional Requirements
- Provide interactive tutorial accessible via `prodigy tutorial`
- Create multi-lesson curriculum covering all features
- Track user progress through lessons
- Provide real-time feedback on user actions
- Include exercises with validation
- Support hints and solutions for stuck users
- Allow skipping to specific lessons
- Provide completion certificates/badges
- Create sandbox environment for safe experimentation
- Support different learning paths (quick, comprehensive)
- Include interactive quizzes to test understanding

### Non-Functional Requirements
- Tutorial runs completely offline
- Each lesson completes in 5-10 minutes
- No permanent changes to user's system
- Progress persists between sessions
- Works in all standard terminals
- Supports interruption and resumption

## Acceptance Criteria

- [ ] Tutorial command launches interactive learning environment
- [ ] 10+ lessons covering core features available
- [ ] Progress tracking shows completion percentage
- [ ] Exercises validate user inputs correctly
- [ ] Hints help users when stuck for >30 seconds
- [ ] Sandbox prevents accidental system changes
- [ ] Tutorial adapts to user's pace and skill level
- [ ] Completion provides summary of learned skills
- [ ] Users can resume from last position
- [ ] Tutorial works without internet connection

## Technical Details

### Implementation Approach
1. Build tutorial framework with state management
2. Create lesson content and exercises
3. Implement sandbox environment
4. Add progress tracking system
5. Build interactive UI with crossterm/tui

### Tutorial Structure
```rust
pub struct Tutorial {
    lessons: Vec<Lesson>,
    current_lesson: usize,
    progress: Progress,
    sandbox: Sandbox,
}

pub struct Lesson {
    id: String,
    title: String,
    objectives: Vec<String>,
    content: Vec<Step>,
    exercises: Vec<Exercise>,
    quiz: Option<Quiz>,
}

pub struct Step {
    instruction: String,
    expected_action: Action,
    validation: Box<dyn Fn(&State) -> bool>,
    hint: Option<String>,
}
```

### Curriculum Outline
```
1. Introduction to Prodigy
   - What is Prodigy
   - Core concepts
   - First command

2. Basic Workflows
   - Creating workflows
   - Running workflows
   - Understanding output

3. Shell Commands
   - Shell integration
   - Command chaining
   - Error handling

4. Claude Integration
   - Claude commands
   - Context passing
   - Best practices

5. Parallel Execution
   - MapReduce basics
   - Parallel workflows
   - Performance benefits

6. Goal Seeking
   - Iterative refinement
   - Success criteria
   - Practical examples

7. Advanced Workflows
   - Conditional execution
   - Dynamic generation
   - Complex patterns

8. Testing Automation
   - Test workflows
   - Coverage improvement
   - CI/CD integration

9. Debugging
   - Troubleshooting
   - Event logs
   - Common issues

10. Best Practices
    - Workflow design
    - Performance optimization
    - Maintenance
```

### APIs and Interfaces
- `prodigy tutorial` - Start interactive tutorial
- `prodigy tutorial --lesson <n>` - Jump to specific lesson
- `prodigy tutorial --progress` - Show progress summary
- `prodigy tutorial --reset` - Reset progress
- `prodigy tutorial --quick` - Quick 15-minute intro

## Dependencies

- **Prerequisites**:
  - Spec 75: Example workflows to use
- **Affected Components**: CLI interface
- **External Dependencies**:
  - crossterm (for terminal UI)
  - tui-rs (for interface)

## Testing Strategy

- **Unit Tests**: Test lesson validation logic
- **Integration Tests**: Test complete lesson flows
- **Usability Tests**: Test with new users
- **Performance Tests**: Ensure responsive UI
- **User Acceptance**: Gather feedback from learners

## Documentation Requirements

- **Code Documentation**: Document tutorial framework
- **User Documentation**: Add tutorial info to README
- **Architecture Updates**: Document tutorial system design

## Implementation Notes

- Keep lessons focused and short
- Provide immediate positive feedback
- Use color and formatting for clarity
- Save progress to ~/.prodigy/tutorial/
- Consider gamification (points, achievements)
- Test with users of different skill levels
- Support keyboard navigation throughout

## Migration and Compatibility

- No breaking changes to existing functionality
- Tutorial is optional feature
- Progress format supports future lessons
- Backward compatible with older terminals