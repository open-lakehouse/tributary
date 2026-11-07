//! Core traits for resources and tasks

use crate::utils::idents::{ResourceId, TaskId, WorkflowInstanceId};
use chrono::Duration;
use std::fmt;

/// A resource that can process tasks
///
/// Resources represent capacity-constrained processors like machines, stations,
/// checkout lanes, support agents, etc.
///
/// # Example
///
/// ```rust,ignore
/// struct CheckoutLane {
///     id: ResourceId,
///     capacity: usize,
///     current_load: usize,
/// }
///
/// impl Resource for CheckoutLane {
///     fn id(&self) -> ResourceId {
///         self.id
///     }
///
///     fn resource_type(&self) -> &str {
///         "checkout_lane"
///     }
///
///     fn capacity(&self) -> usize {
///         self.capacity
///     }
///
///     fn current_load(&self) -> usize {
///         self.current_load
///     }
/// }
/// ```
pub trait Resource: Send + Sync + fmt::Debug {
    /// Unique identifier for this resource
    fn id(&self) -> ResourceId;

    /// Type of resource (e.g., "kitchen_station", "checkout_lane", "loading_dock")
    fn resource_type(&self) -> &str;

    /// Maximum capacity (number of concurrent tasks)
    fn capacity(&self) -> usize;

    /// Current load (number of active tasks)
    fn current_load(&self) -> usize;

    /// Check if resource is available for new tasks
    fn is_available(&self) -> bool {
        self.current_load() < self.capacity()
    }

    /// Get available capacity
    fn available_capacity(&self) -> usize {
        self.capacity().saturating_sub(self.current_load())
    }
}

/// A task that requires processing by a resource
///
/// # Example
///
/// ```rust,ignore
/// struct OrderItem {
///     id: TaskId,
///     item_type: String,
///     prep_time: Duration,
/// }
///
/// impl Task for OrderItem {
///     fn id(&self) -> TaskId {
///         self.id
///     }
///
///     fn required_resource_type(&self) -> &str {
///         &self.item_type
///     }
///
///     fn estimated_duration(&self) -> Duration {
///         self.prep_time
///     }
/// }
/// ```
pub trait Task: Send + Sync + fmt::Debug {
    /// Unique identifier for this task
    fn id(&self) -> TaskId;

    /// Type of resource required (must match Resource::resource_type)
    fn required_resource_type(&self) -> &str;

    /// Estimated duration to complete this task
    fn estimated_duration(&self) -> Duration;

    /// Optional priority (higher = more urgent)
    fn priority(&self) -> i32 {
        0
    }

    /// Optional workflow instance this task belongs to
    ///
    /// Returns `Some(instance_id)` if this task was created by a workflow,
    /// allowing the simulation to coordinate task completion with workflow progression.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// impl Task for Order {
    ///     fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
    ///         self.workflow_instance_id
    ///     }
    /// }
    /// ```
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        None
    }
}

/// A resource that can accept the task being allocated, as seen by an
/// [`AllocationStrategy`].
///
/// The allocator tracks capacity itself, so strategies see the allocator's
/// bookkeeping rather than the domain resource object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceCandidate<'a> {
    /// Resource identifier
    pub id: ResourceId,
    /// Resource type (always equal to the task's required type)
    pub resource_type: &'a str,
    /// Total capacity
    pub capacity: usize,
    /// Free capacity (always at least 1 for a candidate)
    pub available: usize,
}

impl ResourceCandidate<'_> {
    /// Units of capacity currently in use
    pub fn current_load(&self) -> usize {
        self.capacity.saturating_sub(self.available)
    }
}

/// The task being allocated, as seen by an [`AllocationStrategy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskCandidate<'a> {
    /// Task identifier
    pub id: TaskId,
    /// Resource type the task requires
    pub required_resource_type: &'a str,
    /// Task priority (higher = more urgent)
    pub priority: i32,
    /// Owning workflow instance, if any
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

/// Strategy for choosing which resource a pending task is assigned to
///
/// The allocator calls [`assign`](Self::assign) once per pending task, in
/// priority order (highest first, ties in insertion order), passing only the
/// resources of the required type that have free capacity. Candidates are in
/// the order resources were added.
///
/// # Example
///
/// ```
/// use sim_core::resources::{AllocationStrategy, ResourceCandidate, TaskCandidate};
/// use sim_core::ResourceId;
///
/// /// Always pick the resource with the most free capacity.
/// struct MostFree;
///
/// impl AllocationStrategy for MostFree {
///     fn assign(
///         &self,
///         _task: &TaskCandidate<'_>,
///         candidates: &[ResourceCandidate<'_>],
///     ) -> Option<ResourceId> {
///         candidates.iter().max_by_key(|c| c.available).map(|c| c.id)
///     }
/// }
/// ```
pub trait AllocationStrategy: Send + Sync {
    /// Choose a resource for `task` from `candidates`
    ///
    /// Returning `None` leaves the task pending. Returning an ID that is not
    /// among `candidates` is an error.
    fn assign(
        &self,
        task: &TaskCandidate<'_>,
        candidates: &[ResourceCandidate<'_>],
    ) -> Option<ResourceId>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestResource {
        id: ResourceId,
        capacity: usize,
        load: usize,
    }

    impl Resource for TestResource {
        fn id(&self) -> ResourceId {
            self.id
        }

        fn resource_type(&self) -> &str {
            "test"
        }

        fn capacity(&self) -> usize {
            self.capacity
        }

        fn current_load(&self) -> usize {
            self.load
        }
    }

    #[derive(Debug)]
    struct TestTask {
        id: TaskId,
        duration: Duration,
    }

    impl Task for TestTask {
        fn id(&self) -> TaskId {
            self.id
        }

        fn required_resource_type(&self) -> &str {
            "test"
        }

        fn estimated_duration(&self) -> Duration {
            self.duration
        }
    }

    #[test]
    fn test_resource_availability() {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 3,
        };

        assert!(resource.is_available());
        assert_eq!(resource.available_capacity(), 2);
    }

    #[test]
    fn test_resource_full() {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 5,
        };

        assert!(!resource.is_available());
        assert_eq!(resource.available_capacity(), 0);
    }

    #[test]
    fn test_task_creation() {
        let task = TestTask {
            id: TaskId::new(),
            duration: Duration::seconds(30),
        };

        assert_eq!(task.required_resource_type(), "test");
        assert_eq!(task.estimated_duration(), Duration::seconds(30));
        assert_eq!(task.priority(), 0);
    }
}
