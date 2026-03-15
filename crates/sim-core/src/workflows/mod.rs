//! Multi-stage workflow processing
//!
//! This module provides abstractions for workflows that consist of multiple sequential steps.

mod engine;
mod traits;

pub use engine::{WorkflowEngine, WorkflowStepEvent};
pub use traits::{Workflow, WorkflowInstance, WorkflowStep};
