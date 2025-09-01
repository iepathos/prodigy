# Prodigy MVP Launch Checklist

## Launch Goal
Enable developers to successfully run AI-assisted development workflows in Python, JavaScript, and Rust projects within 5 minutes of installation.

## Success Criteria
- [ ] 10 successful workflow runs across 3 different languages
- [ ] Install → first success in < 5 minutes  
- [ ] Zero critical bugs in core workflow execution
- [ ] Usage tracking with clear iteration/command reporting
- [ ] 3+ beta users who successfully complete a workflow

---

## Phase 1: Multi-Language Support (Week 1)
**Goal**: Prodigy works reliably with Python, JavaScript, and Rust projects

### Python Support
- [ ] Detect Python projects (presence of `setup.py`, `pyproject.toml`, or `requirements.txt`)
- [ ] Auto-configure test command (`pytest`, `python -m pytest`, `python -m unittest`)
- [ ] Auto-configure lint/format commands (`black`, `ruff`, `flake8`, `pylint`)
- [ ] Handle Python-specific error messages in test output
- [ ] Create Python example workflows:
  - [ ] `examples/python/improve.yml`
  - [ ] `examples/python/coverage.yml`
  - [ ] `examples/python/security.yml`

### JavaScript/TypeScript Support  
- [ ] Detect JS/TS projects (presence of `package.json`)
- [ ] Auto-configure test command (`npm test`, `yarn test`, `jest`, `vitest`)
- [ ] Auto-configure lint/format commands (`prettier`, `eslint`)
- [ ] Handle JS/TS-specific error messages in test output
- [ ] Create JavaScript example workflows:
  - [ ] `examples/javascript/improve.yml`
  - [ ] `examples/javascript/coverage.yml`
  - [ ] `examples/javascript/modernize.yml`

### Rust Support (Enhancement)
- [ ] Improve existing Rust support
- [ ] Auto-detect test command variations
- [ ] Create additional Rust workflows:
  - [ ] `examples/rust/unsafe-audit.yml`
  - [ ] `examples/rust/dependency-update.yml`

### Language Detection
- [ ] Implement `detect_project_type()` function
- [ ] Support mixed-language projects (e.g., Python + JS)
- [ ] Fallback to manual configuration if detection fails

---

## Phase 2: First-Run Experience (Week 1-2)
**Goal**: New users achieve success within 5 minutes

### Installation
- [ ] Create installation script for Unix/Mac: `install.sh`
- [ ] Create installation script for Windows: `install.ps1`
- [ ] Host installation scripts (GitHub releases or prodigy.dev)
- [ ] Test one-liner installation:
  ```bash
  curl -sSL https://raw.githubusercontent.com/iepathos/prodigy/main/install.sh | sh
  ```
- [ ] Add Prodigy to PATH automatically
- [ ] Verify Claude CLI is installed and configured

### Init Command
- [ ] Fix `prodigy init` to actually work
- [ ] Auto-detect project language and tools
- [ ] Install appropriate Claude commands based on language
- [ ] Create default `improve.yml` workflow for detected language
- [ ] Generate `.prodigy/PROJECT.md` with project context
- [ ] Show clear next steps after init

### Quick Start Flow
- [ ] Test complete flow takes < 5 minutes:
  1. [ ] Install Prodigy
  2. [ ] Run `prodigy init` 
  3. [ ] Run `prodigy cook improve.yml --dry-run`
  4. [ ] Run `prodigy cook improve.yml`
  5. [ ] See meaningful improvements

### Example Workflows
- [ ] Create `examples/` directory with 15+ working workflows
- [ ] Organize by language and use case
- [ ] Each workflow includes comments explaining what it does
- [ ] Test all examples actually work

---

## Phase 3: Safety & Predictability (Week 2)
**Goal**: Users trust Prodigy won't run forever or break their code

### Usage Tracking & Reporting
- [ ] Track number of Claude commands executed
- [ ] Track number of iterations completed
- [ ] Track elapsed time
- [ ] Track files modified
- [ ] Show summary after each run:
  ```
  📊 Session Complete:
     Claude commands: 12
     Iterations: 3/5 (stopped early - tests passing)
     Duration: 4m 32s
     Files modified: 8
     Commits created: 7
  ```

