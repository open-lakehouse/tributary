# Workflows

**Last Updated:** March 16, 2026

## Overview

Workflows orchestrate multi-step business processes in Tributary simulations. They define sequences of tasks that must be processed by different resource types, with automatic state management and progression.

## Objective

This document explains:
- The Workflow trait and WorkflowEngine
- Multi-step process orchestration
- State persistence and advancement
- WorkflowRegistry for concurrent workflows
- Integration with resource allocation

## Prerequisites

- Understanding of [Resources](resources.md)
- Knowledge of [Agents](agents.md)
- [Getting Started](../GETTING_STARTED.md) completed

## The Workflow Trait

```rust
pub trait Workflow: Send + Sync + Debug {
    fn id(&self) -> WorkflowId;
    fn workflow_type(&self) -> &str;
    fn steps(&self) -> &[WorkflowStep];
}

pub struct WorkflowStep {
    pub step_index: usize,
    pub required_resource_type: String,
    pub expected_duration: Duration,
    pub metadata: HashMap<String, String>,
}
```

## WorkflowEngine

The WorkflowEngine manages workflow instances:

```rust
pub struct WorkflowEngine {
    instances: RecordBatch,  // Arrow storage
    instance_to_row: HashMap<WorkflowInstanceId, usize>,
}

impl WorkflowEngine {
    pub fn start_instance(&mut self, workflow: Box<dyn Workflow>) -> WorkflowInstanceId;
    pub fn advance_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()>;
    pub fn complete_instance(&mut self, instance_id: WorkflowInstanceId) -> Result<()>;
}
```

## Implementation Example

### Food Order Workflow

```rust
use sim_core::{Workflow, WorkflowStep, WorkflowId};
use chrono::Duration;
use std::collections::HashMap;

#[derive(Debug)]
pub struct FoodOrderWorkflow {
    id: WorkflowId,
    customer_id: AgentId,
    items: Vec<String>,
}

impl FoodOrderWorkflow {
    pub fn new(customer_id: AgentId, items: Vec<String>) -> Self {
        Self {
            id: WorkflowId::new(),
            customer_id,
            items,
        }
    }

    fn build_steps() -> Vec<WorkflowStep> {
        vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "prep_station".to_string(),
                expected_duration: Duration::minutes(2),
                metadata: HashMap::from([
                    ("step_name".to_string(), "prepare".to_string()),
                ]),
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "stove".to_string(),
                expected_duration: Duration::minutes(5),
                metadata: HashMap::from([
                    ("step_name".to_string(), "cook".to_string()),
                ]),
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "packaging".to_string(),
                expected_duration: Duration::minutes(1),
                metadata: HashMap::from([
                    ("step_name".to_string(), "package".to_string()),
                ]),
            },
        ]
    }
}

impl Workflow for FoodOrderWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "food_order"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &Self::build_steps()
    }
}
```

## WorkflowRegistry (Multi-Workflow)

The WorkflowRegistry manages multiple concurrent workflows:

```rust
pub struct WorkflowRegistry {
    workflows: HashMap<String, Box<dyn WorkflowFactory>>,
    engines: HashMap<String, WorkflowEngine>,
}

impl WorkflowRegistry {
    pub fn register_workflow<F>(&mut self, workflow_type: &str, factory: F)
        where F: WorkflowFactory + 'static;

    pub fn start_workflow(&mut self, workflow_type: &str, params: WorkflowParams) -> Result<WorkflowInstanceId>;

    pub fn advance_all(&mut self, ctx: &SimulationContext) -> Result<()>;
}
```

### Example: Multiple Workflow Types

```rust
// Register workflows
let mut registry = WorkflowRegistry::new();
registry.register_workflow("food_order", FoodOrderFactory);
registry.register_workflow("supply_delivery", SupplyDeliveryFactory);
registry.register_workflow("courier_delivery", CourierDeliveryFactory);

// Start instances
let food_instance = registry.start_workflow("food_order", FoodOrderParams { /* ... */ })?;
let supply_instance = registry.start_workflow("supply_delivery", SupplyParams { /* ... */ })?;

// Advance all workflows each tick
registry.advance_all(&ctx)?;
```

## Step-Based Processing

### Task Creation from Workflow Steps

```rust
pub trait TaskFactory: Send + Sync {
    fn create_task(&self, workflow_instance: &WorkflowInstance, step: &WorkflowStep) -> Box<dyn Task>;
}

pub struct FoodOrderTaskFactory;

impl TaskFactory for FoodOrderTaskFactory {
    fn create_task(&self, instance: &WorkflowInstance, step: &WorkflowStep) -> Box<dyn Task> {
        Box::new(FoodPrepTask {
            id: TaskId::new(),
            workflow_instance_id: instance.id,
            step_index: step.step_index,
            required_resource_type: step.required_resource_type.clone(),
            estimated_duration: step.expected_duration,
        })
    }
}
```

### Automatic Advancement

```rust
impl SimulationHooks for MyHooks {
    fn on_task_completed(&mut self, task_id: TaskId, instance_id: WorkflowInstanceId) -> Result<()> {
        // Advance workflow to next step
        self.workflow_engine.advance_instance(instance_id)?;

        // Check if workflow is complete
        if self.workflow_engine.is_complete(instance_id)? {
            self.workflow_engine.complete_instance(instance_id)?;

            // Notify agent
            if let Some(agent_id) = self.get_agent_for_workflow(instance_id) {
                self.notify_agent_completion(agent_id)?;
            }
        }

        Ok(())
    }
}
```

## Examples from Reference Implementations

