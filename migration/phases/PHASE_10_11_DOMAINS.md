# Phase 10-11: Demand Generation & Logistics Domain

**Status**: NOT STARTED
**Priority**: 🟢 VALIDATION
**Estimated Effort**: 2 weeks (1 week each)
**Dependencies**: Phase 5 (workflows) recommended
**Blocks**: Framework validation

---

## Phase 10: Demand Generation Framework (Week 1)

### Objectives
1. Implement PoissonDemandGenerator (exponential inter-arrival times)
2. Implement TimeOfDayDemandGenerator (hourly rate multipliers)
3. Implement SpatialDemandGenerator (H3-based geographic clustering)
4. Integrate with SimulationRunner
5. Apply to ghost kitchen for realistic order patterns

### Key Implementations

**File**: `crates/sim-core/src/demand/models.rs`

```rust
pub struct PoissonDemandGenerator {
    pub rate: f64,  // events per hour
    pub demand_type: String,
}

impl DemandGenerator for PoissonDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        let delta_hours = ctx.step_duration().as_secs_f64() / 3600.0;
        let lambda = self.rate * delta_hours;

        // Poisson: P(k events) = (lambda^k * e^-lambda) / k!
        let k = poisson_sample(lambda);

        (0..k).map(|_| Demand {
            demand_type: self.demand_type.clone(),
            location: None,  // Spatial generator adds this
            metadata: HashMap::new(),
        }).collect()
    }
}

pub struct TimeOfDayDemandGenerator {
    pub base_generator: Box<dyn DemandGenerator>,
    pub hourly_multipliers: [f64; 24],
}

impl DemandGenerator for TimeOfDayDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        let hour = ctx.current_time().hour() as usize;
        let multiplier = self.hourly_multipliers[hour];

        let mut demands = self.base_generator.generate(ctx);

        // Scale demand count by hour multiplier
        let scaled_count = (demands.len() as f64 * multiplier).round() as usize;
        demands.truncate(scaled_count);
        demands
    }
}

pub struct SpatialDemandGenerator {
    pub base_generator: Box<dyn DemandGenerator>,
    pub h3_probabilities: HashMap<H3Cell, f64>,  // cell → demand probability
}

impl DemandGenerator for SpatialDemandGenerator {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand> {
        let mut demands = self.base_generator.generate(ctx);

        // Assign locations based on H3 probabilities
        for demand in &mut demands {
            let cell = sample_from_distribution(&self.h3_probabilities);
            demand.location = Some(h3_cell_center(cell));
        }

        demands
    }
}
```

### Integration with SimulationRunner

**File**: `crates/sim-core/src/runner/mod.rs`

```rust
pub struct SimulationRunner<A, R, T, W> {
    // ... existing fields ...
    demand_generator: Option<Box<dyn DemandGenerator>>,
}

impl SimulationRunner {
    pub fn tick(&mut self) -> Result<()> {
        // ... existing tick logic ...

        // Generate demands
        if let Some(generator) = &self.demand_generator {
            let demands = generator.generate(&self.context);

            // Process demands (domain-specific logic via hooks)
            if let Some(hooks) = &mut self.hooks {
                hooks.on_demands_generated(&demands, &self.context)?;
            }
        }

        Ok(())
    }
}
```

### Acceptance Criteria
- [ ] Three demand generator implementations
- [ ] Integration with SimulationRunner
- [ ] Ghost kitchen uses TimeOfDay + Spatial demand
- [ ] Peak hours (12pm, 6pm) show 3x order rate
- [ ] Spatial clustering observable in event data
- [ ] 10+ unit tests

---

## Phase 11: Logistics Domain (Week 2)

### Objectives
1. Create `crates/domains/logistics` crate
2. Implement logistics domain types (Driver, Package, Warehouse, etc.)
3. Build package shipment workflow
4. Validate framework works for non-kitchen domain
5. Run 24-hour logistics simulation