### Iteration & Command Limits
- [ ] Implement hard iteration limit (default: 5)
- [ ] Implement per-iteration command limit (default: 20)
- [ ] Add `--max-iterations` flag
- [ ] Add `--max-commands` flag
- [ ] Show warnings before starting:
  ```
  ⚠️  This workflow will run up to 5 iterations
     Each iteration may execute 3-5 Claude commands
     Continue? (y/N)
  ```

### Early Termination
- [ ] Stop when no changes are made
- [ ] Stop when tests pass (for test workflows)
- [ ] Stop when specific success criteria are met
- [ ] Add `--stop-on-success` flag
- [ ] Add Ctrl+C handler for graceful shutdown

### Rollback Capability
- [ ] Implement `prodigy rollback` command
- [ ] Rollback to state before last cook session
- [ ] Show what will be rolled back before confirming
- [ ] Work with both regular and worktree sessions

### Dry Run Mode
- [ ] Implement `--dry-run` flag
- [ ] Show what commands would be executed
- [ ] Estimate number of iterations likely
- [ ] Preview workflow without making changes

### Progress Indicators
- [ ] Show current iteration number
- [ ] Show current command being executed
- [ ] Show elapsed time
- [ ] Show spinner/progress for long operations
- [ ] Clear indication when waiting for Claude

---

## Phase 4: Error Handling & Recovery (Week 2-3)
**Goal**: Prodigy handles failures gracefully

### Error Messages
- [ ] Replace cryptic Rust errors with helpful messages
- [ ] Suggest fixes for common problems
- [ ] Include relevant context in error messages
- [ ] Different error formats for --verbose mode

### Claude Error Handling
- [ ] Retry on rate limits with exponential backoff
- [ ] Handle Claude CLI not installed
- [ ] Handle Claude CLI not authenticated  
- [ ] Handle Claude command not found
- [ ] Handle malformed Claude responses

### Workflow Validation
- [ ] Validate YAML syntax before execution
- [ ] Check for missing required fields
- [ ] Validate command references exist
- [ ] Check for circular dependencies
- [ ] Show clear error for invalid workflows

### Recovery & Resume
- [ ] Save state after each successful step
- [ ] Implement `--resume` flag to continue interrupted session
- [ ] Clean up worktrees on failure
- [ ] Clean up temporary files on exit
- [ ] Handle git conflicts gracefully

### Timeout Handling
- [ ] Add timeout for Claude commands (default: 5 min)
- [ ] Add timeout for shell commands (default: 10 min)
- [ ] Add overall session timeout (default: 1 hour)
- [ ] Show clear message when timeout occurs
- [ ] Allow configuration of timeouts

---

## Phase 5: Documentation (Week 3)
**Goal**: Users can learn and troubleshoot independently

### Installation Guide
- [ ] Step-by-step for Mac/Linux
- [ ] Step-by-step for Windows (WSL2)
- [ ] Claude CLI setup instructions
- [ ] Troubleshooting installation issues
- [ ] System requirements

### Getting Started Guide
- [ ] 5-minute quickstart
- [ ] First workflow walkthrough
- [ ] Understanding the output
- [ ] Customizing your first workflow
- [ ] Common patterns

### Workflow Writing Guide
- [ ] YAML syntax reference
- [ ] Available commands documentation
- [ ] Variable interpolation examples
- [ ] Conditional execution examples
- [ ] 10+ complete workflow examples

### Troubleshooting Guide
- [ ] Common errors and solutions
- [ ] Debug mode usage
- [ ] Log file locations
- [ ] Getting help / reporting issues
- [ ] FAQ section

### Command Reference
- [ ] Document all Prodigy CLI commands
- [ ] Document all Claude commands
- [ ] Document all flags and options
- [ ] Environment variables
- [ ] Configuration files

---

## Phase 6: Testing & Validation (Week 3-4)
**Goal**: Verify everything works as expected

### Core Functionality Tests
- [ ] Test workflow execution on Python project
- [ ] Test workflow execution on JavaScript project  
- [ ] Test workflow execution on Rust project
- [ ] Test worktree isolation
- [ ] Test iteration limits
- [ ] Test rollback functionality

