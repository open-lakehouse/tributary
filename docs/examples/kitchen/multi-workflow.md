# Multi-Workflow Kitchen Example

Demonstrates Phase 9 multi-workflow orchestration with FoodOrderWorkflow, SupplyChainWorkflow, and CourierDeliveryWorkflow running concurrently.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    WorkflowRegistry                              │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐  ┌──────────────────┐  ┌───────────────┐ │
│  │ FoodOrder        │  │ SupplyChain      │  │ CourierDeliv  │ │
│  │ Workflow         │  │ Workflow         │  │ Workflow      │ │
│  │                  │  │                  │  │               │ │
│  │ 1. Prep          │  │ 1. CheckInv      │  │ 1. Pickup     │ │
│  │ 2. Cook          │  │ 2. OrderSupply   │  │ 2. Transit    │ │
│  │ 3. Package       │  │ 3. Receive       │  │ 3. Deliver    │ │
│  └──────────────────┘  └──────────────────┘  └───────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Complete Example

Location: `/Users/robert.pack/code/management/crates/sim-kitchen/tests/multi_workflow_e2e_test.rs`

### 1. Define Workflows

```rust
use sim_core::{Workflow, WorkflowId, WorkflowStep};

// Food Order Workflow (3 steps)
struct FoodOrderWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl Workflow for FoodOrderWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "food_order"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

// Supply Chain Workflow (3 steps)
struct SupplyChainWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl Workflow for SupplyChainWorkflow {
    fn workflow_type(&self) -> &str {
        "supply_chain"
    }
    // ...
}

// Courier Delivery Workflow (3 steps)
struct CourierDeliveryWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl Workflow for CourierDeliveryWorkflow {
    fn workflow_type(&self) -> &str {
        "courier_delivery"
    }
    // ...
}
```

### 2. Setup Workflow Registry

```rust
use sim_core::{WorkflowRegistry, WorkflowEngine};

let mut registry = WorkflowRegistry::new();

// Register food order workflow
let mut food_engine = WorkflowEngine::new();
food_engine.register_workflow(FoodOrderWorkflow::new());
registry.register("food_order".to_string(), food_engine);

// Register supply chain workflow
let mut supply_engine = WorkflowEngine::new();
supply_engine.register_workflow(SupplyChainWorkflow::new());
registry.register("supply_chain".to_string(), supply_engine);

// Register courier delivery workflow
let mut courier_engine = WorkflowEngine::new();
courier_engine.register_workflow(CourierDeliveryWorkflow::new());
registry.register("courier_delivery".to_string(), courier_engine);

// Register with runner
runner.set_workflow_registry(registry);
```

### 3. Task Factories

```rust
// Food order task factory
struct FoodOrderTaskFactory;

impl TaskFactory<FoodOrderTask> for FoodOrderTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>
    ) -> FoodOrderTask {
        FoodOrderTask {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            step_type: step.required_resource_type.clone(),
            duration: step.expected_duration,
        }
    }
}

// Supply chain task factory
struct SupplyChainTaskFactory {
    inventory: Arc<Mutex<InventoryManager>>,
}

impl TaskFactory<SupplyDeliveryTask> for SupplyChainTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>
    ) -> SupplyDeliveryTask {
        let mut inventory = self.inventory.lock().unwrap();

        match step.step_index {
            0 => {
                // Check inventory
                let low_stock_items = inventory.check_low_stock();
                SupplyDeliveryTask {
                    id: TaskId::new(),
                    workflow_instance_id: Some(instance_id),
                    step_type: "inventory_check".to_string(),
                    items: low_stock_items,
                    duration: step.expected_duration,
                }
            }
            1 => {
                // Order supplies
                SupplyDeliveryTask {
                    id: TaskId::new(),
                    workflow_instance_id: Some(instance_id),
                    step_type: "supply_order".to_string(),
                    items: vec!["ingredients".to_string()],
                    duration: step.expected_duration,
                }
            }
            2 => {
                // Receive supplies
                inventory.restock("ingredients".to_string(), 100);
                SupplyDeliveryTask {
                    id: TaskId::new(),
                    workflow_instance_id: Some(instance_id),
                    step_type: "supply_receive".to_string(),
                    items: vec![],
                    duration: step.expected_duration,
                }
            }
            _ => panic!("Invalid step index"),
        }
    }
}
```