### Domain Types

#### Agents
```rust
pub struct Driver {
    pub id: AgentId,
    pub status: DriverStatus,
    pub position: Point<f64>,
    pub current_route: Option<JourneyId>,
}

pub enum DriverStatus {
    Idle,
    TravelingToWarehouse,
    LoadingPackages,
    DeliveringPackages,
    ReturningToDepot,
}

// Package as agent (for tracking)
pub struct Package {
    pub id: AgentId,
    pub status: PackageStatus,
    pub origin: LocationId,
    pub destination: LocationId,
}
```

#### Locations
```rust
pub struct Warehouse {
    pub id: LocationId,
    pub position: Point<f64>,
    pub capacity: usize,
    pub current_packages: usize,
}

pub struct DeliveryAddress {
    pub id: LocationId,
    pub position: Point<f64>,
}
```

#### Resources
```rust
pub struct SortingStation {
    pub id: ResourceId,
    pub capacity: usize,
    pub current_load: usize,
}

pub struct LoadingDock {
    pub id: ResourceId,
    pub capacity: usize,  // packages per hour
}
```

#### Workflows
```rust
pub struct PackageShipmentWorkflow;

impl Workflow for PackageShipmentWorkflow {
    fn steps(&self) -> &[WorkflowStep] {
        &[
            WorkflowStep {
                step_index: 0,
                required_resource_type: "sorting_station".into(),
                expected_duration: Duration::from_secs(300),  // 5 min
                metadata: hashmap! { "step" => "sorting" },
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "loading_dock".into(),
                expected_duration: Duration::from_secs(600),  // 10 min
                metadata: hashmap! { "step" => "loading" },
            },
            // Step 2: In-transit (handled by driver agent + journey)
            // Step 3: Delivered (workflow completes)
        ]
    }
}
```

### Events

```rust
pub struct PackageReceivedPayload {
    pub package_id: String,
    pub warehouse_id: String,
    pub destination_address: String,
}

pub struct PackageSortedPayload {
    pub package_id: String,
    pub sorting_station_id: String,
    pub route_assigned: String,
}

pub struct PackageLoadedPayload {
    pub package_id: String,
    pub driver_id: String,
    pub vehicle_id: String,
}

pub struct PackageInTransitPayload {
    pub package_id: String,
    pub driver_id: String,
    pub estimated_arrival: DateTime<Utc>,
}

pub struct PackageDeliveredPayload {
    pub package_id: String,
    pub driver_id: String,
    pub delivery_address: String,
    pub delivery_time: DateTime<Utc>,
}
```

### Crate Structure

```
crates/domains/logistics/
├── src/
│   ├── lib.rs
│   ├── agents/
│   │   ├── mod.rs
│   │   ├── driver.rs
│   │   └── package.rs
│   ├── locations/
│   │   ├── mod.rs
│   │   ├── warehouse.rs
│   │   └── delivery_address.rs
│   ├── resources/
│   │   ├── mod.rs
│   │   ├── sorting_station.rs
│   │   └── loading_dock.rs
│   ├── workflows/
│   │   ├── mod.rs
│   │   └── shipment.rs
│   ├── events/
│   │   ├── mod.rs
│   │   └── package_events.rs
│   └── simulation.rs
├── tests/
│   └── integration_test.rs
└── Cargo.toml
```

### Simulation Setup

