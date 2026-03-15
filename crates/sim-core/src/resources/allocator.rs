//! Resource allocator implementation

use super::traits::{AllocationStrategy, Resource, Task};
use crate::utils::idents::{ResourceId, TaskId};
use crate::Result;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Assignment of a task to a resource
#[derive(Debug, Clone)]
pub struct Assignment {
    pub task_id: TaskId,
    pub resource_id: ResourceId,
}

/// Manages resource allocation for tasks
///
/// This generic allocator works with any Resource and Task types and uses pluggable
/// allocation strategies to assign tasks to resources.
///
/// # Type Parameters
///
/// * `R` - Resource type
/// * `T` - Task type
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::{ResourceAllocator, LeastLoaded};
///
/// let mut allocator = ResourceAllocator::<MyResource, MyTask>::new();
///
/// // Add resources
/// allocator.add_resource(resource1);
/// allocator.add_resource(resource2);
///
/// // Assign tasks using least-loaded strategy
/// let assignments = allocator.assign_tasks(&tasks, &LeastLoaded)?;
/// ```
pub struct ResourceAllocator<R: Resource, T: Task> {
    /// Map of resource ID to resource
    resources: HashMap<ResourceId, R>,
    /// Map of task ID to task
    pending_tasks: HashMap<TaskId, T>,
    /// Currently assigned tasks (task_id -> resource_id)
    assignments: HashMap<TaskId, ResourceId>,
    /// Phantom data
    _phantom: PhantomData<(R, T)>,
}

impl<R: Resource, T: Task> ResourceAllocator<R, T> {
    /// Create a new resource allocator
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            pending_tasks: HashMap::new(),
            assignments: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    /// Add a resource
    pub fn add_resource(&mut self, resource: R) {
        let id = resource.id();
        self.resources.insert(id, resource);
    }

    /// Remove a resource
    pub fn remove_resource(&mut self, id: ResourceId) -> Option<R> {
        self.resources.remove(&id)
    }

    /// Add a pending task
    pub fn add_task(&mut self, task: T) {
        let id = task.id();
        self.pending_tasks.insert(id, task);
    }

    /// Remove a task
    pub fn remove_task(&mut self, id: TaskId) -> Option<T> {
        self.pending_tasks.remove(&id)
    }

    /// Get resource count
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    /// Get pending task count
    pub fn pending_task_count(&self) -> usize {
        self.pending_tasks.len()
    }

    /// Assign pending tasks to available resources
    ///
    /// # Arguments
    ///
    /// * `strategy` - Allocation strategy to use
    ///
    /// # Returns
    ///
    /// Vector of assignments made
    pub fn assign_tasks(
        &mut self,
        strategy: &dyn AllocationStrategy<R, T>,
    ) -> Result<Vec<Assignment>> {
        let mut assignments = Vec::new();
        let mut to_remove = Vec::new();

        for (task_id, task) in &self.pending_tasks {
            // Find available resources of the required type
            let available: Vec<&R> = self
                .resources
                .values()
                .filter(|r| {
                    r.resource_type() == task.required_resource_type() && r.is_available()
                })
                .collect();

            if let Some(resource_id) = strategy.assign(task, &available) {
                assignments.push(Assignment {
                    task_id: *task_id,
                    resource_id,
                });
                self.assignments.insert(*task_id, resource_id);
                to_remove.push(*task_id);
            }
        }

        // Remove assigned tasks from pending
        for task_id in to_remove {
            self.pending_tasks.remove(&task_id);
        }

        Ok(assignments)
    }

    /// Get assignment for a task
    pub fn get_assignment(&self, task_id: TaskId) -> Option<ResourceId> {
        self.assignments.get(&task_id).copied()
    }

    /// Complete a task (remove assignment)
    pub fn complete_task(&mut self, task_id: TaskId) {
        self.assignments.remove(&task_id);
    }

    /// Get all assignments
    pub fn assignments(&self) -> &HashMap<TaskId, ResourceId> {
        &self.assignments
    }
}

impl<R: Resource, T: Task> Default for ResourceAllocator<R, T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::LeastLoaded;
    use chrono::Duration;

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
    }

    impl Task for TestTask {
        fn id(&self) -> TaskId {
            self.id
        }
        fn required_resource_type(&self) -> &str {
            "test"
        }
        fn estimated_duration(&self) -> Duration {
            Duration::seconds(10)
        }
    }

    #[test]
    fn test_allocator_creation() {
        let allocator = ResourceAllocator::<TestResource, TestTask>::new();
        assert_eq!(allocator.resource_count(), 0);
        assert_eq!(allocator.pending_task_count(), 0);
    }

    #[test]
    fn test_add_resources_and_tasks() {
        let mut allocator = ResourceAllocator::new();

        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        };
        let task = TestTask { id: TaskId::new() };

        allocator.add_resource(resource);
        allocator.add_task(task);

        assert_eq!(allocator.resource_count(), 1);
        assert_eq!(allocator.pending_task_count(), 1);
    }

    #[test]
    fn test_assign_tasks() {
        let mut allocator = ResourceAllocator::new();

        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        };
        let task = TestTask { id: TaskId::new() };

        let resource_id = resource.id();
        let task_id = task.id();

        allocator.add_resource(resource);
        allocator.add_task(task);

        let assignments = allocator.assign_tasks(&LeastLoaded).unwrap();

        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].task_id, task_id);
        assert_eq!(assignments[0].resource_id, resource_id);
        assert_eq!(allocator.pending_task_count(), 0);
    }

    #[test]
    fn test_complete_task() {
        let mut allocator = ResourceAllocator::new();

        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        };
        let task = TestTask { id: TaskId::new() };

        let task_id = task.id();

        allocator.add_resource(resource);
        allocator.add_task(task);

        allocator.assign_tasks(&LeastLoaded).unwrap();
        assert!(allocator.get_assignment(task_id).is_some());

        allocator.complete_task(task_id);
        assert!(allocator.get_assignment(task_id).is_none());
    }
}
