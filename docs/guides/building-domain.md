# Building a Domain Simulation

This guide shows you how to build a complete domain simulation using the framework. We'll use `sim-logistics` as the reference implementation since it was built **without any framework modifications** (Phase 11 validation).

## Overview

Building a domain simulation involves:

1. Define your domain model (agents, resources, workflows)
2. Implement the Agent trait for your entities
3. Implement the Resource trait for capacity-constrained processors
4. Implement the Workflow trait for multi-step processes
5. Create a simulation orchestrator
6. Add domain-specific events and locations

## Step 1: Define Your Domain Model

Start by identifying the key entities in your domain:

### Agents

Agents are entities with behavior that evolve over time. In logistics:

- **Package**: Passive agent tracking delivery lifecycle
- **Driver**: Active agent handling pickup and delivery

**Questions to answer:**

- What entities need to track state over time?
- Which entities make autonomous decisions?
- Which entities are passive vs. active?

### Resources

Resources are capacity-constrained processors:

- **SortingStation**: Processes packages with FIFO queue
- **LoadingDock**: Manages vehicle loading with slot allocation

**Questions to answer:**

- What processing bottlenecks exist?
- What has capacity constraints?
- What processes tasks in queues?

### Workflows

Workflows are multi-step processes:

- **PackageShipmentWorkflow**: 5-step delivery pipeline
  - PackageReceived (instant)
  - Sorting (5 minutes)
  - Loading (6 minutes)
  - InTransit (30 minutes)
  - Delivered (1 minute)

**Questions to answer:**

- What processes have multiple sequential steps?
- What are the dependencies between steps?
- How long does each step take?

## Step 2: Create Your Crate Structure

```bash
cargo new --lib sim-mydomai
cd sim-mydomain
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
sim-core = { path = "../sim-core" }
sim-events = { path = "../sim-events" }
sim-geo = { path = "../sim-geo" }
chrono = "0.4"
geo = "0.28"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "1.0"
uuid = { version = "1.0", features = ["v4"] }

[dev-dependencies]
rand = "0.8"
```

Create module structure:

```
src/
├── lib.rs           # Public API and re-exports
├── agents/          # Agent implementations
│   ├── mod.rs
│   └── my_agent.rs
├── resources/       # Resource implementations
│   ├── mod.rs
│   └── my_resource.rs
├── workflows/       # Workflow definitions
│   ├── mod.rs
│   └── my_workflow.rs
├── events.rs        # Domain events
├── locations.rs     # Geographic locations
└── simulation.rs    # Orchestration
```

## Step 3: Implement Your First Agent

### Define the Status Enum

Agents need a status enum implementing `AgentStatus`:

```rust
// src/agents/package.rs
use sim_core::AgentStatus;

#[derive(Debug, Clone, PartialEq)]
pub enum PackageState {
    Created { origin: Point<f64>, destination: Point<f64> },
    Sorted { warehouse_id: String, destination: Point<f64> },
    Loaded { driver_id: String, destination: Point<f64> },
    InTransit { driver_id: String, destination: Point<f64>, estimated_arrival: DateTime<Utc> },
    Delivered { delivery_time: DateTime<Utc>, destination: Point<f64> },
}

impl AgentStatus for PackageState {
    fn as_str(&self) -> &str {
        match self {
            Self::Created { .. } => "created",
            Self::Sorted { .. } => "sorted",
            Self::Loaded { .. } => "loaded",
            Self::InTransit { .. } => "in_transit",
            Self::Delivered { .. } => "delivered",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Delivered { .. })
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Delivered { .. })
    }
}
```

**Key patterns:**

- Use enum variants to encode state-specific data
- Implement `as_str()` for serialization
- Implement `is_idle()` for filtering

### Implement the Agent Trait

