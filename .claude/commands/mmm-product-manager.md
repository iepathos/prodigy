# Product Manager Command

Act as a Product Manager, evaluating the current state and proposing next steps for you to review.

## Primary Functions

1. **Analyze Current Project State**
   - Review completed implementations and features
   - Identify gaps or missing functionality
   - Assess code quality and technical debt
   - Check test coverage and documentation
   - Evaluate recent development activity

2. **Propose Next Steps**
   - Generate prioritized improvement recommendations
   - Suggest task assignments by specialization
   - Identify blocking issues or dependencies
   - Recommend architectural improvements

3. **Update Project Context**
   - Suggest updates to .mmm/ documentation
   - Propose CLAUDE.md improvements
   - Recommend new patterns or conventions
   - Flag outdated documentation

## How This Command Works

This command performs analysis as a Product Manager agent and presents findings for manual review.  You'll receive:

1. **Current State Summary**
   - Component implementation status
   - Recent achievements and changes
   - Active development areas
   - Known issues and TODOs

2. **Proposed Next Steps**
   - Prioritized task recommendations
   - Suggested agent specializations
   - Dependency considerations
   - Risk assessments

3. **Context Update Recommendations**
   - Specific documentation changes needed
   - New patterns to document
   - Outdated sections to refresh

## Execution Process

### Step 1: Comprehensive Project Analysis

The command will analyze:
- Implementation completeness of all components
- Code quality metrics and technical debt
- Test coverage and documentation gaps
- Recent development activity and patterns
- TODOs, FIXMEs, and unimplemented sections

### Step 2: Generate Improvement Proposals

Based on the analysis, the command will propose:

**High Priority Tasks:**
- Tasks that block other development
- Critical bug fixes or security issues
- Missing core functionality

**Medium Priority Tasks:**
- Feature enhancements
- Documentation updates
- Test coverage improvements
- Performance optimizations

**Low Priority Tasks:**
- Code refactoring
- Nice-to-have features
- Style improvements

### Step 3: Recommend Context Updates

The command will suggest specific updates to:

**.mmm/ Directory:**
- PROJECT.md - Current capabilities and limitations
- ARCHITECTURE.md - System design documentation
- CONVENTIONS.md - Coding patterns and standards
- ROADMAP.md - Progress tracking and next steps
- HISTORY.md - Completed work and decisions

**CLAUDE.md:**
- Latest implementation status
- New patterns discovered
- Common tasks and examples
- Important notes and gotchas

## Current State Analysis (July 2025)

Based on the latest analysis, here's what this command has found:

### ✅ Completed Components
- **Core Architecture**: Orchestrator, MCP server, task scheduler, agent manager
- **Agent Specializations**: 7 types including PM with comprehensive tools
- **GitHub Import**: Full repository analysis and task generation
- **Git Operations**: Merge logic, conflict resolution, file locking
- **CLI & Web UI**: Complete command interface and Vue.js dashboard

### 🚧 Gaps Identified
- **Missing .mmm/ directory**: Context management system not initialized
- **Database Persistence**: SQLx added but not implemented
- **Agent Spawning**: Has TODO - actual Claude Code process spawning needed
- **Test Coverage**: Limited integration tests, no coverage reporting

### 📋 Proposed Next Steps

**High Priority (Blocking Issues):**
1. **Initialize .mmm/ Context System**
   - Create directory structure
   - Generate initial PROJECT.md, ARCHITECTURE.md, etc.
   - Specialization: Product Manager

2. **Fix Claude Code Agent Spawning**
   - Implement actual process spawning in claude_wrapper.rs
   - Handle both Max subscription and API key modes
   - Specialization: Backend Engineer

3. **Implement Database Persistence**
   - Create SQLx migrations for projects/tasks/agents
   - Update Orchestrator to use database
   - Keep DashMap as cache layer
   - Specialization: Backend Engineer

**Medium Priority (Feature Completion):**
4. **Increase Test Coverage**
   - Add integration tests for full workflows
   - Implement coverage reporting
   - Target 80% coverage
   - Specialization: QA Engineer

5. **Complete MCP Handler TODOs**
   - Implement task claiming logic
   - Add dependency notifications
   - Fix status broadcasts
   - Specialization: Backend Engineer

6. **Run Self-Improvement**
   - Execute self-improvement workflow
   - Monitor recursive improvement
   - Specialization: Product Manager

**Low Priority (Enhancements):**
7. **Add Monitoring & Metrics**
   - Implement performance tracking
   - Add agent resource monitoring
   - Specialization: DevOps Engineer

8. **Enhance Web Dashboard**
   - Real-time task visualization
   - Agent performance metrics
   - Specialization: Frontend Engineer

## Context Update Recommendations

### Create .mmm/PROJECT.md
- Project overview

### Update CLAUDE.md
- Current implementation status

## Manual Review Questions

1. **Priority Agreement**: Do you agree with the priority ordering above?
2. **Resource Allocation**: How many agents should work on each task?
3. **Timeline**: What's your target timeline for high priority items?
4. **Context Creation**: Should we initialize .mmm/ now?
5. **Self-Improvement**: Ready to run the self-improvement spec?

This PM analysis provides you with actionable recommendations without automatically creating tasks in the system.