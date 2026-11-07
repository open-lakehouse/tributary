//! Resource allocation and management
//!
//! This module provides abstractions for capacity-constrained resources that process tasks.

mod allocator;
mod factory;
mod schema;
mod strategies;
mod traits;

pub use allocator::{Assignment, CompletedTask, ResourceAllocator};
pub use factory::TaskFactory;
pub use schema::{resource_schema, task_schema, task_status};
pub use strategies::{FirstFit, LeastLoaded, Random, RoundRobin};
pub use traits::{AllocationStrategy, Resource, ResourceCandidate, Task, TaskCandidate};
