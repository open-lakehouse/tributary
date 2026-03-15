//! Resource allocator implementation

use super::traits::{AllocationStrategy, Resource, Task};
use crate::utils::idents::{ResourceId, TaskId, WorkflowInstanceId};
use crate::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::marker::PhantomData;

/// Assignment of a task to a resource
#[derive(Debug, Clone)]
pub struct Assignment {
    pub task_id: TaskId,
    pub resource_id: ResourceId,
}

/// Track when a task was allocated and the task itself
#[derive(Debug, Clone)]
struct AllocatedTask<T: Task> {
    task: T,
    allocated_at: DateTime<Utc>,
    resource_id: ResourceId,
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
    /// Currently assigned tasks with allocation time and task data
    allocated_tasks: HashMap<TaskId, AllocatedTask<T>>,
    /// Phantom data
    _phantom: PhantomData<(R, T)>,
}

impl<R: Resource, T: Task> ResourceAllocator<R, T> {
    /// Create a new resource allocator
    pub fn new() -> Self {
        Self {
            resources: HashMap::new(),
            pending_tasks: HashMap::new(),
            allocated_tasks: HashMap::new(),
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

    /// Get iterator over pending tasks
    pub fn pending_tasks(&self) -> impl Iterator<Item = (TaskId, &T)> {
        self.pending_tasks.iter().map(|(id, task)| (*id, task))
    }

    /// Assign pending tasks to available resources
    ///
    /// # Arguments
    ///
    /// * `strategy` - Allocation strategy to use
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// Vector of assignments made
    pub fn assign_tasks(
        &mut self,
        strategy: &dyn AllocationStrategy<R, T>,
        current_time: DateTime<Utc>,
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
                to_remove.push(*task_id);
            }
        }

        // Move assigned tasks from pending to allocated
        for task_id in to_remove {
            if let Some(task) = self.pending_tasks.remove(&task_id) {
                let resource_id = assignments
                    .iter()
                    .find(|a| a.task_id == task_id)
                    .map(|a| a.resource_id)
                    .expect("Assignment should exist for removed task");

                self.allocated_tasks.insert(
                    task_id,
                    AllocatedTask {
                        task,
                        allocated_at: current_time,
                        resource_id,
                    },
                );
            }
        }

        Ok(assignments)
    }

    /// Get assignment for a task
    pub fn get_assignment(&self, task_id: TaskId) -> Option<ResourceId> {
        self.allocated_tasks
            .get(&task_id)
            .map(|allocated| allocated.resource_id)
    }

    /// Complete a task (remove assignment)
    pub fn complete_task(&mut self, task_id: TaskId) {
        self.allocated_tasks.remove(&task_id);
    }

    /// Get all assignments as a map of task_id -> resource_id
    pub fn assignments(&self) -> HashMap<TaskId, ResourceId> {
        self.allocated_tasks
            .iter()
            .map(|(task_id, allocated)| (*task_id, allocated.resource_id))
            .collect()
    }

    /// Get tasks that completed since a given time
    ///
    /// Returns a vector of (TaskId, WorkflowInstanceId) pairs for tasks that:
    /// - Are currently allocated to resources
    /// - Have workflow instance IDs
    /// - Have been allocated long enough to complete (allocated_at + estimated_duration <= current_time)
    ///
    /// This method also removes the completed tasks from the allocator.
    pub fn get_completed_tasks(
        &mut self,
        current_time: DateTime<Utc>,
    ) -> Vec<(TaskId, WorkflowInstanceId)> {
        let mut completed = Vec::new();

        // Find completed tasks
        for (task_id, allocated) in &self.allocated_tasks {
            let completion_time = allocated.allocated_at + allocated.task.estimated_duration();
            if completion_time <= current_time
                && let Some(instance_id) = allocated.task.workflow_instance_id()
            {
                completed.push((*task_id, instance_id));
            }
        }

        // Remove completed tasks
        for (task_id, _) in &completed {
            self.allocated_tasks.remove(task_id);
        }

        completed
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

        let current_time = Utc::now();
        let assignments = allocator.assign_tasks(&LeastLoaded, current_time).unwrap();

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

        let current_time = Utc::now();
        allocator.assign_tasks(&LeastLoaded, current_time).unwrap();
        assert!(allocator.get_assignment(task_id).is_some());

        allocator.complete_task(task_id);
        assert!(allocator.get_assignment(task_id).is_none());
    }

    #[test]
    fn test_get_completed_tasks() {
        use crate::utils::idents::WorkflowInstanceId;

        #[derive(Debug)]
        struct WorkflowTask {
            id: TaskId,
            instance_id: WorkflowInstanceId,
            duration: Duration,
        }

        impl Task for WorkflowTask {
            fn id(&self) -> TaskId {
                self.id
            }
            fn required_resource_type(&self) -> &str {
                "test"
            }
            fn estimated_duration(&self) -> Duration {
                self.duration
            }
            fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
                Some(self.instance_id)
            }
        }

