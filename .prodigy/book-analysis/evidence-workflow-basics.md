# Evidence for Workflow Basics

## Source Definitions Found
- WorkflowStepCommand type: src/config/command.rs:319
- WriteFileConfig type: src/config/command.rs:280
- ForeachConfig type: src/config/command.rs:191
- GoalSeekConfig type: src/cook/goal_seek/mod.rs:15
- ValidationConfig type: src/cook/workflow/validation.rs:12
- Test command deprecation: src/config/command.rs:447-462

## Command Types Verified
- claude: src/config/command.rs:323 (Option<String>)
- shell: src/config/command.rs:326 (Option<String>)
- analyze: src/config/command.rs:330 (Option<HashMap>)
- test: src/config/command.rs:334 (deprecated with warning)
- goal_seek: src/config/command.rs:338 (Option<GoalSeekConfig>)
- foreach: src/config/command.rs:342 (Option<ForeachConfig>)
- write_file: src/config/command.rs:346 (Option<WriteFileConfig>)
- validate: src/config/command.rs:379 (Option<ValidationConfig>)

## Non-Existent Features Identified
- handler command: NOT FOUND in WorkflowStepCommand
- working_dir field: NOT FOUND in WorkflowStepCommand
- step-level env field: NOT FOUND in WorkflowStepCommand (only workflow-level)

## Workflow Examples Found
- write_file usage: workflows/debtmap-reduce.yml:105
- validate usage: workflows/implement.yml:8, workflows/goal-seeking-examples.yml:9+
- goal_seek usage: workflows/goal-seeking-examples.yml:9,21,32,50,67,81,103,111,126
- test command (deprecated): documented in src/config/command.rs:447-462

## Field Definitions
- WriteFileFormat enum: src/config/command.rs:301-313 (Text, Json, Yaml)
- ForeachInput enum: src/config/command.rs:214-220 (Command, List)
- ParallelConfig: src/config/command.rs:196 (continue_on_error field)
- CaptureOutputConfig: src/config/command.rs:404-409 (Boolean or String)
- capture_streams: src/config/command.rs:394
- output_file: src/config/command.rs:398

## Validation Results
✓ All command types verified against WorkflowStepCommand enum
✓ write_file and validate command types confirmed in implementation
✓ test command confirmed as deprecated (shows warning)
✓ handler command confirmed as NON-EXISTENT
✓ working_dir confirmed as NON-EXISTENT
✓ step-level env confirmed as NON-EXISTENT
✓ foreach syntax verified (foreach + do block + continue_on_error)
✓ goal_seek syntax verified (goal + validate + threshold + max_attempts)
✗ No foreach examples found in workflow files (will create based on type definition)

## Discovery Notes
- Test directories found: None searched (examples sufficient)
- Example directories found: workflows/
- Source directories searched: src/config/, src/cook/goal_seek/, src/cook/workflow/
