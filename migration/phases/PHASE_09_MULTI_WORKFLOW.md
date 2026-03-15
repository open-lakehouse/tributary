# Phase 9: Multiple Concurrent Workflows & Supply Chain

**Status**: NOT STARTED
**Priority**: 🔴 CRITICAL
**Estimated Effort**: 2 weeks
**Dependencies**: Phase 5 (Workflow Integration) must be complete
**Blocks**: Supply chain simulation demo

---

## Objectives

1. Support multiple workflow types in same simulation
2. Implement supply chain workflows for ghost kitchen
3. Add inventory management to kitchens
4. Create distribution site locations and supply driver agents
5. Run 3 concurrent workflows: food ordering + delivery + supply chain

---

## Three Workflows to Implement

### 1. Customer Food Ordering Workflow (already exists, enhance)
```
OrderPlaced → AssignedToKitchen → PreparingFood → FoodReady →
PickedUpByCourier → InTransit → Delivered
```

### 2. Courier Delivery Workflow (NEW)
```
AwaitingPickup → TravelingToKitchen → PickingUpOrder →
TravelingToCustomer → DeliveringOrder → ReturningToIdle
```
- Courier agent syncs with workflow state
- Integrates with MovementHandler for journeys

### 3. Kitchen Supply Ordering Workflow (NEW)
```
MonitoringInventory → SupplyOrderPlaced → OrderProcessedAtDistributor →
SupplyInTransit → SupplyReceived → InventoryReplenished
```
- Triggered when kitchen inventory low
- Distribution sites process orders
- Supply drivers deliver to kitchens

---

## Architecture: WorkflowRegistry Pattern

**File**: `crates/sim-core/src/workflows/registry.rs` (new)

```rust
pub struct WorkflowRegistry {
    workflows: HashMap<String, Box<dyn Workflow>>,
    engines: HashMap<String, WorkflowEngine>,
}

impl WorkflowRegistry {
    pub fn register_workflow(&mut self, name: String, workflow: Box<dyn Workflow>) {
        let engine = WorkflowEngine::new(workflow.clone());
        self.workflows.insert(name.clone(), workflow);
        self.engines.insert(name, engine);
    }

    pub fn start_instance(&mut self, workflow_name: &str) -> Result<WorkflowInstanceId> {
        let engine = self.engines.get_mut(workflow_name)
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_name))?;
        engine.start_instance()
    }

    pub fn tick_all(&mut self, current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        let mut all_events = Vec::new();
        for engine in self.engines.values_mut() {
            all_events.extend(engine.tick(current_time)?);
        }
        Ok(all_events)
    }
}
```

---

## New Components

### Inventory Resource
**File**: `crates/sim-kitchen/src/resources/inventory.rs` (new)

```rust
pub struct Inventory {
    pub kitchen_id: ResourceId,
    pub item_quantities: HashMap<String, f64>,  // item → quantity
    pub reorder_points: HashMap<String, f64>,   // item → min threshold
}

impl Inventory {
    pub fn consume(&mut self, item: &str, quantity: f64) -> Result<()> {
        let current = self.item_quantities.get_mut(item)
            .ok_or_else(|| anyhow!("Item not in inventory"))?;

        if *current < quantity {
            return Err(anyhow!("Insufficient inventory"));
        }

        *current -= quantity;
        Ok(())
    }

    pub fn needs_reorder(&self, item: &str) -> bool {
        if let (Some(&qty), Some(&threshold)) =
            (self.item_quantities.get(item), self.reorder_points.get(item))
        {
            qty < threshold
        } else {
            false
        }
    }

    pub fn replenish(&mut self, item: &str, quantity: f64) {
        *self.item_quantities.entry(item.to_string()).or_insert(0.0) += quantity;
    }
}
```

### Distribution Site Location
**File**: `crates/sim-kitchen/src/locations/distribution_site.rs` (new)

```rust
pub struct DistributionSite {
    pub id: LocationId,
    pub position: Point<f64>,
    pub inventory: HashMap<String, f64>,  // Unlimited supply for simulation
    pub pending_orders: Vec<SupplyOrder>,
}

impl Location for DistributionSite {
    fn id(&self) -> LocationId { self.id }
    fn location_type(&self) -> &str { "distribution_site" }
    fn position(&self) -> Point<f64> { self.position }
    // ... other trait methods
}
```

### Supply Driver Agent
**File**: `crates/sim-kitchen/src/agents/supply_driver.rs` (new)

```rust
pub struct SupplyDriver {
    pub id: AgentId,
    pub status: SupplyDriverStatus,
    pub position: Point<f64>,
    pub current_delivery: Option<SupplyOrder>,
}

pub enum SupplyDriverStatus {
    Idle,
    TravelingToDistributor,
    LoadingSupplies,
    DeliveringToKitchen,
}

impl Agent for SupplyDriver {
    // Similar to Courier agent pattern
}
```