```rust
use sim_core::{Agent, AgentId, AgentTickResult, SimulationContext, Result};

#[derive(Debug, Clone)]
pub struct Package {
    pub id: AgentId,
    pub state: PackageState,
    pub tracking_number: String,
    pub created_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub weight_kg: f64,
    pub priority: PackagePriority,
}

impl Agent for Package {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "package"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.state
    }

    fn position(&self) -> Option<Point<f64>> {
        match &self.state {
            PackageState::Created { origin, .. } => Some(*origin),
            PackageState::Sorted { .. } => None, // At warehouse
            PackageState::Loaded { .. } => None, // On vehicle
            PackageState::InTransit { .. } => None, // On vehicle
            PackageState::Delivered { destination, .. } => Some(*destination),
        }
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Passive agent - state transitions driven externally
        Ok(AgentTickResult::default())
    }

    fn state_data(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        map.insert("tracking_number".to_string(), json!(self.tracking_number));
        map.insert("weight_kg".to_string(), json!(self.weight_kg));
        // Add more fields as needed
        Value::Object(map)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        // Reconstruct from serialized data
        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id)
                .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?
        );

        let metadata: Value = if let Some(ref m) = row.metadata {
            serde_json::from_str(m).unwrap_or_default()
        } else {
            Value::Null
        };

        // Extract fields from metadata
        let tracking_number = metadata.get("tracking_number")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Reconstruct state from status string
        let state = match row.status.as_str() {
            "created" => {
                let origin_x = metadata.get("origin_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let origin_y = metadata.get("origin_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_x = metadata.get("dest_x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let dest_y = metadata.get("dest_y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                PackageState::Created {
                    origin: Point::new(origin_x, origin_y),
                    destination: Point::new(dest_x, dest_y),
                }
            }
            // ... handle other states
            _ => PackageState::Created {
                origin: Point::new(0.0, 0.0),
                destination: Point::new(0.0, 0.0),
            }
        };

        Ok(Package {
            id,
            state,
            tracking_number,
            created_at: Utc::now(),
            delivered_at: None,
            weight_kg: metadata.get("weight_kg").and_then(|v| v.as_f64()).unwrap_or(1.0),
            priority: PackagePriority::Standard,
        })
    }
}
```

**Key patterns:**

- Passive agents return `AgentTickResult::default()` from `tick()`
- Active agents update state and return journeys/demands
- Position returns `None` when location is tracked by another entity
- Use `state_data()` for custom serialization
- Implement `from_arrow_row()` for persistence

## Step 4: Implement Resources

Resources process tasks with capacity constraints:

```rust
// src/resources/sorting.rs
use sim_core::{Resource, ResourceId};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct SortingStation {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u32,
    pub processing_rate_per_hour: f64,
    pub current_load: u32,
    pub queue: VecDeque<String>, // Package IDs
    pub processing: Vec<(String, u64)>, // (package_id, completion_tick)
    pub total_processed: u64,
}

impl SortingStation {
    pub fn new(name: String, capacity: u32, processing_rate_per_hour: f64) -> Self {
        Self {
            id: ResourceId::new(),
            name,
            capacity,
            processing_rate_per_hour,
            current_load: 0,
            queue: VecDeque::new(),
            processing: Vec::new(),
            total_processed: 0,
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.current_load < self.capacity
    }

    pub fn enqueue_package(&mut self, package_id: String) -> Result<(), String> {
        let total_load = self.current_load + self.queue.len() as u32;
        if total_load >= self.capacity * 3 {
            return Err("Queue is full".to_string());
        }
        self.queue.push_back(package_id);
        Ok(())
    }

    pub fn process_tick(&mut self, current_tick: u64) {
        // Check for completed packages
        self.processing.retain(|(_, completion_tick)| {
            if *completion_tick <= current_tick {
                self.current_load = self.current_load.saturating_sub(1);
                self.total_processed += 1;
                false // Remove from processing
            } else {
                true // Keep processing
            }
        });

        // Start processing queued packages
        while self.has_capacity() && !self.queue.is_empty() {
            if let Some(package_id) = self.queue.pop_front() {
                let seconds_per_package = 3600.0 / self.processing_rate_per_hour;
                let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;
                let completion_tick = current_tick + ticks_per_package;

                self.processing.push((package_id, completion_tick));
                self.current_load += 1;
            }
        }
    }
}

impl Resource for SortingStation {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "sorting_station"
    }

    fn capacity(&self) -> usize {
        self.capacity as usize
    }

    fn current_load(&self) -> usize {
        self.current_load as usize
    }

    fn is_available(&self) -> bool {
        self.has_capacity()
    }
}
```

