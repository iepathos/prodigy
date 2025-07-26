# ROADMAP.md - Focused Development Plan

## Overview

This roadmap focuses on making `mmm improve` a robust, reliable tool that actually works. No more scaffolding - just working functionality.

## ✅ Phase 1: Core Foundation (COMPLETED)

### What Works Now
- [x] Dead simple CLI: `mmm improve [--target 8.0] [--verbose]`
- [x] Project analysis (language, framework, health detection)
- [x] JSON-based state management (simple and readable)
- [x] Basic Claude CLI integration structure
- [x] Minimal codebase (learning system removed)

## ✅ Phase 2: Working Claude Integration (COMPLETED)

### Priority: Make It Actually Work
- [x] **Real Claude CLI subprocess calls**
  - Execute actual `claude` commands with /mmm-code-review and /mmm-implement-spec
  - Handle stdout/stderr properly
  - Error handling for Claude CLI failures

- [x] **File Modification Pipeline**
  - Parse Claude responses for file changes
  - Apply changes safely with backups
  - Validate changes before applying

- [x] **Working Improvement Loop**
  - Analyze → Call Claude Review → Call Claude Implement → Apply Changes → Re-analyze → Repeat
  - Automatic termination when target reached or no issues found
  - Progress tracking and feedback with verbose mode

### Success Criteria
- User runs `mmm improve`
- Tool actually modifies files
- Code quality demonstrably improves
- Process is hands-off and reliable

## 📋 Phase 3: Robustness (NEXT)

### Core Reliability
- [ ] **Better Error Handling**
  - Graceful Claude CLI failures
  - Safe file operation rollbacks
  - Clear error messages to user

- [ ] **Enhanced Context Building**
  - Better project analysis
  - Smarter file selection for Claude
  - More accurate health scoring

- [ ] **Improved UX**
  - Real-time progress indicators
  - Better success/failure feedback
  - Clearer improvement summaries

### Success Criteria
- Works reliably across different project types
- Fails gracefully with helpful messages
- Users understand what's happening

## 🚀 Phase 4: Polish (FUTURE)

### Extended Language Support
- [ ] Python projects
- [ ] Go projects
- [ ] TypeScript/JavaScript enhancements
- [ ] Generic language fallbacks

### Enhanced Features
- [ ] Custom focus areas (`--focus tests`)
- [ ] Dry run mode (`--dry-run`)
- [ ] Better progress visualization
- [ ] Integration with common tools

## Non-Goals

Things we're **NOT** building:
- ❌ Complex workflow engines
- ❌ Plugin systems
- ❌ Monitoring dashboards
- ❌ Multi-project management
- ❌ Web interfaces
- ❌ Learning/AI systems
- ❌ Complex configuration

## Success Metrics

### Phase 2 (Working)
- `mmm improve` actually modifies files
- Users see code quality improvements
- Process completes without manual intervention

### Phase 3 (Robust)
- <5% failure rate on supported projects
- Users understand what went wrong when it fails
- Works on projects beyond our test cases

### Phase 4 (Polished)
- Supports 3+ programming languages well
- Sub-10 second startup time
- Clear, helpful progress feedback

## Development Principles

1. **Working > Perfect**: Make it work first, polish later
2. **Simple > Complex**: Choose simple solutions over clever ones
3. **Real > Simulated**: Actual Claude integration over mocking
4. **Users > Features**: Focus on user value over feature count
5. **Reliable > Fast**: Correctness over performance optimization

## Release Strategy

- **v0.1.0**: Current - Basic structure, no real Claude integration
- **v0.2.0**: Working - Actually calls Claude and modifies files
- **v0.3.0**: Robust - Reliable error handling and UX
- **v1.0.0**: Production - Polished, multi-language support