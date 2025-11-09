## Understanding the Workflow

> **Note**: All workflow phases execute in isolated git worktrees using a parent/child architecture. A single parent worktree hosts setup, reduce, and merge phases, while each map agent runs in a child worktree branched from the parent. Agents automatically merge back to the parent upon completion. The parent is merged to the original branch only with user confirmation at the end. This isolation ensures the main repository remains untouched during execution (Spec 127).

