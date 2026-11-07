# sim-kitchen

Ghost kitchen simulation demonstrating the sim-core framework.

## Overview

This crate is a reference implementation showing how to build domain-specific simulations using sim-core, sim-geo, and sim-events. It models a ghost kitchen business where customers order food, kitchens prepare orders, and the system emits CloudEvents for all state changes.

## Domain Model

### Agents

**Customer**:
- States: Idle → Ordering → WaitingForFood → Eating → Done
- Behavior: Probabilistically decides to order food
- Outputs: Demands (food orders)
- Configurable: Order probability, eating duration

### Resources

**Kitchen**:
- Type: Capacity-constrained processor
- Function: Prepares food orders
- Tracking: Current load vs total capacity

### Tasks

**Order**:
- Represents: Food order to be prepared
- Properties: Items list, estimated prep time
- Priority: Based on item count

### Events

Five CloudEvents 1.0 compliant event types:
- `io.kitchen.order.created` - Customer places order
- `io.kitchen.order.assigned` - Order assigned to kitchen
- `io.kitchen.order.completed` - Order ready
- `io.kitchen.customer.eating` - Customer starts eating
- `io.kitchen.customer.done` - Customer finishes

## Quick Start

### Run the Example

```bash
cargo run -p sim-kitchen --example basic
```

### Use as Library

```rust
use sim_kitchen::{build_simulation, add_customers};

// Create simulation with 3 kitchens, 5 capacity each
let mut runner = build_simulation(3, 5, true);

// Add 20 customers
add_customers(&mut runner, 20);

// Run for 100 steps (50 minutes with 30s steps)
runner.run_for_steps(100)?;

println!("Simulation complete!");
```

## Example Output

```
🍔 Ghost Kitchen Simulation Example

🏁 Ghost Kitchen Simulation Starting
   Time: 2026-03-15 15:32:20 UTC
   Time step: 30 seconds

📝 [Step 1] Order created: order-019cf220-...
📝 [Step 2] Order created: order-019cf220-...
...

🏁 Simulation Complete
   Total steps: 100
   Elapsed time: 50 minutes
   Total events: 20

📊 Event Summary:
   io.kitchen.order.created: 20
```

## Building Your Own Simulation

This example demonstrates the key patterns:

### 1. Define Your Agent

```rust
use sim_core::{Agent, AgentStatus, AgentTickResult, SimulationContext};

#[derive(Debug)]
pub struct MyAgent {
    id: AgentId,
    status: MyStatus,
}

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Implement behavior
        // Return demands, journeys, state changes
    }
}
```

### 2. Define Your Resource

```rust
use sim_core::{Resource, ResourceId};

#[derive(Debug)]
pub struct MyResource {
    id: ResourceId,
    capacity: usize,
}

impl Resource for MyResource {
    fn resource_type(&self) -> &str { "my_type" }
    fn capacity(&self) -> usize { self.capacity }
    fn current_load(&self) -> usize { /* track load */ }
}
```

### 3. Define Your Task

```rust
use sim_core::{Task, TaskId};
use chrono::Duration;

#[derive(Debug)]
pub struct MyTask {
    id: TaskId,
}

impl Task for MyTask {
    fn required_resource_type(&self) -> &str { "my_type" }
    fn estimated_duration(&self) -> Duration { /* ... */ }
}
```

### 4. Define Event Payloads

```rust
use sim_events::EventPayload;
use serde_json::json;

#[derive(Debug)]
pub struct MyEvent {
    pub entity_id: String,
}

impl EventPayload for MyEvent {
    fn event_type(&self) -> &str {
        "com.example.my.event"
    }

    fn source(&self) -> String {
        format!("/entities/{}", self.entity_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({ "entity_id": self.entity_id })
    }
}
```

### 5. Create Simulation Hooks

```rust
use sim_core::SimulationHooks;
use sim_events::{Event, EventDispatcher};

pub struct MyHooks {
    pub dispatcher: EventDispatcher,
}

impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        // Process demands, emit events
        for result in results {
            for demand in &result.demands {
                let event = Event::new(Box::new(MyEvent { /* ... */ }), ctx.current_time());
                self.dispatcher.emit(event);
            }
        }
        Ok(())
    }

    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        self.dispatcher.flush_to_parquet("events.parquet")?;
        Ok(())
    }
}
```

### 6. Build and Run