        let mut allocator = ResourceAllocator::new();

        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        };

        let instance_id = WorkflowInstanceId::new();
        let task = WorkflowTask {
            id: TaskId::new(),
            instance_id,
            duration: Duration::seconds(10),
        };

        let task_id = task.id();

        allocator.add_resource(resource);
        allocator.add_task(task);

        // Allocate task at T=0
        let start_time = Utc::now();
        allocator.assign_tasks(&LeastLoaded, start_time).unwrap();

        // Check at T=5 (not complete yet)
        let mid_time = start_time + Duration::seconds(5);
        let completed = allocator.get_completed_tasks(mid_time);
        assert_eq!(completed.len(), 0);
        assert!(allocator.get_assignment(task_id).is_some());

        // Check at T=10 (just completed)
        let end_time = start_time + Duration::seconds(10);
        let completed = allocator.get_completed_tasks(end_time);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, task_id);
        assert_eq!(completed[0].1, instance_id);

        // Task should be removed after completion
        assert!(allocator.get_assignment(task_id).is_none());
    }

    #[test]
    fn test_get_completed_tasks_ignores_non_workflow_tasks() {
        use crate::utils::idents::WorkflowInstanceId;

        #[derive(Debug)]
        struct WorkflowTask {
            id: TaskId,
            instance_id: Option<WorkflowInstanceId>,
            duration: Duration,
        }

        impl Task for WorkflowTask {
            fn id(&self) -> TaskId {
                self.id
            }
            fn required_resource_type(&self) -> &str {
                "test"
            }
            fn estimated_duration(&self) -> Duration {
                self.duration
            }
            fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
                self.instance_id
            }
        }

        let mut allocator = ResourceAllocator::new();

        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        };

        // Task WITH workflow instance ID
        let instance_id = WorkflowInstanceId::new();
        let workflow_task = WorkflowTask {
            id: TaskId::new(),
            instance_id: Some(instance_id),
            duration: Duration::seconds(10),
        };
        let workflow_task_id = workflow_task.id();

        // Task WITHOUT workflow instance ID (regular task)
        let regular_task = WorkflowTask {
            id: TaskId::new(),
            instance_id: None,
            duration: Duration::seconds(10),
        };
        let regular_task_id = regular_task.id();

        allocator.add_resource(resource);
        allocator.add_task(workflow_task);
        allocator.add_task(regular_task);

        // Allocate tasks at T=0
        let start_time = Utc::now();
        allocator.assign_tasks(&LeastLoaded, start_time).unwrap();

        // Both tasks should be assigned
        assert!(allocator.get_assignment(workflow_task_id).is_some());
        assert!(allocator.get_assignment(regular_task_id).is_some());

        // Check at T=10 (both complete)
        let end_time = start_time + Duration::seconds(10);
        let completed = allocator.get_completed_tasks(end_time);

        // Only workflow task should be returned
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, workflow_task_id);
        assert_eq!(completed[0].1, instance_id);

        // Workflow task removed, regular task still assigned
        assert!(allocator.get_assignment(workflow_task_id).is_none());
        assert!(allocator.get_assignment(regular_task_id).is_some());
    }

    #[test]
    fn test_multiple_tasks_complete_simultaneously() {
        use crate::utils::idents::WorkflowInstanceId;

        #[derive(Debug)]
        struct WorkflowTask {
            id: TaskId,
            instance_id: WorkflowInstanceId,
            duration: Duration,
        }

        impl Task for WorkflowTask {
            fn id(&self) -> TaskId {
                self.id
            }
            fn required_resource_type(&self) -> &str {
                "test"
            }
            fn estimated_duration(&self) -> Duration {
                self.duration
            }
            fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
                Some(self.instance_id)
            }
        }

        let mut allocator = ResourceAllocator::new();

        // Add multiple resources
        for _ in 0..3 {
            allocator.add_resource(TestResource {
                id: ResourceId::new(),
                capacity: 5,
                load: 0,
            });
        }

        // Add 3 tasks with same duration
        let instance_ids: Vec<WorkflowInstanceId> = (0..3).map(|_| WorkflowInstanceId::new()).collect();
        let task_ids: Vec<TaskId> = instance_ids
            .iter()
            .map(|&instance_id| {
                let task = WorkflowTask {
                    id: TaskId::new(),
                    instance_id,
                    duration: Duration::seconds(10),
                };
                let task_id = task.id();
                allocator.add_task(task);
                task_id
            })
            .collect();

        // Allocate all tasks at T=0
        let start_time = Utc::now();
        allocator.assign_tasks(&LeastLoaded, start_time).unwrap();

        // All should be assigned
        for task_id in &task_ids {
            assert!(allocator.get_assignment(*task_id).is_some());
        }

        // Check at T=10 (all complete simultaneously)
        let end_time = start_time + Duration::seconds(10);
        let completed = allocator.get_completed_tasks(end_time);

        // All 3 tasks should be returned
        assert_eq!(completed.len(), 3);

        // Verify all task IDs and instance IDs are present
        for (task_id, instance_id) in &completed {
            assert!(task_ids.contains(task_id));
            assert!(instance_ids.contains(instance_id));
        }

        // All tasks should be removed
        for task_id in &task_ids {
            assert!(allocator.get_assignment(*task_id).is_none());
        }
    }
}
