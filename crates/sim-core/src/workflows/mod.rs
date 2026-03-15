//! Multi-stage workflow processing
//!
//! This module provides abstractions for workflows that consist of multiple sequential steps.

mod engine;
mod schema;
mod traits;

#[cfg(test)]
mod arrow_tests;

pub use engine::{WorkflowEngine, WorkflowStepEvent};
pub use schema::{workflow_instance_schema, InstanceStatus};
pub use traits::{Workflow, WorkflowInstance, WorkflowStep};
