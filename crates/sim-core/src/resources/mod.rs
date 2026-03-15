//! Resource allocation and management
//!
//! This module provides abstractions for capacity-constrained resources that process tasks.

mod allocator;
#[cfg(test)]
mod arrow_tests;
mod factory;
mod schema;
mod strategies;
mod traits;

pub use allocator::{Assignment, ResourceAllocator};
pub use factory::TaskFactory;
pub use schema::{resource_schema, task_schema, task_status};
pub use strategies::{LeastLoaded, Random, RoundRobin};
pub use traits::{AllocationStrategy, Resource, Task};