### Edge Case Tests
- [ ] Test with no git repository
- [ ] Test with dirty git state
- [ ] Test with merge conflicts
- [ ] Test with missing dependencies
- [ ] Test with slow/timeout scenarios
- [ ] Test interruption and resume

### Integration Tests
- [ ] Full install → init → cook flow
- [ ] Multi-language project handling
- [ ] Large file handling
- [ ] Long-running workflows
- [ ] Parallel worktree sessions

### Beta Testing
- [ ] Recruit 3-5 beta users
- [ ] Provide beta testing guide
- [ ] Collect feedback form/template
- [ ] Track success/failure rate
- [ ] Document common issues
- [ ] Iterate based on feedback

---

## Phase 7: Polish & Launch Prep (Week 4)
**Goal**: Ready for public announcement

### Final Polish
- [ ] Clean up all TODO comments in code
- [ ] Remove debug print statements
- [ ] Optimize performance bottlenecks
- [ ] Consistent error message format
- [ ] Professional README

### Release Preparation
- [ ] Version number (0.1.0)
- [ ] CHANGELOG.md with initial release notes
- [ ] LICENSE file (MIT or Apache 2.0)
- [ ] CONTRIBUTING.md guidelines
- [ ] GitHub release with binaries
- [ ] GitHub Actions CI/CD setup

### Documentation Site
- [ ] Consider simple docs site (mdBook or similar)
- [ ] Searchable documentation
- [ ] Copy-paste friendly examples
- [ ] Video walkthrough (optional)
- [ ] Architecture overview

### Community Setup
- [ ] GitHub issue templates
- [ ] GitHub discussions enabled
- [ ] Discord or Slack (optional)
- [ ] Twitter/social announcement ready
- [ ] Blog post draft (optional)

---

## Launch Day Checklist

### Pre-Launch (Day Before)
- [ ] All Phase 1-6 items complete
- [ ] Beta testers confirmed success
- [ ] Installation scripts tested on fresh VMs
- [ ] Documentation reviewed for accuracy
- [ ] README polished and compelling

### Launch Day
- [ ] Make repository public
- [ ] Publish GitHub release
- [ ] Post on Hacker News (Show HN)
- [ ] Post on Reddit (r/programming, r/rust, etc.)
- [ ] Tweet announcement
- [ ] Monitor for issues
- [ ] Respond to feedback quickly

### Post-Launch (Week 1)
- [ ] Daily check for critical issues
- [ ] Respond to all GitHub issues
- [ ] Fix any critical bugs immediately  
- [ ] Thank early adopters
- [ ] Collect feature requests

---

## Definition of Done

The MVP is complete when:

1. **A Python developer** can go from zero to improved test coverage in < 5 minutes
2. **A JavaScript developer** can run a code improvement workflow successfully
3. **A Rust developer** can use Prodigy to improve their existing project
4. **All workflows** complete within iteration limits without errors
5. **Beta users** report successful experiences
6. **Documentation** is sufficient for self-service

---

## Risk Mitigation

### Highest Risks
1. **Claude CLI changes** → Document required version, test compatibility
2. **Platform issues** → Focus on Mac/Linux first, Windows can wait
3. **Infinite loops** → Hard limits, timeouts, circuit breakers
4. **Data loss** → Git worktrees, atomic commits, rollback capability
5. **Poor UX** → Beta test extensively, iterate on feedback

### Contingency Plans
- If multi-language is too complex → Launch with Python only
- If rollback is too hard → Document manual recovery steps
- If Windows doesn't work → Document WSL2 requirement
- If beta feedback is poor → Delay launch, iterate more

---

## Success Metrics Post-Launch

### Week 1 Goals
- 100+ GitHub stars
- 10+ successful user reports
- < 5 critical bugs reported
- 50+ workflow runs completed

### Month 1 Goals  
- 500+ GitHub stars
- 100+ active users
- 5+ community contributed workflows
- 3+ blog posts/tutorials by users

---

## Notes

- **Focus on Python first** if time is tight - largest AI/ML community
- **Perfect is the enemy of good** - ship when "good enough"
- **User feedback > features** - listen and iterate quickly
- **Documentation > features** - users need to understand it
- **Stability > performance** - slow but reliable wins
