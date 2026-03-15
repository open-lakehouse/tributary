//! Core traits for resources and tasks

use crate::utils::idents::{ResourceId, TaskId};
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
}

/// Strategy for assigning tasks to resources
///
/// # Example
///
/// ```rust,ignore
/// struct PriorityStrategy;
///
/// impl AllocationStrategy<MyResource, MyTask> for PriorityStrategy {
///     fn assign(&self, task: &MyTask, available: &[&MyResource]) -> Option<ResourceId> {
///         // Sort by priority and assign to least loaded
///         available
///             .iter()
///             .min_by_key(|r| r.current_load())
///             .map(|r| r.id())
///     }
/// }
/// ```
pub trait AllocationStrategy<R: Resource, T: Task>: Send + Sync {
    /// Assign a task to one of the available resources
    ///
    /// # Arguments
    ///
    /// * `task` - The task to assign
    /// * `available` - List of resources that can accept this task type
    ///
    /// # Returns
    ///
    /// The ID of the chosen resource, or None if no suitable resource is available
    fn assign(&self, task: &T, available: &[&R]) -> Option<ResourceId>;
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
