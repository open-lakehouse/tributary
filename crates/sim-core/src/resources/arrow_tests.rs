//! Comprehensive tests for Arrow RecordBatch-based resource allocation
//!
//! This test suite validates the RecordBatch migration with:
//! - Basic operations (add, remove, query)
//! - Bulk allocation scenarios
//! - Phase 5 workflow integration
//! - Performance benchmarks
//! - Edge cases

use super::allocator::ResourceAllocator;
use super::schema::task_status;
use super::traits::{AllocationStrategy, Resource, Task};
use crate::utils::idents::{ResourceId, TaskId, WorkflowInstanceId};
use arrow::array::{Array, BinaryArray, StringArray};
use chrono::{Duration, Utc};

/// Test resource implementation
#[derive(Debug, Clone)]
struct TestResource {
    id: ResourceId,
    capacity: usize,
    load: usize,
    resource_type: String,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }
    fn resource_type(&self) -> &str {
        &self.resource_type
    }
    fn capacity(&self) -> usize {
        self.capacity
    }
    fn current_load(&self) -> usize {
        self.load
    }
}

/// Test task implementation
#[derive(Debug, Clone)]
struct TestTask {
    id: TaskId,
    task_type: String,
    duration: Duration,
    workflow_instance_id: Option<WorkflowInstanceId>,
    priority: i32,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }
    fn required_resource_type(&self) -> &str {
        &self.task_type
    }
    fn estimated_duration(&self) -> Duration {
        self.duration
    }
    fn priority(&self) -> i32 {
        self.priority
    }
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

/// Simple test allocation strategy
struct TestStrategy;

impl AllocationStrategy<TestResource, TestTask> for TestStrategy {
    fn assign(&self, _task: &TestTask, available: &[&TestResource]) -> Option<ResourceId> {
        available.first().map(|r| r.id())
    }
}

#[test]
fn test_1_add_100_resources_verify_structure() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 100 resources
    for i in 0..100 {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
            resource_type: format!("type_{}", i % 5), // 5 different types
        };
        allocator.add_resource(resource).unwrap();
    }

    // Verify count
    assert_eq!(allocator.resource_count(), 100);

    // Verify RecordBatch structure
    let resources_batch = &allocator.resources;
    assert_eq!(resources_batch.num_rows(), 100);
    assert_eq!(resources_batch.num_columns(), 6);

    // Verify resource types are correctly stored
    let type_array = resources_batch.column(1).as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(type_array.value(0), "type_0");
    assert_eq!(type_array.value(50), "type_0");
}

#[test]
fn test_2_add_100_tasks_with_workflow_ids() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Create some workflow instances
    let workflow_ids: Vec<WorkflowInstanceId> = (0..10).map(|_| WorkflowInstanceId::new()).collect();

    // Add 100 tasks, assigning them to workflows
    for i in 0..100 {
        let task = TestTask {
            id: TaskId::new(),
            task_type: format!("type_{}", i % 5),
            duration: Duration::seconds(10),
            workflow_instance_id: Some(workflow_ids[i % 10]),
            priority: (i as i32) % 3,
        };
        allocator.add_task(task).unwrap();
    }

    // Verify count
    assert_eq!(allocator.pending_task_count(), 100);

    // Verify RecordBatch structure
    let tasks_batch = &allocator.tasks;
    assert_eq!(tasks_batch.num_rows(), 100);
    assert_eq!(tasks_batch.num_columns(), 8);

    // Verify workflow IDs are stored
    let workflow_array = tasks_batch.column(5).as_any().downcast_ref::<BinaryArray>().unwrap();
    for i in 0..100 {
        assert!(!workflow_array.is_null(i), "Workflow ID should not be null for task {}", i);
    }

    // Verify all tasks are pending
    let status_array = tasks_batch.column(2).as_any().downcast_ref::<StringArray>().unwrap();
    for i in 0..100 {
        assert_eq!(status_array.value(i), task_status::PENDING);
    }
}

