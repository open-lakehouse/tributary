# Custom Workflows Guide

This guide covers implementing the `Workflow` trait, step-based processing, workflow state management, and integration with the WorkflowEngine.

## Table of Contents

1. [Workflow Trait Overview](#workflow-trait-overview)
2. [Step-Based Processing](#step-based-processing)
3. [Workflow State Management](#workflow-state-management)
4. [Task Factory Pattern](#task-factory-pattern)
5. [WorkflowEngine Integration](#workflowengine-integration)
6. [Testing Strategies](#testing-strategies)
7. [Example: PackageShipmentWorkflow](#example-packageshipmentworkflow)

## Workflow Trait Overview

Workflows represent multi-step processes that require coordination across multiple resources. From `/crates/sim-core/src/workflows`:

```rust
pub trait Workflow: Send + Sync {
    fn id(&self) -> WorkflowId;
    fn workflow_type(&self) -> &str;
    fn steps(&self) -> &[WorkflowStep];

    // Provided methods
    fn step_count(&self) -> usize {
        self.steps().len()
    }

    fn get_step(&self, index: usize) -> Option<&WorkflowStep> {
        self.steps().get(index)
    }
}
```

### WorkflowStep Structure

```rust
pub struct WorkflowStep {
    pub step_index: usize,
    pub required_resource_type: String,
    pub expected_duration: Duration,
    pub metadata: HashMap<String, String>,
}
```

### Core Concepts

- **Sequential Steps**: Steps execute in order (0, 1, 2, ...)
- **Resource Matching**: Each step requires a specific resource type
- **Duration Estimation**: Expected time for planning
- **Metadata**: Additional step information

## Step-Based Processing

### Workflow Lifecycle

```
┌─────────────────┐
│ Workflow Created│
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Instance Started│ ← WorkflowEngine tracks this
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Step 0: Task  │
│   Created       │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Resource Assigns│
│ and Processes   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Step 0 Complete │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│   Step 1: Task  │
│   Created       │
└────────┬────────┘
         │
        ...
         │
         ▼
┌─────────────────┐
│ All Steps Done  │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│Workflow Complete│
└─────────────────┘
```

### Defining Workflow Steps

From `/crates/sim-logistics/src/workflows/shipment.rs`:

```rust
impl PackageShipmentWorkflow {
    pub fn new(
        tracking_number: String,
        warehouse_id: String,
        destination: Point<f64>,
        weight_kg: f64,
    ) -> Self {
        let steps = vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "package_receiver".to_string(),
                expected_duration: Duration::seconds(0), // Instant
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "PackageReceived".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package arrives at warehouse".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "sorting_station".to_string(),
                expected_duration: Duration::seconds(300), // 5 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Sorting".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package sorted and routed for delivery".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "loading_dock".to_string(),
                expected_duration: Duration::seconds(360), // 6 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Loading".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package loaded onto delivery vehicle".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 3,
                required_resource_type: "driver".to_string(),
                expected_duration: Duration::seconds(1800), // 30 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "InTransit".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package in transit to destination".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 4,
                required_resource_type: "delivery_confirmer".to_string(),
                expected_duration: Duration::seconds(60), // 1 minute
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Delivered".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package delivered and confirmed".to_string(),
                    );
                    m
                },
            },
        ];

        Self {
            workflow_id: WorkflowId::new(),
            tracking_number,
            warehouse_id,
            destination,
            weight_kg,
            steps,
        }
    }
}
```

**Best Practices:**

- **Sequential Indices**: Start at 0, increment by 1
- **Descriptive Names**: Use metadata for human-readable names
- **Realistic Durations**: Based on domain analysis
- **Resource Matching**: Match existing Resource implementations
- **Rich Metadata**: Add context for debugging and visualization

### Implementing the Workflow Trait

```rust
#[derive(Debug, Clone)]
pub struct PackageShipmentWorkflow {
    pub workflow_id: WorkflowId,
    pub tracking_number: String,
    pub warehouse_id: String,
    pub destination: Point<f64>,
    pub weight_kg: f64,
    steps: Vec<WorkflowStep>,
}

impl Workflow for PackageShipmentWorkflow {
    fn id(&self) -> WorkflowId {
        self.workflow_id
    }

    fn workflow_type(&self) -> &str {
        "package_shipment"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}
```

**Key Points:**

- **Unique ID**: Each workflow instance gets unique ID
- **Type String**: Used for filtering and grouping
- **Immutable Steps**: Steps slice is read-only
- **Domain Data**: Store workflow-specific fields

## Workflow State Management

### WorkflowInstance State

The framework manages workflow state internally:

```rust
pub struct WorkflowInstance {
    pub id: WorkflowInstanceId,
    pub workflow_id: WorkflowId,
    pub workflow_type: String,
    pub current_step: usize,
    pub status: WorkflowStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum WorkflowStatus {
    Active,
    Completed,
    Failed { reason: String },
}
```

### State Progression

```rust
// Initial state
WorkflowInstance {
    id: instance_123,
    workflow_id: workflow_456,
    current_step: 0,
    status: Active,
}

// After step 0 completes
WorkflowInstance {
    current_step: 1,  // Advances
    status: Active,
}

// After all steps complete
WorkflowInstance {
    current_step: 5,  // Past last step
    status: Completed,
    completed_at: Some(timestamp),
}
```

## Task Factory Pattern

Workflows need to create tasks for each step. Use the TaskFactory trait:

```rust
pub trait TaskFactory<T: Task>: Send + Sync {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>,
    ) -> T;
}
```

### Implementing Task and TaskFactory

From `/crates/sim-logistics/src/workflows/shipment.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ShipmentTask {
    pub id: TaskId,
    pub task_type: String,
    pub tracking_number: String,
    pub weight_kg: f64,
    pub warehouse_id: String,
    pub destination: Point<f64>,
    pub required_resource_type: String,
    pub estimated_duration: Duration,
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for ShipmentTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.required_resource_type
    }

    fn estimated_duration(&self) -> Duration {
        self.estimated_duration
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

#[derive(Debug, Clone)]
pub struct ShipmentTaskFactory {
    pub tracking_number: String,
    pub weight_kg: f64,
    pub warehouse_id: String,
    pub destination: Point<f64>,
}

impl TaskFactory<ShipmentTask> for ShipmentTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>,
    ) -> ShipmentTask {
        // Determine task type based on step index
        let task_type = match step.step_index {
            0 => "receive_package",
            1 => "sort_package",
            2 => "load_package",
            3 => "transit_package",
            4 => "deliver_package",
            _ => "unknown",
        };

        ShipmentTask {
            id: TaskId::new(),
            task_type: task_type.to_string(),
            tracking_number: self.tracking_number.clone(),
            weight_kg: self.weight_kg,
            warehouse_id: self.warehouse_id.clone(),
            destination: self.destination,
            required_resource_type: step.required_resource_type.clone(),
            estimated_duration: step.expected_duration,
            workflow_instance_id: Some(instance_id),
        }
    }
}
```

**Key Patterns:**

- **Task Type Mapping**: Map step index to task type
- **Data Forwarding**: Copy workflow data to task
- **Unique Task IDs**: Generate new ID for each task
- **Instance Linking**: Include workflow_instance_id

## WorkflowEngine Integration

### Registering Workflows

```rust
use sim_core::{WorkflowEngine, WorkflowRegistry};

// Create workflow registry
let mut registry = WorkflowRegistry::new();

// Create and register workflow
let workflow = PackageShipmentWorkflow::new(
    "PKG-123".to_string(),
    "warehouse-1".to_string(),
    Point::new(-122.4, 37.8),
    2.5,
);

registry.register_workflow(workflow);

// Create workflow engine
let mut engine = WorkflowEngine::new(registry);
```

### Starting Workflow Instances

```rust
// Create task factory for this workflow
let factory = ShipmentTaskFactory::new(
    "PKG-123".to_string(),
    2.5,
    "warehouse-1".to_string(),
    Point::new(-122.4, 37.8),
);

// Start workflow instance
let instance_id = engine.start_workflow(
    workflow.id(),
    Box::new(factory),
)?;
```

### Processing Workflow Steps

```rust
// In simulation loop
loop {
    // Check for completed tasks
    for task_id in completed_task_ids {
        // Notify engine of completion
        engine.complete_task(task_id)?;
    }

    // Get next tasks to process
    let pending_tasks = engine.get_pending_tasks();

    // Assign to resources
    for task in pending_tasks {
        resource_allocator.assign_task(task)?;
    }
}
```

## Testing Strategies

### Workflow Definition Tests

```rust
#[test]
fn test_shipment_workflow_creation() {
    let warehouse_id = "warehouse-1".to_string();
    let destination = Point::new(-122.4, 37.8);

    let workflow = PackageShipmentWorkflow::new(
        "PKG-123".to_string(),
        warehouse_id.clone(),
        destination,
        2.5,
    );

    assert_eq!(workflow.workflow_type(), "package_shipment");
    assert_eq!(workflow.tracking_number(), "PKG-123");
    assert_eq!(workflow.warehouse_id(), &warehouse_id);
    assert_eq!(workflow.weight_kg(), 2.5);
    assert_eq!(workflow.destination(), destination);
}

#[test]
fn test_workflow_steps_in_correct_order() {
    let workflow = PackageShipmentWorkflow::new(
        "PKG-456".to_string(),
        "w1".to_string(),
        Point::new(0.0, 0.0),
        1.0,
    );

    let steps = workflow.steps();
    assert_eq!(steps.len(), 5);

    // Verify step order
    assert_eq!(steps[0].step_index, 0);
    assert_eq!(steps[0].metadata.get("name").unwrap(), "PackageReceived");
    assert_eq!(steps[0].required_resource_type, "package_receiver");

    assert_eq!(steps[1].step_index, 1);
    assert_eq!(steps[1].metadata.get("name").unwrap(), "Sorting");
    assert_eq!(steps[1].required_resource_type, "sorting_station");

    assert_eq!(steps[2].step_index, 2);
    assert_eq!(steps[2].metadata.get("name").unwrap(), "Loading");
    assert_eq!(steps[2].required_resource_type, "loading_dock");
}

#[test]
fn test_total_workflow_duration() {
    let workflow = PackageShipmentWorkflow::new(
        "PKG-TIMING".to_string(),
        "w1".to_string(),
        Point::new(0.0, 0.0),
        1.0,
    );

    // Calculate total expected duration
    let total_seconds: i64 = workflow
        .steps()
        .iter()
        .map(|step| step.expected_duration.num_seconds())
        .sum();

    // 0 + 300 + 360 + 1800 + 60 = 2520 seconds (42 minutes)
    assert_eq!(total_seconds, 2520);
}
```

### Task Factory Tests

```rust
#[test]
fn test_task_factory_generates_correct_task_types() {
    let instance_id = WorkflowInstanceId::new();
    let destination = Point::new(1.0, 2.0);

    let factory = ShipmentTaskFactory::new(
        "PKG-789".to_string(),
        3.5,
        "warehouse-2".to_string(),
        destination,
    );

    let workflow = PackageShipmentWorkflow::new(
        "PKG-789".to_string(),
        "warehouse-2".to_string(),
        destination,
        3.5,
    );

    let steps = workflow.steps();
    let current_time = Utc::now();

    // Test each step's task generation
    let task0 = factory.create_task(instance_id, &steps[0], current_time);
    assert_eq!(task0.task_type, "receive_package");
    assert_eq!(task0.required_resource_type(), "package_receiver");

    let task1 = factory.create_task(instance_id, &steps[1], current_time);
    assert_eq!(task1.task_type, "sort_package");
    assert_eq!(task1.required_resource_type(), "sorting_station");

    let task2 = factory.create_task(instance_id, &steps[2], current_time);
    assert_eq!(task2.task_type, "load_package");
    assert_eq!(task2.required_resource_type(), "loading_dock");
}

#[test]
fn test_task_has_workflow_instance_id() {
    let instance_id = WorkflowInstanceId::new();
    let destination = Point::new(1.0, 1.0);

    let factory = ShipmentTaskFactory::new(
        "PKG-TEST".to_string(),
        2.0,
        "w1".to_string(),
        destination,
    );

    let step = WorkflowStep {
        step_index: 1,
        required_resource_type: "sorting_station".to_string(),
        expected_duration: Duration::seconds(300),
        metadata: HashMap::new(),
    };

    let task = factory.create_task(instance_id, &step, Utc::now());

    assert_eq!(task.workflow_instance_id(), Some(instance_id));
    assert_eq!(task.tracking_number, "PKG-TEST");
    assert_eq!(task.weight_kg, 2.0);
}

#[test]
fn test_task_factory_creates_unique_task_ids() {
    let destination = Point::new(0.0, 0.0);
    let factory = ShipmentTaskFactory::new(
        "PKG-UNIQUE".to_string(),
        1.0,
        "w1".to_string(),
        destination,
    );

    let step = WorkflowStep {
        step_index: 0,
        required_resource_type: "package_receiver".to_string(),
        expected_duration: Duration::seconds(0),
        metadata: HashMap::new(),
    };

    let instance_id = WorkflowInstanceId::new();
    let current_time = Utc::now();

    let task1 = factory.create_task(instance_id, &step, current_time);
    let task2 = factory.create_task(instance_id, &step, current_time);

    assert_ne!(task1.id(), task2.id());
}
```

### Workflow Progression Tests

```rust
#[test]
fn test_workflow_progression() {
    let workflow = PackageShipmentWorkflow::new(
        "PKG-111".to_string(),
        "w1".to_string(),
        Point::new(0.0, 0.0),
        1.5,
    );

    assert_eq!(workflow.step_count(), 5);

    // Verify we can get each step
    assert!(workflow.get_step(0).is_some());
    assert!(workflow.get_step(1).is_some());
    assert!(workflow.get_step(2).is_some());
    assert!(workflow.get_step(3).is_some());
    assert!(workflow.get_step(4).is_some());
    assert!(workflow.get_step(5).is_none());

    // Verify step durations
    let steps = workflow.steps();
    assert_eq!(steps[0].expected_duration, Duration::seconds(0));
    assert_eq!(steps[1].expected_duration, Duration::seconds(300));
    assert_eq!(steps[2].expected_duration, Duration::seconds(360));
    assert_eq!(steps[3].expected_duration, Duration::seconds(1800));
    assert_eq!(steps[4].expected_duration, Duration::seconds(60));
}

#[test]
fn test_multiple_instances_running_concurrently() {
    let dest1 = Point::new(1.0, 1.0);
    let dest2 = Point::new(2.0, 2.0);
    let dest3 = Point::new(3.0, 3.0);

    // Create multiple workflow instances
    let workflow1 = PackageShipmentWorkflow::new(
        "PKG-001".to_string(),
        "w1".to_string(),
        dest1,
        1.0,
    );

    let workflow2 = PackageShipmentWorkflow::new(
        "PKG-002".to_string(),
        "w1".to_string(),
        dest2,
        2.0,
    );

    let workflow3 = PackageShipmentWorkflow::new(
        "PKG-003".to_string(),
        "w2".to_string(),
        dest3,
        3.0,
    );

    // Each should have unique workflow IDs
    assert_ne!(workflow1.id(), workflow2.id());
    assert_ne!(workflow1.id(), workflow3.id());
    assert_ne!(workflow2.id(), workflow3.id());

    // But same structure
    assert_eq!(workflow1.step_count(), 5);
    assert_eq!(workflow2.step_count(), 5);
    assert_eq!(workflow3.step_count(), 5);

    // And same workflow type
    assert_eq!(workflow1.workflow_type(), "package_shipment");
    assert_eq!(workflow2.workflow_type(), "package_shipment");
    assert_eq!(workflow3.workflow_type(), "package_shipment");
}
```

## Example: PackageShipmentWorkflow

Full implementation from `/crates/sim-logistics/src/workflows/shipment.rs`:

### Workflow Structure

```rust
#[derive(Debug, Clone)]
pub struct PackageShipmentWorkflow {
    pub workflow_id: WorkflowId,
    pub tracking_number: String,
    pub warehouse_id: String,
    pub destination: Point<f64>,
    pub weight_kg: f64,
    steps: Vec<WorkflowStep>,
}
```

### Step Definitions

```
Step 0: PackageReceived (0s)     - Instant receipt
Step 1: Sorting (300s / 5min)    - Sort and route
Step 2: Loading (360s / 6min)    - Load onto vehicle
Step 3: InTransit (1800s / 30min)- Deliver to destination
Step 4: Delivered (60s / 1min)   - Confirm delivery

Total: 2520 seconds (42 minutes)
```

### Resource Mapping

```
"package_receiver"     → Warehouse receipt system
"sorting_station"      → SortingStation resource
"loading_dock"         → LoadingDock resource
"driver"               → Driver agent
"delivery_confirmer"   → Confirmation system
```

## Best Practices

1. **Step Ordering**: Use sequential indices (0, 1, 2...)
2. **Resource Matching**: Ensure resource types exist
3. **Duration Accuracy**: Base on real-world timings
4. **Metadata Richness**: Add names and descriptions
5. **Unique IDs**: Each workflow instance gets unique ID
6. **Task Factory**: Implement for step-to-task conversion
7. **Testing**: Test definition, factory, and progression
8. **Documentation**: Document step semantics

## Common Patterns

### Linear Workflow

```rust
// Step 0 → Step 1 → Step 2 → Step 3 → Complete
let steps = vec![
    WorkflowStep { step_index: 0, ... },
    WorkflowStep { step_index: 1, ... },
    WorkflowStep { step_index: 2, ... },
    WorkflowStep { step_index: 3, ... },
];
```

### Parallel Steps (via multiple workflows)

```rust
// Create separate workflows for parallel paths
let workflow_a = PathAWorkflow::new(...);
let workflow_b = PathBWorkflow::new(...);
// Both run concurrently
```

### Conditional Steps (via metadata)

```rust
let step = WorkflowStep {
    step_index: 2,
    required_resource_type: "processor".to_string(),
    expected_duration: Duration::seconds(300),
    metadata: {
        let mut m = HashMap::new();
        m.insert("condition".to_string(), "if_priority_high".to_string());
        m.insert("skip_if".to_string(), "priority_low".to_string());
        m
    },
};
```

## Troubleshooting

### "Workflow never progresses past first step"

- Ensure WorkflowEngine.complete_task() is called
- Verify task has workflow_instance_id set
- Check that Task::workflow_instance_id() returns Some()

### "Resource type mismatch"

- Verify WorkflowStep.required_resource_type matches Resource.resource_type()
- Check for typos in resource type strings
- Ensure resource is registered in ResourceAllocator

### "Task factory creates wrong tasks"

- Verify step_index mapping in create_task()
- Check that factory data is correctly forwarded
- Test factory with all step indices

## Next Steps

- Read [Testing Strategies](./testing-strategies.md) for comprehensive testing
- Read [Performance Tuning](./performance-tuning.md) for optimization
- Read [Custom Resources Guide](./custom-resources.md) for resource integration
