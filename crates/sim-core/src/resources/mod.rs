//! Resource allocation and management
//!
//! This module provides abstractions for capacity-constrained resources that process tasks.

mod allocator;
mod strategies;
mod traits;

pub use allocator::ResourceAllocator;
pub use strategies::{LeastLoaded, Random, RoundRobin};
pub use traits::{AllocationStrategy, Resource, Task};
