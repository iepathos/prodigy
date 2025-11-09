## Sub-Workflows

Execute child workflows as part of a parent workflow. Sub-workflows can run in parallel and have their own parameters and outputs.

*Implementation Status: Sub-workflow configuration, validation (`validate_sub_workflows` in `composer.rs:381-395`), and composition are fully implemented. Sub-workflow definitions work correctly and are validated. The `SubWorkflowExecutor` structure exists (`sub_workflow.rs:181-227`) but execution integration with the main workflow executor runtime is in progress.*