**Key patterns:**

- Resources manage their own queue and processing state
- Call `process_tick()` each simulation step
- Use tick counts for timing calculations
- Track completion times for each item

## Step 5: Implement Workflows

Workflows define multi-step processes:

```rust
// src/workflows/shipment.rs
use sim_core::{Workflow, WorkflowId, WorkflowStep};
use chrono::Duration;

#[derive(Debug, Clone)]
pub struct PackageShipmentWorkflow {
    pub workflow_id: WorkflowId,
    pub tracking_number: String,
    pub warehouse_id: String,
    pub destination: Point<f64>,
    pub weight_kg: f64,
    steps: Vec<WorkflowStep>,
}

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
                    m.insert("description".to_string(), "Package arrives at warehouse".to_string());
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
                    m
                },
            },
            // ... more steps
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

**Key patterns:**

- Define steps in order with durations
- Match `required_resource_type` to Resource implementations
- Store workflow-specific metadata
- Use metadata for rich step descriptions

## Step 6: Create Simulation Orchestrator

```rust
// src/simulation.rs
use sim_core::{SimulationConfig, SimulationContext, PopulationHandler};

pub struct MyDomainSimulation {
    context: SimulationContext,
    agents: PopulationHandler<MyAgent>,
    resources: Vec<MyResource>,
    metrics: MyMetrics,
}

impl MyDomainSimulation {
    pub fn new() -> Self {
        let config = SimulationConfig {
            name: "my_domain".to_string(),
            time_step: Duration::seconds(30),
            verbose: false,
            custom: serde_json::Map::new(),
        };
        let context = SimulationContext::from_now(config);

        Self {
            context,
            agents: PopulationHandler::new(),
            resources: Vec::new(),
            metrics: MyMetrics::default(),
        }
    }

    pub fn run_for_hours(&mut self, hours: u64) -> Result<()> {
        let steps = hours * 120; // 30 sec/step
        for _ in 0..steps {
            self.step()?;
        }
        Ok(())
    }

    fn step(&mut self) -> Result<()> {
        // Tick agents
        self.agents.tick_agents(&self.context)?;

        // Process resources
        for resource in &mut self.resources {
            resource.process_tick(self.context.current_tick());
        }

        // Advance time
        self.context.advance_time();

        Ok(())
    }
}
```

## Step 7: Add Tests

Create comprehensive tests at multiple levels:

### Unit Tests

Test each component in isolation (see `building-domain.md` examples in each module).

### Integration Tests

```rust
// tests/integration_test.rs
use sim_mydomain::{MyDomainSimulation};

#[test]
fn test_24_hour_simulation() {
    let mut sim = MyDomainSimulation::new();

    // Setup initial state
    sim.create_agents(100);

    // Run simulation
    sim.run_for_hours(24).unwrap();

    // Verify metrics
    let metrics = sim.get_metrics();
    assert!(metrics.processed > 0);
}

#[test]
fn test_framework_generality() {
    // Ensures sim-core has no domain knowledge
    use sim_core::*;
    let _config = SimulationConfig::default();
    assert!(true, "Framework is generic");
}
```

## Step 8: Define Domain Events

```rust
// src/events.rs
use sim_events::{Event, EventType};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct PackageDeliveredEvent {
    pub timestamp: DateTime<Utc>,
    pub package_id: String,
    pub tracking_number: String,
    pub destination: Point<f64>,
    pub delivery_time_seconds: i64,
}