```rust
pub fn create_logistics_simulation() -> LogisticsSimulation {
    let mut sim = SimulationRunner::builder()
        .context(create_context())
        .build()
        .unwrap();

    // Add warehouses
    sim.add_locations(vec![
        Warehouse { id: warehouse_1, position: Point::new(-122.4, 37.8), ... },
        Warehouse { id: warehouse_2, position: Point::new(-122.3, 37.7), ... },
    ]);

    // Add delivery addresses (1000 locations)
    sim.add_locations(create_delivery_addresses(1000));

    // Add resources
    sim.add_resources(vec![
        SortingStation { id: sort_1, capacity: 10, ... },
        SortingStation { id: sort_2, capacity: 10, ... },
        LoadingDock { id: dock_1, capacity: 5, ... },
    ]);

    // Add drivers
    sim.add_agents(vec![
        Driver { id: driver_1, status: Idle, ... },
        Driver { id: driver_2, status: Idle, ... },
        // ... 20 drivers total
    ]);

    // Register workflow
    sim.register_workflow("package_shipment", PackageShipmentWorkflow);

    // Add demand generator (packages arriving at warehouses)
    let demand = PoissonDemandGenerator {
        rate: 100.0,  // 100 packages per hour
        demand_type: "package_arrival".into(),
    };
    sim.set_demand_generator(Box::new(demand));

    sim
}
```

### Validation Tests

**File**: `crates/domains/logistics/tests/integration_test.rs`

```rust
#[test]
fn test_logistics_simulation_24_hours() {
    let mut sim = create_logistics_simulation();

    sim.run_for_duration(Duration::from_hours(24)).unwrap();

    // Verify packages processed
    let delivered = sim.events().query(
        "SELECT * FROM events WHERE type = 'logistics.package.delivered'"
    ).unwrap();

    assert!(delivered.len() > 100, "Should deliver 100+ packages in 24 hours");

    // Verify workflows completed
    let sorted = sim.events().query(
        "SELECT * FROM events WHERE type = 'logistics.package.sorted'"
    ).unwrap();

    assert_eq!(sorted.len(), delivered.len(), "All delivered packages should be sorted");

    // Verify drivers utilized
    let driver_trips = sim.events().query(
        "SELECT DISTINCT driver_id FROM events WHERE type = 'logistics.package.delivered'"
    ).unwrap();

    assert!(driver_trips.len() >= 10, "At least 10 drivers should be used");
}

#[test]
fn test_framework_abstraction_no_domain_coupling() {
    // This test ensures sim-core has ZERO knowledge of logistics domain

    // Should compile without logistics types
    use sim_core::*;

    // Framework should work with generic types only
    let _runner: SimulationRunner<
        Box<dyn Agent>,
        Box<dyn Resource>,
        Box<dyn Task>,
        Box<dyn Workflow>
    > = SimulationRunner::builder().build().unwrap();

    // This proves framework is truly generic
}
```

### Acceptance Criteria

**Phase 10 (Demand Generation)**:
- [ ] Three demand generator implementations
- [ ] Poisson, TimeOfDay, Spatial models working
- [ ] Ghost kitchen uses realistic demand patterns
- [ ] Peak hours observable in simulation data
- [ ] 10+ unit tests

**Phase 11 (Logistics Domain)**:
- [ ] Logistics domain crate created
- [ ] All domain types implemented (agents, locations, resources, workflows)
- [ ] Package shipment workflow functional
- [ ] 24-hour simulation runs successfully
- [ ] 100+ packages delivered
- [ ] Events emitted for all workflow steps
- [ ] Framework requires ZERO changes (proves generality)
- [ ] 15+ tests in logistics crate

---

## Success Criteria (Both Phases)

- [ ] **Demand Generators Work**: Order patterns realistic
- [ ] **Logistics Domain Complete**: Second domain validates framework
- [ ] **Framework Unchanged**: Zero modifications to sim-core, sim-geo, sim-events
- [ ] **Tests Pass**: 25+ new tests, all passing
- [ ] **Documentation**: Both domains documented as examples

---

## Parallel Execution

**Week 1**: Phase 10 (Demand Generation) - Agent A
**Week 2**: Phase 11 (Logistics Domain) - Agent B

Minimal dependencies, can be done sequentially or by different people.

---

**Estimated Effort**: 2 weeks
**Depends On**: Phase 5 (helpful but not required)
**Validates**: Framework generality, abstractions sufficiency
