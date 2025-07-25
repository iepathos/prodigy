# Development Loop Flow

This document describes how MMM creates and manages self-sufficient Claude Code development loops.

## Overview

MMM implements an **iterative, automated development cycle** that can take specifications and implement them through multiple Claude interactions with minimal human intervention. The system maintains complete context awareness and state persistence across iterations.

## Core Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Specification │───▶│  Context Builder │───▶│ Claude Invocation│
│    Detection    │    │                  │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         ▲                                                ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   State Update  │◀───│   Validation     │◀───│ Response Process│
│                 │    │                  │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         ▲                                                ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│  Next Iteration │◀───│  Code Extraction │◀───│ Action Execution│
│    Decision     │    │                  │    │                 │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Detailed Flow Steps

### 1. Specification Detection
- **File Monitoring**: Watches `specs/` directory for new or modified `.md` files
- **Event Triggers**: Automatically initiates workflows when specs are added/modified
- **Spec Parsing**: Extracts objectives, acceptance criteria, and metadata from markdown

**Relevant Code**: `src/spec/parser.rs`, `src/workflow/event.rs`

### 2. Context Building
- **Priority Queue**: Assembles context with Critical → High → Medium → Low priority items
- **Token Optimization**: Intelligently truncates content to fit within Claude's context window
- **Dynamic Variables**: Injects project metadata, previous iterations, and relevant code

**Key Components**:
- Current project context (name, language, framework)
- Specification content and acceptance criteria  
- Previous iteration results and feedback
- Relevant code snippets and project structure
- Dependencies between specifications

**Relevant Code**: `src/claude/context.rs:98-141`

### 3. Claude Invocation
- **Prompt Templates**: Uses YAML-based templates with variable substitution
- **Model Selection**: Chooses appropriate Claude model based on task complexity
- **Token Tracking**: Monitors usage against daily/project limits
- **Retry Logic**: Handles failures with exponential backoff

**Relevant Code**: `src/claude/api.rs`, `src/claude/prompt.rs`

### 4. Response Processing
- **Parser Chain**: Multiple parsers extract different content types
- **Code Block Extraction**: Identifies and extracts code with language detection
- **Command Parsing**: Recognizes MMM-specific commands (`@mmm:complete`, `@mmm:needs-review`)
- **Validation**: Ensures response meets quality criteria

**Relevant Code**: `src/claude/response.rs:144-174`

### 5. Action Execution
- **Code Application**: Writes extracted code to appropriate files
- **Command Execution**: Runs shell commands, tests, and build processes
- **File Operations**: Creates, modifies, or deletes files as directed
- **Error Handling**: Captures and processes execution failures

### 6. Code Extraction
- **Multi-format Support**: Handles various code formats and languages
- **Context Preservation**: Maintains proper indentation and formatting
- **Dependency Detection**: Identifies imports and dependencies
- **Quality Checks**: Validates syntax and basic structure

### 7. Validation
- **Acceptance Criteria**: Checks if spec requirements are met
- **Quality Gates**: Runs linting, testing, and build verification
- **Regression Testing**: Ensures new changes don't break existing functionality
- **Human Checkpoints**: Triggers manual review when configured

### 8. State Update
- **SQLite Persistence**: Records execution history and current state
- **Progress Tracking**: Updates spec completion status
- **Metrics Collection**: Stores performance and usage data
- **Snapshot Creation**: Saves state for potential rollback

**Database Schema**: `migrations/20250125000000_initial_schema.sql`

## Self-Sufficiency Mechanisms

### Intelligent Context Management
- **Memory System**: Short-term exchanges + long-term compressed summaries
- **Relationship Graphs**: Tracks dependencies between specs and code components
- **Adaptive Prioritization**: Learns which context is most valuable over time

### Advanced Prompt Engineering
```yaml
# Example prompt template structure
template: |
  ## Current Project Context
  Project: {project_name}
  Language: {primary_language}
  Framework: {framework}
  
  ## Feature Specification  
  {spec_content}
  
  ## Current Implementation Status
  Completed specs: {completed_specs}
  Current iteration: {iteration}
  
  ## Previous Feedback
  {previous_feedback}
```

### Workflow Automation
- **Multi-stage Processes**: Planning → Implementation → Integration → Review → Finalization
- **Conditional Logic**: Executes different paths based on spec complexity, test results, etc.
- **Parallel Execution**: Runs independent tasks concurrently for efficiency
- **Event-driven Triggers**: Responds to file changes, test failures, review requests

## Loop Termination Conditions

The development loop continues until one of these conditions is met:

1. **Successful Completion**: Claude responds with `@mmm:complete` or equivalent
2. **Maximum Iterations**: Reaches configured iteration limit (default: 3)
3. **Human Intervention**: Manual pause or termination
4. **Critical Failure**: Unrecoverable error state
5. **Checkpoint Timeout**: Human review timeout expires

## Error Recovery

- **Automatic Retry**: Transient failures retry with exponential backoff
- **State Rollback**: Can revert to previous working state
- **Alternative Strategies**: Tries different approaches on repeated failures
- **Human Escalation**: Notifies human operators for complex issues

## Integration Points

### CI/CD Systems
- **GitHub Actions**: Automatic workflow triggers on spec changes
- **Build Pipelines**: Integration with existing build and test infrastructure
- **Deployment Gates**: Automated deployment after successful completion

### External Tools
- **Version Control**: Git integration for commits and branching
- **Testing Frameworks**: Supports various test runners and formats
- **Code Quality**: Linting, formatting, and security scanning integration

## Performance Characteristics

- **Concurrent Processing**: Multiple specs can be processed simultaneously
- **Efficient State Queries**: SQLite indexes optimize common queries
- **Minimal Memory Footprint**: Streams large files and uses lazy loading
- **Token Optimization**: Intelligent prompt compression reduces API costs

## Monitoring and Observability

- **Real-time Dashboard**: Live view of active workflows and progress
- **Execution Metrics**: Performance tracking and bottleneck identification
- **Audit Trails**: Complete history of all actions and decisions
- **Custom Alerts**: Configurable notifications for important events

## Configuration Examples

### Basic Loop Configuration
```toml
[loop]
max_iterations = 5
timeout = "2h"
auto_commit = false

[claude]
model = "claude-3-sonnet"
temperature = 0.7
max_tokens = 4000
```

### Advanced Workflow Configuration
```yaml
stages:
  - name: implementation
    max_retries: 3
    timeout: 30m
    parallel: true
    
checkpoints:
  - after: implementation
    required: true
    timeout: 24h
```

This self-sufficient loop system enables MMM to handle complex development tasks with minimal human intervention while maintaining high quality and providing extensive visibility into the development process.