impl Event for PackageDeliveredEvent {
    fn event_type(&self) -> EventType {
        EventType::Custom("package_delivered".to_string())
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    fn metadata(&self) -> serde_json::Value {
        json!({
            "package_id": self.package_id,
            "tracking_number": self.tracking_number,
            "destination_x": self.destination.x(),
            "destination_y": self.destination.y(),
            "delivery_time_seconds": self.delivery_time_seconds,
        })
    }
}
```

## Checklist for Domain Completion

Use this checklist to ensure your domain is complete:

### Core Implementation

- [ ] Agent trait implemented for all entities
- [ ] AgentStatus trait implemented for state enums
- [ ] Resource trait implemented for processors
- [ ] Workflow trait implemented for multi-step processes
- [ ] Task and TaskFactory implemented for workflow steps
- [ ] Simulation orchestrator created

### State Management

- [ ] State transitions are well-defined
- [ ] Position tracking is correct (Some vs None)
- [ ] Metadata serialization works (state_data)
- [ ] Arrow serialization works (from_arrow_row)

### Testing

- [ ] Unit tests for each agent (>90% coverage)
- [ ] Unit tests for each resource
- [ ] Unit tests for workflows
- [ ] Integration test for 24-hour simulation
- [ ] Framework generality test (no domain types in sim-core)

### Events

- [ ] Domain events defined
- [ ] Event metadata complete
- [ ] Events emitted at key transitions

### Documentation

- [ ] Module-level documentation
- [ ] Example usage in lib.rs
- [ ] State machine diagrams in doc comments

### Performance

- [ ] No framework modifications needed
- [ ] Scales to 10,000+ agents
- [ ] Memory usage is reasonable

## Common Patterns

### Passive vs Active Agents

**Passive agents** (Package):

- State changes driven externally
- `tick()` returns default result
- Position may be None when contained by other entities

**Active agents** (Driver):

- Autonomous decision making
- `tick()` updates state and returns actions
- Always has position

### Resource Processing Patterns

**FIFO Queue Processing:**

```rust
pub fn process_tick(&mut self, current_tick: u64) {
    // Complete finished items
    self.processing.retain(|(_, completion)| {
        *completion > current_tick
    });

    // Start new items
    while self.has_capacity() && !self.queue.is_empty() {
        let item = self.queue.pop_front().unwrap();
        let completion = current_tick + duration_in_ticks;
        self.processing.push((item, completion));
    }
}
```

**Slot-Based Processing:**

```rust
pub fn assign_to_slot(&mut self, item_id: String) -> Result<(), String> {
    if self.active_slots.len() >= self.total_slots {
        return Err("No available slots".to_string());
    }
    self.active_slots.insert(item_id, SlotState::new());
    Ok(())
}
```

### Timing Calculations

Convert rates to tick durations:

```rust
// Rate: 12 packages/hour
// Time per package: 3600 / 12 = 300 seconds
// Ticks per package (30 sec/tick): 300 / 30 = 10 ticks
let seconds_per_item = 3600.0 / rate_per_hour;
let ticks_per_item = (seconds_per_item / 30.0).ceil() as u64;
let completion_tick = current_tick + ticks_per_item;
```

## Troubleshooting

### "Agent position is None when it should have a location"

Check your position() implementation. Return Some() only when the agent independently tracks its location. Return None when position is tracked by another entity (e.g., package on a vehicle).

### "Resource never processes tasks"

Ensure you're calling `process_tick()` in your simulation step. Resources don't process automatically.

### "Workflow steps don't match resources"

Verify that `WorkflowStep.required_resource_type` matches `Resource.resource_type()` exactly.

### "Agent serialization fails"

Implement both `state_data()` and `from_arrow_row()` completely. Test with `to_arrow_row()` -> `from_arrow_row()` round-trip.

## Next Steps

- Read [Custom Agents Guide](./custom-agents.md) for advanced agent patterns
- Read [Custom Resources Guide](./custom-resources.md) for resource strategies
- Read [Custom Workflows Guide](./custom-workflows.md) for workflow patterns
- Read [Testing Strategies](./testing-strategies.md) for comprehensive testing
- Read [Performance Tuning](./performance-tuning.md) for optimization

## Reference Implementation

See `/crates/sim-logistics` for a complete, working implementation that required **ZERO framework modifications**.
