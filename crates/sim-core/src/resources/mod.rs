//! Resource allocation and management
//!
//! This module provides abstractions for capacity-constrained resources that process tasks.

mod allocator;
mod factory;
mod strategies;
mod traits;

pub use allocator::{Assignment, ResourceAllocator};
pub use factory::TaskFactory;
pub use strategies::{LeastLoaded, Random, RoundRobin};
pub use traits::{AllocationStrategy, Resource, Task};
