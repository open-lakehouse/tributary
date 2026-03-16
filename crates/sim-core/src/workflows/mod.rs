//! Multi-stage workflow processing
//!
//! This module provides abstractions for workflows that consist of multiple sequential steps.

mod engine;
mod instance_table;
mod schema;
mod traits;

#[cfg(test)]
mod arrow_tests;

pub use engine::{WorkflowEngine, WorkflowStepEvent};
pub use instance_table::{
    BatchUpdateList, WorkflowInstanceRowUpdate, WorkflowInstanceTable, WorkflowInstanceUpdate,
};
pub use schema::{InstanceStatus, workflow_instance_schema};
pub use traits::{Workflow, WorkflowInstance, WorkflowStep};
