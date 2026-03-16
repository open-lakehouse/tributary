# Supply Chain Workflow Documentation

## Overview

The Supply Chain Workflow manages the complete process of ordering inventory from distribution sites and delivering it to kitchens. This workflow is designed to run concurrently with food order and courier delivery workflows as part of Phase 9.

## Workflow Diagram

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                        Supply Chain Workflow                            │
│                          (50 minute total)                              │
└─────────────────────────────────────────────────────────────────────────┘

        ┌──────────────┐
        │ OrderPlaced  │  Step 0: Instant (0s)
        │ (order_      │  Resource: order_processor
        │  processor)  │  Task: place_order
        └──────┬───────┘
               │
               ▼
        ┌──────────────┐
        │ Driver       │  Step 1: 5 minutes (300s)
        │ Assigned     │  Resource: delivery_driver
        │ (delivery_   │  Task: assign_driver
        │  driver)     │
        └──────┬───────┘
               │
               ▼
        ┌──────────────┐
        │ PickingUp    │  Step 2: 10 minutes (600s)
        │ (delivery_   │  Resource: delivery_driver
        │  driver)     │  Task: pickup_inventory
        │              │  • Driver travels to distribution site
        │              │  • Loads inventory items
        └──────┬───────┘
               │
               ▼
        ┌──────────────┐
        │ InTransit    │  Step 3: 30 minutes (1800s)
        │ (delivery_   │  Resource: delivery_driver
        │  driver)     │  Task: transport_inventory
        │              │  • Driver transports to kitchen
        └──────┬───────┘
               │
               ▼
        ┌──────────────┐
        │ Delivered    │  Step 4: 5 minutes (300s)
        │ (receiving_  │  Resource: receiving_station
        │  station)    │  Task: deliver_inventory
        │              │  • Unload inventory
        │              │  • Verify quantities
        │              │  • Update kitchen inventory
        └──────────────┘
```

## Workflow Metadata

Each workflow instance carries the following metadata:

| Field | Type | Description |
|-------|------|-------------|
| `workflow_id` | `WorkflowId` | Unique identifier for workflow definition |
| `item_type` | `String` | Type of inventory (e.g., "flour", "vegetables") |
| `quantity` | `u32` | Amount to order (units) |
| `source_site_id` | `Uuid` | Distribution site UUID |
| `destination_kitchen_id` | `Uuid` | Target kitchen UUID |

## Task Types

The TaskFactory generates the following task types based on workflow steps:

1. **place_order** (Step 0)
   - Instantaneous order placement
   - Validates inventory levels
   - Creates delivery request

2. **assign_driver** (Step 1)
   - Finds available delivery driver
   - Assigns to pickup task
   - Duration: 5 minutes

3. **pickup_inventory** (Step 2)
   - Driver travels to distribution site
   - Loads inventory items
   - Duration: 10 minutes

4. **transport_inventory** (Step 3)
   - Driver transports to destination
   - Longest step in workflow
   - Duration: 30 minutes

5. **deliver_inventory** (Step 4)
   - Unload at kitchen
   - Verify quantities
   - Trigger inventory update
   - Duration: 5 minutes

## Resource Requirements

The workflow requires three resource types:

### 1. order_processor
- Used in: Step 0 (OrderPlaced)
- Validates inventory needs
- Creates supply orders

### 2. delivery_driver
- Used in: Steps 1-3 (DriverAssigned, PickingUp, InTransit)
- Handles pickup and transportation
- Capacity: Concurrent deliveries

### 3. receiving_station
- Used in: Step 4 (Delivered)
- Verifies deliveries
- Updates inventory system

## Implementation Details

### SupplyChainWorkflow Struct

```rust
pub struct SupplyChainWorkflow {
    pub workflow_id: WorkflowId,
    pub item_type: String,
    pub quantity: u32,
    pub source_site_id: Uuid,
    pub destination_kitchen_id: Uuid,
    steps: Vec<WorkflowStep>,
}
```

### SupplyDeliveryTask Struct

```rust
pub struct SupplyDeliveryTask {
    pub id: TaskId,
    pub task_type: String,
    pub item_type: String,
    pub quantity: u32,
    pub source_site_id: Uuid,
    pub destination_kitchen_id: Uuid,
    pub required_resource_type: String,
    pub estimated_duration: Duration,
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}
```

### SupplyChainTaskFactory

Implements `TaskFactory<SupplyDeliveryTask>` trait:
- Converts workflow steps into concrete tasks
- Preserves metadata across tasks
- Links tasks to workflow instances

## Trait Implementations

### Workflow Trait

```rust
impl Workflow for SupplyChainWorkflow {
    fn id(&self) -> WorkflowId { ... }
    fn workflow_type(&self) -> &str { "supply_chain" }
    fn steps(&self) -> &[WorkflowStep] { ... }
}
```

### Task Trait

```rust
impl Task for SupplyDeliveryTask {
    fn id(&self) -> TaskId { ... }
    fn required_resource_type(&self) -> &str { ... }
    fn estimated_duration(&self) -> Duration { ... }
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> { ... }
}
```

### TaskFactory Trait

```rust
impl TaskFactory<SupplyDeliveryTask> for SupplyChainTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>,
    ) -> SupplyDeliveryTask { ... }
}
```

## Usage Example

```rust
use sim_kitchen::workflows::{SupplyChainWorkflow, SupplyChainTaskFactory};
use sim_core::{Workflow, WorkflowInstance};