#[test]
fn test_3_bulk_allocation_100_tasks_to_100_resources() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 100 resources of type "test"
    let resource_ids: Vec<ResourceId> = (0..100)
        .map(|_| {
            let resource = TestResource {
                id: ResourceId::new(),
                capacity: 10,
                load: 0,
                resource_type: "test".to_string(),
            };
            let id = resource.id();
            allocator.add_resource(resource).unwrap();
            id
        })
        .collect();

    // Add 100 tasks of type "test"
    let task_ids: Vec<TaskId> = (0..100)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(WorkflowInstanceId::new()),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Allocate tasks
    let current_time = Utc::now();
    let assignments = allocator.assign_tasks(&TestStrategy, current_time).unwrap();

    // Verify all tasks were assigned
    assert_eq!(assignments.len(), 100);

    // Verify no pending tasks remain
    assert_eq!(allocator.pending_task_count(), 0);

    // Verify all tasks have assignments
    for task_id in &task_ids {
        assert!(allocator.get_assignment(*task_id).is_some());
    }
}

#[test]
fn test_4_update_task_status_transitions() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    let resource = TestResource {
        id: ResourceId::new(),
        capacity: 10,
        load: 0,
        resource_type: "test".to_string(),
    };
    allocator.add_resource(resource).unwrap();

    let task = TestTask {
        id: TaskId::new(),
        task_type: "test".to_string(),
        duration: Duration::seconds(10),
        workflow_instance_id: Some(WorkflowInstanceId::new()),
        priority: 0,
    };
    let task_id = task.id();
    allocator.add_task(task).unwrap();

    // Initial state: pending
    assert_eq!(allocator.pending_task_count(), 1);

    // Allocate: pending -> allocated
    let current_time = Utc::now();
    allocator.assign_tasks(&TestStrategy, current_time).unwrap();
    assert_eq!(allocator.pending_task_count(), 0);
    assert!(allocator.get_assignment(task_id).is_some());

    // Complete: allocated -> completed
    allocator.complete_task(task_id).unwrap();

    // Verify task status is completed in RecordBatch
    let tasks_batch = &allocator.tasks;
    let task_row = allocator.task_id_to_row.get(&task_id).unwrap();
    let status_array = tasks_batch.column(2).as_any().downcast_ref::<StringArray>().unwrap();
    assert_eq!(status_array.value(*task_row), task_status::COMPLETED);
}

#[test]
fn test_5_release_resources_verify_capacity_updates() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    let resource = TestResource {
        id: ResourceId::new(),
        capacity: 10,
        load: 0,
        resource_type: "test".to_string(),
    };
    let resource_id = resource.id();
    allocator.add_resource(resource).unwrap();

    // Check initial available capacity
    let resources_batch = &allocator.resources;
    let resource_row = allocator.resource_id_to_row.get(&resource_id).unwrap();
    let available_array = resources_batch.column(3).as_any().downcast_ref::<arrow::array::Int64Array>().unwrap();
    assert_eq!(available_array.value(*resource_row), 10);

    // Add and allocate a task
    let task = TestTask {
        id: TaskId::new(),
        task_type: "test".to_string(),
        duration: Duration::seconds(10),
        workflow_instance_id: None,
        priority: 0,
    };
    let task_id = task.id();
    allocator.add_task(task).unwrap();

    let current_time = Utc::now();
    allocator.assign_tasks(&TestStrategy, current_time).unwrap();

    // Verify capacity decreased
    let resources_batch = &allocator.resources;
    let resource_row = allocator.resource_id_to_row.get(&resource_id).unwrap();
    let available_array = resources_batch.column(3).as_any().downcast_ref::<arrow::array::Int64Array>().unwrap();
    assert_eq!(available_array.value(*resource_row), 9);

    // Complete task and verify capacity increased
    allocator.complete_task(task_id).unwrap();

    let resources_batch = &allocator.resources;
    let resource_row = allocator.resource_id_to_row.get(&resource_id).unwrap();
    let available_array = resources_batch.column(3).as_any().downcast_ref::<arrow::array::Int64Array>().unwrap();
    assert_eq!(available_array.value(*resource_row), 10);
}