### sim-kitchen: Supply Chain Workflow

5-step workflow for ingredient delivery:

```rust
pub struct SupplyDeliveryWorkflow {
    steps: vec![
        WorkflowStep {  // Step 0: Driver to distribution site
            required_resource_type: "supply_driver".to_string(),
            expected_duration: Duration::minutes(15),
        },
        WorkflowStep {  // Step 1: Loading at distribution site
            required_resource_type: "loading_station".to_string(),
            expected_duration: Duration::minutes(10),
        },
        WorkflowStep {  // Step 2: Transit to kitchen
            required_resource_type: "supply_driver".to_string(),
            expected_duration: Duration::minutes(15),
        },
        WorkflowStep {  // Step 3: Unloading at kitchen
            required_resource_type: "receiving_dock".to_string(),
            expected_duration: Duration::minutes(10),
        },
        WorkflowStep {  // Step 4: Return to distribution
            required_resource_type: "supply_driver".to_string(),
            expected_duration: Duration::minutes(15),
        },
    ],
}
```

**File:** `crates/sim-kitchen/src/workflows/supply_delivery.rs`

### sim-logistics: Package Shipment Workflow

5-step workflow for package delivery:

```rust
pub struct PackageShipmentWorkflow {
    steps: vec![
        WorkflowStep {  // Step 0: Sorting
            required_resource_type: "sorting_station".to_string(),
            expected_duration: Duration::minutes(5),
        },
        WorkflowStep {  // Step 1: Loading onto truck
            required_resource_type: "loading_dock".to_string(),
            expected_duration: Duration::minutes(3),
        },
        WorkflowStep {  // Step 2: Transit
            required_resource_type: "driver".to_string(),
            expected_duration: Duration::minutes(30),
        },
        WorkflowStep {  // Step 3: Unloading
            required_resource_type: "loading_dock".to_string(),
            expected_duration: Duration::minutes(2),
        },
        WorkflowStep {  // Step 4: Final mile delivery
            required_resource_type: "driver".to_string(),
            expected_duration: Duration::minutes(10),
        },
    ],
}
```

**File:** `crates/sim-logistics/src/workflows/package_shipment.rs`

## Performance Considerations

### Arrow Storage

Workflow instances stored in RecordBatch:

- **Fast state queries**: SQL-based workflow analytics
- **Efficient updates**: Batch advancement of multiple instances
- **Snapshot support**: Full workflow state persistence

### Multi-Workflow Coordination

Phase 9 achieved **sub-millisecond average tick** with 3 concurrent workflows:

- Food order workflow
- Supply delivery workflow
- Courier delivery workflow

**Comfortably under the 100ms target**

## Common Pitfalls

### Pitfall 1: Step Index Mismatch

```rust
// ❌ Wrong - indices don't match
steps: vec![
    WorkflowStep { step_index: 0, /* ... */ },
    WorkflowStep { step_index: 2, /* ... */ },  // Gap!
    WorkflowStep { step_index: 3, /* ... */ },
]

// ✅ Correct - sequential indices
steps: vec![
    WorkflowStep { step_index: 0, /* ... */ },
    WorkflowStep { step_index: 1, /* ... */ },
    WorkflowStep { step_index: 2, /* ... */ },
]
```

### Pitfall 2: Missing Resource Type

```rust
// ❌ Wrong - resource type doesn't exist
WorkflowStep {
    required_resource_type: "nonexistent".to_string(),  // No matching resource!
    /* ... */
}

// ✅ Correct - matching existing resource
WorkflowStep {
    required_resource_type: "kitchen".to_string(),  // Kitchen resource exists
    /* ... */
}
```

### Pitfall 3: Not Handling Completion

```rust
// ❌ Wrong - workflow never completes
fn on_task_completed(&mut self, task_id: TaskId) -> Result<()> {
    // Advance but never check if done
    self.engine.advance_instance(instance_id)?;
    Ok(())
}

// ✅ Correct - complete when done
fn on_task_completed(&mut self, task_id: TaskId, instance_id: WorkflowInstanceId) -> Result<()> {
    self.engine.advance_instance(instance_id)?;

    if self.engine.is_complete(instance_id)? {
        self.engine.complete_instance(instance_id)?;
        // Notify agents, emit events, etc.
    }

    Ok(())
}
```

## Testing Workflows

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_steps() {
        let workflow = FoodOrderWorkflow::new(AgentId::new(), vec!["burger".to_string()]);

        assert_eq!(workflow.steps().len(), 3);
        assert_eq!(workflow.steps()[0].step_index, 0);
        assert_eq!(workflow.steps()[0].required_resource_type, "prep_station");
    }

    #[test]
    fn test_workflow_advancement() {
        let mut engine = WorkflowEngine::new();
        let workflow = Box::new(FoodOrderWorkflow::new(AgentId::new(), vec![]));

        let instance_id = engine.start_instance(workflow);

        // Get instance
        let instance = engine.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 0);

        // Advance
        engine.advance_instance(instance_id).unwrap();
        let instance = engine.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 1);
    }
}
```

## Related Documentation

- [Resources](resources.md) - Task processing and allocation
- [Agents](agents.md) - Workflow triggering via demands
- [Snapshots](snapshots.md) - Workflow state persistence

## Summary

Workflows orchestrate multi-step processes:

- Define steps with resource requirements and durations
- WorkflowEngine manages instance lifecycle
- WorkflowRegistry enables concurrent workflow types
- Automatic advancement on task completion
- Full Arrow storage integration
- Examples: 5-step supply chains, package delivery

---

**Last Updated:** March 16, 2026