```rust
use sim_core::{SimulationRunner, SimulationConfig, PopulationHandler, ResourceAllocator};

// Configure
let config = SimulationConfig {
    name: "my-sim".to_string(),
    time_step: Duration::seconds(60),
    ..Default::default()
};

// Create runner
let mut runner = SimulationRunner::new(config);

// Add resources
let mut allocator = ResourceAllocator::new();
allocator.add_resource(MyResource { /* ... */ });
runner.set_resource_allocator(allocator);

// Add agents
let mut population = PopulationHandler::new();
population.add_agent(MyAgent { /* ... */ });
runner.set_population_handler(population);

// Set hooks
runner.set_hooks(Box::new(MyHooks {
    dispatcher: EventDispatcher::new(),
}));

// Run
runner.run_for_duration(Duration::hours(8))?;
```

## Module Structure

```
src/
├── agents.rs       - Customer agent implementation
├── resources.rs    - Kitchen resource and Order task
├── events.rs       - CloudEvents payload definitions
├── simulation.rs   - Simulation hooks and orchestration
└── lib.rs          - Public API
```

## Testing

Run the test suite:

```bash
cargo test -p sim-kitchen
```

All modules have comprehensive unit tests:
- Agent lifecycle and behavior
- Resource capacity management
- Event payload serialization
- Simulation setup and configuration

## Dependencies

- `sim-core 0.0.1` - Core simulation abstractions
- `sim-geo 0.0.1` - Geospatial components
- `sim-events 0.0.1` - CloudEvents infrastructure
- `chrono 0.4` - Time handling
- `geo 0.31` - Geospatial types
- `h3o 0.9` - H3 indexing
- `rand 0.9` - Random number generation
- `serde_json 1.0` - JSON serialization

## Version Information

Current version: `0.0.1`

See [CHANGELOG.md](CHANGELOG.md) for detailed version history.

## Documentation

- [Architecture](docs/architecture.md) - Multi-workflow patterns, reference implementation design
- [Patterns](docs/patterns.md) - Common usage patterns and best practices
- [Troubleshooting](docs/troubleshooting.md) - Common issues, debugging tips, and FAQ

## Performance

The simulation is lightweight and fast:
- 100 steps with 20 agents: < 1 second
- Memory usage: Minimal (all in-memory)
- Event emission: Buffered for efficiency

## Extension Ideas

### Add Geospatial Features

Use sim-geo for customer locations and journeys:

```rust
use sim_geo::{LocationHandler, MovementHandler};

// Track kitchen locations
let mut locations = LocationHandler::new();

// Track customer journeys
let mut movement = MovementHandler::new();

// In hooks: process journey requests from agents
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        for result in results {
            for journey in &result.journeys {
                self.movement.start_journey(journey)?;
            }
        }
        self.movement.advance_journeys(ctx.time_step())?;
        Ok(())
    }
}
```

### Add Multi-Step Workflows

Use workflow engine for complex food preparation:

```rust
use sim_core::{Workflow, WorkflowStep, WorkflowEngine};

#[derive(Debug)]
struct FoodPrepWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl Workflow for FoodPrepWorkflow {
    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

// Steps: Prep → Cook → Package
let workflow = FoodPrepWorkflow {
    id: WorkflowId::new(),
    steps: vec![
        WorkflowStep {
            step_index: 0,
            required_resource_type: "prep_station".to_string(),
            expected_duration: Duration::minutes(5),
            metadata: HashMap::new(),
        },
        // ...
    ],
};
```

### Add Realistic Data Generation

Generate realistic order patterns:

```rust
// Peak hours at lunch and dinner
fn order_probability(hour: u32) -> f64 {
    match hour {
        11..=13 => 0.4,  // Lunch rush
        18..=20 => 0.5,  // Dinner rush
        _ => 0.1,        // Off-peak
    }
}

// Item popularity distribution
fn random_items() -> Vec<String> {
    let mut rng = rand::rng();
    match rng.random_range(0..100) {
        0..=50 => vec!["burger".to_string()],
        51..=75 => vec!["burger".to_string(), "fries".to_string()],
        _ => vec!["burger".to_string(), "fries".to_string(), "drink".to_string()],
    }
}
```

## Related Crates

- `sim-core` - Core simulation framework
- `sim-geo` - Geospatial components (routing, H3 indexing)
- `sim-events` - CloudEvents infrastructure
- `sim-logistics` - Validation implementation (package delivery)

## License

MIT OR Apache-2.0