#[test]
fn test_6_query_tasks_by_workflow_instance() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add resources
    for _ in 0..10 {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
            resource_type: "test".to_string(),
        };
        allocator.add_resource(resource).unwrap();
    }

    // Create two workflow instances
    let workflow_id_1 = WorkflowInstanceId::new();
    let workflow_id_2 = WorkflowInstanceId::new();

    // Add tasks for workflow 1
    let workflow_1_tasks: Vec<TaskId> = (0..5)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(workflow_id_1),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Add tasks for workflow 2
    let workflow_2_tasks: Vec<TaskId> = (0..5)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(workflow_id_2),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Query tasks by workflow (manual implementation since DataFusion not fully integrated)
    let workflow_array = allocator.tasks.column(5).as_any().downcast_ref::<BinaryArray>().unwrap();
    let id_array = allocator.tasks.column(0).as_any().downcast_ref::<BinaryArray>().unwrap();

    let mut workflow_1_found = 0;
    for i in 0..allocator.tasks.num_rows() {
        if !workflow_array.is_null(i) {
            let wf_id = WorkflowInstanceId::from_bytes(workflow_array.value(i)).unwrap();
            if wf_id == workflow_id_1 {
                workflow_1_found += 1;
            }
        }
    }

    assert_eq!(workflow_1_found, 5);
}

#[test]
fn test_7_get_completed_tasks_with_workflow_ids() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add resources
    for _ in 0..5 {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
            resource_type: "test".to_string(),
        };
        allocator.add_resource(resource).unwrap();
    }

    let workflow_id = WorkflowInstanceId::new();

    // Add tasks with short duration
    let task_ids: Vec<TaskId> = (0..5)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(workflow_id),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Allocate at T=0
    let start_time = Utc::now();
    allocator.assign_tasks(&TestStrategy, start_time).unwrap();

    // Check at T=5 (not complete)
    let mid_time = start_time + Duration::seconds(5);
    let completed = allocator.get_completed_tasks(mid_time).unwrap();
    assert_eq!(completed.len(), 0);

    // Check at T=10 (complete)
    let end_time = start_time + Duration::seconds(10);
    let completed = allocator.get_completed_tasks(end_time).unwrap();
    assert_eq!(completed.len(), 5);

    // Verify all have correct workflow ID
    for (_, wf_id) in &completed {
        assert_eq!(*wf_id, workflow_id);
    }
}

#[test]
fn test_8_edge_case_no_available_resources() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add NO resources

    // Add tasks
    for _ in 0..10 {
        let task = TestTask {
            id: TaskId::new(),
            task_type: "test".to_string(),
            duration: Duration::seconds(10),
            workflow_instance_id: Some(WorkflowInstanceId::new()),
            priority: 0,
        };
        allocator.add_task(task).unwrap();
    }

    // Try to allocate
    let current_time = Utc::now();
    let assignments = allocator.assign_tasks(&TestStrategy, current_time).unwrap();

    // No assignments should be made
    assert_eq!(assignments.len(), 0);
    assert_eq!(allocator.pending_task_count(), 10);
}

#[test]
fn test_9_edge_case_resource_exhaustion() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 1 resource with capacity 1
    let resource = TestResource {
        id: ResourceId::new(),
        capacity: 1,
        load: 0,
        resource_type: "test".to_string(),
    };
    allocator.add_resource(resource).unwrap();

    // Add 10 tasks
    for _ in 0..10 {
        let task = TestTask {
            id: TaskId::new(),
            task_type: "test".to_string(),
            duration: Duration::seconds(10),
            workflow_instance_id: Some(WorkflowInstanceId::new()),
            priority: 0,
        };
        allocator.add_task(task).unwrap();
    }

    // Allocate
    let current_time = Utc::now();
    let assignments = allocator.assign_tasks(&TestStrategy, current_time).unwrap();

    // Only 1 task should be assigned
    assert_eq!(assignments.len(), 1);
    assert_eq!(allocator.pending_task_count(), 9);
}