// Create workflow definition
let workflow = SupplyChainWorkflow::new(
    "flour".to_string(),
    100,
    distribution_site_id,
    kitchen_id,
);

// Create workflow instance
let instance = WorkflowInstance::new(
    workflow.id(),
    workflow.step_count(),
);

// Create task factory
let factory = SupplyChainTaskFactory::new(
    "flour".to_string(),
    100,
    distribution_site_id,
    kitchen_id,
);

// Generate task for current step
let task = factory.create_task(
    instance.id,
    workflow.get_step(instance.current_step).unwrap(),
    current_time,
);
```

## Test Coverage

The implementation includes 8 comprehensive tests:

1. ✓ `test_supply_chain_workflow_creation` - Workflow instantiation
2. ✓ `test_workflow_steps_in_correct_order` - Step sequence validation
3. ✓ `test_task_factory_generates_correct_task_types` - Task generation
4. ✓ `test_workflow_progression` - Step advancement logic
5. ✓ `test_multiple_instances_running_concurrently` - Concurrent workflows
6. ✓ `test_completion_triggers_inventory_update` - Completion handling
7. ✓ `test_task_has_workflow_instance_id` - Instance tracking
8. ✓ `test_task_factory_creates_unique_task_ids` - ID uniqueness

All tests pass and verify correct workflow behavior.

## Integration Points

### Phase 9 Coordination

This workflow integrates with:
- **Agent C**: Courier delivery workflow (parallel)
- **Agent D**: Supply driver agent implementation
- **Agent E**: Distribution site locations
- **Agent F**: Food order workflow (parallel)

### Inventory Management

Upon completion (Step 4 - Delivered):
- Kitchen inventory levels updated
- Reorder triggers reset
- Metrics recorded for analytics

## Future Enhancements

Potential improvements for later phases:
- Priority ordering for critical inventory
- Multi-stop delivery routes
- Temperature-controlled transport
- Real-time tracking updates
- Delivery time windows

## Files Modified

1. Created: `crates/sim-kitchen/src/workflows/supply_chain.rs` (580 lines)
2. Created: `crates/sim-kitchen/src/workflows/mod.rs`
3. Modified: `crates/sim-kitchen/src/lib.rs` (added workflows module export)

## Compilation Status

Module compiles independently with zero warnings.
Integration testing pending resolution of concurrent agent work.