---

## Tasks Summary

### Week 1: Infrastructure
1. **Task 9.1**: Create WorkflowRegistry (1 day)
2. **Task 9.2**: Add Inventory resource to kitchens (1 day)
3. **Task 9.3**: Create DistributionSite location type (1 day)
4. **Task 9.4**: Create SupplyDriver agent (1 day)
5. **Task 9.5**: Define supply chain workflow (1 day)

### Week 2: Integration & Testing
6. **Task 9.6**: Integrate Courier workflow with movement (2 days)
7. **Task 9.7**: Connect inventory consumption to cooking workflow (1 day)
8. **Task 9.8**: Implement supply ordering trigger logic (1 day)
9. **Task 9.9**: End-to-end integration test (2 days)

---

## Integration Test Scenario

**File**: `crates/sim-kitchen/tests/multi_workflow_integration.rs`

```rust
#[test]
fn test_three_concurrent_workflows() {
    let mut sim = create_supply_chain_simulation();

    // Initial state:
    // - 10 customers (can place orders)
    // - 3 kitchens (with limited inventory)
    // - 5 couriers (idle)
    // - 2 distribution sites (unlimited supplies)
    // - 3 supply drivers (idle)

    // Run for 24 simulated hours
    sim.run_for_duration(Duration::from_hours(24))?;

    // Verify all three workflows executed:

    // 1. Food ordering workflow
    let food_orders = sim.events().query(
        "SELECT * FROM events WHERE type = 'caspers.order.delivered'"
    )?;
    assert!(food_orders.len() > 0, "Customers should have ordered food");

    // 2. Courier delivery workflow
    let courier_deliveries = sim.events().query(
        "SELECT * FROM events WHERE type = 'caspers.courier.delivered_order'"
    )?;
    assert!(courier_deliveries.len() > 0, "Couriers should have delivered orders");

    // 3. Supply chain workflow
    let supply_deliveries = sim.events().query(
        "SELECT * FROM events WHERE type = 'caspers.supply.delivered'"
    )?;
    assert!(supply_deliveries.len() > 0, "Kitchens should have received supplies");

    // Verify inventory was consumed and replenished
    let final_inventories = get_kitchen_inventories(&sim);
    for inv in final_inventories {
        assert!(inv.total_quantity() > 0, "Kitchens should maintain inventory");
    }
}
```

---

## Event Types to Add

### Supply Chain Events
```rust
pub struct SupplyOrderPlacedPayload {
    pub order_id: String,
    pub kitchen_id: String,
    pub distributor_id: String,
    pub items: HashMap<String, f64>,
}

pub struct SupplyDeliveredPayload {
    pub order_id: String,
    pub kitchen_id: String,
    pub driver_id: String,
    pub items: HashMap<String, f64>,
}

pub struct InventoryLowPayload {
    pub kitchen_id: String,
    pub item: String,
    pub current_quantity: f64,
    pub threshold: f64,
}
```

### Courier Events
```rust
pub struct CourierPickedUpOrderPayload {
    pub courier_id: String,
    pub order_id: String,
    pub kitchen_id: String,
}

pub struct CourierDeliveredOrderPayload {
    pub courier_id: String,
    pub order_id: String,
    pub customer_id: String,
    pub delivery_time_minutes: f64,
}
```

---

## Acceptance Criteria (Phase Complete)

- [ ] **WorkflowRegistry Pattern**: Multiple workflow types registered
- [ ] **Three Workflows Running**: Food orders + courier delivery + supply chain
- [ ] **Inventory System**: Kitchens consume and replenish inventory
- [ ] **Distribution Sites**: Process supply orders
- [ ] **Supply Drivers**: Deliver to kitchens
- [ ] **Integration Test**: All three workflows execute in 24-hour simulation
- [ ] **Events Emitted**: All workflow state changes generate CloudEvents
- [ ] **No Deadlocks**: Simulation runs without blocking
- [ ] **Tests Pass**: 20+ new tests, all passing

---

## Visualization

After completing this phase, a 24-hour simulation should show:

**Customers**:
- Place ~100 food orders
- Receive deliveries within 30-60 minutes

**Kitchens**:
- Prepare food (inventory consumed)
- Order supplies when low (3-5 supply orders)
- Receive supply deliveries

**Couriers**:
- Pick up prepared orders
- Deliver to customers
- Return to idle

**Supply Drivers**:
- Pick up from distributors
- Deliver to kitchens
- Return to idle

**Metrics**:
- Average food delivery time: 45 minutes
- Average supply delivery time: 2 hours
- Kitchen inventory never reaches zero (successful replenishment)

---

**Estimated Effort**: 2 weeks
**Depends On**: Phase 5 (workflow integration)
**Unblocks**: Supply chain demonstration, realistic multi-agent simulation