### 4. Coordinating Hooks

```rust
struct MultiWorkflowHooks {
    food_task_factory: Box<dyn TaskFactory<FoodOrderTask>>,
    supply_task_factory: Box<dyn TaskFactory<SupplyDeliveryTask>>,
    courier_task_factory: Box<dyn TaskFactory<CourierDeliveryTask>>,
    inventory: Arc<Mutex<InventoryManager>>,
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for MultiWorkflowHooks {
    fn on_workflow_step_completed(
        &mut self,
        ctx: &SimulationContext,
        event: &WorkflowStepEvent
    ) -> Result<()> {
        match event {
            WorkflowStepEvent::StepCompleted { workflow_id, instance_id, step_index, .. } => {
                // Emit events based on workflow type
                let workflow_type = self.get_workflow_type(workflow_id)?;

                match workflow_type.as_str() {
                    "food_order" => {
                        if *step_index == 2 {
                            // Final step - trigger courier delivery
                            self.start_courier_delivery(instance_id, ctx)?;
                        }
                    }
                    "supply_chain" => {
                        if *step_index == 2 {
                            // Inventory replenished
                            self.emit_inventory_updated_event(ctx)?;
                        }
                    }
                    "courier_delivery" => {
                        if *step_index == 2 {
                            // Order delivered
                            self.emit_order_delivered_event(instance_id, ctx)?;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        Ok(())
    }
}
```

### 5. Running the Simulation

```rust
fn main() -> Result<()> {
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);

    // Setup workflow registry
    let registry = setup_workflow_registry();
    runner.set_workflow_registry(registry);

    // Setup inventory
    let inventory = Arc::new(Mutex::new(InventoryManager::new()));

    // Setup hooks
    let hooks = MultiWorkflowHooks {
        food_task_factory: Box::new(FoodOrderTaskFactory),
        supply_task_factory: Box::new(SupplyChainTaskFactory {
            inventory: inventory.clone(),
        }),
        courier_task_factory: Box::new(CourierDeliveryTaskFactory),
        inventory,
        event_dispatcher: EventDispatcher::new(),
    };

    runner.set_hooks(Box::new(hooks));

    // Start workflows
    let food_workflow_id = WorkflowId::new();
    let supply_workflow_id = WorkflowId::new();

    runner.workflow_registry_mut()
        .unwrap()
        .get_engine_mut("food_order")
        .unwrap()
        .start_instance(food_workflow_id)?;

    runner.workflow_registry_mut()
        .unwrap()
        .get_engine_mut("supply_chain")
        .unwrap()
        .start_instance(supply_workflow_id)?;

    // Run
    runner.run_for_steps(200)?;

    println!("Multi-workflow simulation complete");
    println!("Total workflow steps completed: {}", runner.stats().total_workflow_steps);

    Ok(())
}
```

## Key Patterns

1. **WorkflowRegistry**: Central registry for multiple workflow engines
2. **Workflow-specific TaskFactories**: Each workflow type has its own task factory
3. **Workflow Coordination**: Hooks coordinate between workflows (e.g., food order completion triggers courier delivery)
4. **Shared State**: Inventory manager shared between workflows

## Expected Behavior

```
Step 0: Starting food order workflow, supply chain workflow
Step 50: Food prep complete, cooking started
Step 75: Supply chain checks inventory
Step 100: Food order packaged, courier delivery triggered
Step 125: Supply order placed
Step 150: Courier in transit
Step 175: Supplies received, inventory restocked
Step 200: Order delivered

Total workflow steps completed: 9 (3 per workflow)
```

## See Also

- [inventory-management.md](./inventory-management.md) - Detailed inventory system
- [basic.md](./basic.md) - Simple single-workflow example
- `/Users/robert.pack/code/management/crates/sim-kitchen/tests/multi_workflow_e2e_test.rs` - Full runnable test