#[test]
fn test_10_performance_1000_tasks_100_resources() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 100 resources
    let start_resources = std::time::Instant::now();
    for _ in 0..100 {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 20,
            load: 0,
            resource_type: "test".to_string(),
        };
        allocator.add_resource(resource).unwrap();
    }
    let resources_time = start_resources.elapsed();
    println!("Adding 100 resources: {:?}", resources_time);

    // Add 1000 tasks
    let start_tasks = std::time::Instant::now();
    for _ in 0..1000 {
        let task = TestTask {
            id: TaskId::new(),
            task_type: "test".to_string(),
            duration: Duration::seconds(10),
            workflow_instance_id: Some(WorkflowInstanceId::new()),
            priority: 0,
        };
        allocator.add_task(task).unwrap();
    }
    let tasks_time = start_tasks.elapsed();
    println!("Adding 1000 tasks: {:?}", tasks_time);

    // Allocate all tasks
    let start_allocate = std::time::Instant::now();
    let current_time = Utc::now();
    let assignments = allocator.assign_tasks(&TestStrategy, current_time).unwrap();
    let allocate_time = start_allocate.elapsed();
    println!("Allocating 1000 tasks: {:?}", allocate_time);
    println!("Assignments made: {}", assignments.len());

    // Verify allocations
    assert!(assignments.len() > 0);
    assert!(allocate_time.as_millis() < 5000, "Allocation should be fast");
}

#[test]
fn test_11_remove_resource_and_rebuild_batch() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 10 resources
    let resource_ids: Vec<ResourceId> = (0..10)
        .map(|_| {
            let resource = TestResource {
                id: ResourceId::new(),
                capacity: 10,
                load: 0,
                resource_type: "test".to_string(),
            };
            let id = resource.id();
            allocator.add_resource(resource).unwrap();
            id
        })
        .collect();

    assert_eq!(allocator.resource_count(), 10);

    // Remove the 5th resource
    allocator.remove_resource(resource_ids[4]).unwrap();

    // Verify count
    assert_eq!(allocator.resource_count(), 9);

    // Verify removed resource is not in index
    assert!(!allocator.resource_id_to_row.contains_key(&resource_ids[4]));

    // Verify other resources are still accessible
    assert!(allocator.resource_id_to_row.contains_key(&resource_ids[0]));
    assert!(allocator.resource_id_to_row.contains_key(&resource_ids[9]));
}

#[test]
fn test_12_remove_task_and_rebuild_batch() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add 10 tasks
    let task_ids: Vec<TaskId> = (0..10)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(WorkflowInstanceId::new()),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    assert_eq!(allocator.pending_task_count(), 10);

    // Remove the 5th task
    let result = allocator.remove_task(task_ids[4]).unwrap();
    assert!(result.is_some());

    // Verify count
    assert_eq!(allocator.pending_task_count(), 9);

    // Verify removed task is not in index
    assert!(!allocator.task_id_to_row.contains_key(&task_ids[4]));

    // Verify other tasks are still accessible
    assert!(allocator.task_id_to_row.contains_key(&task_ids[0]));
    assert!(allocator.task_id_to_row.contains_key(&task_ids[9]));
}

#[test]
fn test_13_mixed_workflow_and_non_workflow_tasks() {
    let mut allocator = ResourceAllocator::<TestResource, TestTask>::new();

    // Add resources
    for _ in 0..5 {
        let resource = TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
            resource_type: "test".to_string(),
        };
        allocator.add_resource(resource).unwrap();
    }

    let workflow_id = WorkflowInstanceId::new();

    // Add workflow tasks
    let workflow_task_ids: Vec<TaskId> = (0..5)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: Some(workflow_id),
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Add non-workflow tasks
    let regular_task_ids: Vec<TaskId> = (0..5)
        .map(|_| {
            let task = TestTask {
                id: TaskId::new(),
                task_type: "test".to_string(),
                duration: Duration::seconds(10),
                workflow_instance_id: None,
                priority: 0,
            };
            let id = task.id();
            allocator.add_task(task).unwrap();
            id
        })
        .collect();

    // Allocate all
    let start_time = Utc::now();
    allocator.assign_tasks(&TestStrategy, start_time).unwrap();

    // Complete at T=10
    let end_time = start_time + Duration::seconds(10);
    let completed = allocator.get_completed_tasks(end_time).unwrap();

    // Only workflow tasks should be returned
    assert_eq!(completed.len(), 5);

    // All completed tasks should have workflow ID
    for (_, wf_id) in &completed {
        assert_eq!(*wf_id, workflow_id);
    }
}
