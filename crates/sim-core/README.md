# sim-core

Generic framework for agent-based simulations.

## Overview

`sim-core` provides domain-agnostic abstractions and runtime for building agent-based simulations. It handles population management, resource allocation, workflow execution, and time progression, allowing you to focus on your domain-specific logic.

## Features

- **Agent Population Management** - Generic agent handler with tick-based updates
- **Resource Allocation** - Capacity-constrained processors with pluggable strategies
- **Workflow Engine** - Multi-stage process orchestration
- **Simulation Runner** - Complete orchestration with lifecycle hooks
- **Time Management** - Configurable time steps and simulation context
- **Type-Safe IDs** - UUID-based identifiers (v5/v7) for all entities
- **Statistics Tracking** - Built-in metrics for monitoring

## Quick Start

### 1. Define Your Agent

```rust
use sim_core::{Agent, AgentStatus, AgentTickResult, SimulationContext, AgentId};
use geo::Point;

#[derive(Debug)]
struct Customer {
    id: AgentId,
    status: CustomerStatus,
    location: Point<f64>,
}

#[derive(Debug)]
enum CustomerStatus {
    Idle,
    Ordering,
    WaitingForFood,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Ordering => "ordering",
            Self::WaitingForFood => "waiting",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

impl Agent for Customer {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "customer"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        // Implement agent behavior
        let mut result = AgentTickResult::default();

        match self.status {
            CustomerStatus::Idle => {
                // Decide to order
                self.status = CustomerStatus::Ordering;
                // Add demand
                // ...
            }
            _ => {}
        }

        Ok(result)
    }
}
```

### 2. Create and Run Simulation

```rust
use sim_core::{SimulationRunner, SimulationConfig, PopulationHandler};
use chrono::Duration;

// Configure simulation
let config = SimulationConfig {
    name: "restaurant-sim".to_string(),
    time_step: Duration::seconds(30),  // 30 second steps
    verbose: false,
    custom: Default::default(),
};

// Create runner
let mut runner = SimulationRunner::new(config);

// Add population
let mut population = PopulationHandler::new();
population.add_agent(Customer { /* ... */ });
population.add_agent(Customer { /* ... */ });

runner.set_population_handler(population);

// Run simulation
runner.run_for_duration(Duration::hours(8))?;

println!("Simulation complete!");
println!("Steps: {}", runner.stats().total_steps);
println!("Agent ticks: {}", runner.stats().total_agent_ticks);
```

### 3. Add Lifecycle Hooks

```rust
use sim_core::SimulationHooks;

struct MyHooks;

impl SimulationHooks for MyHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!("Simulation starting at {}", ctx.current_time());
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        if ctx.step() % 100 == 0 {
            println!("Step {}: elapsed {:?}", ctx.step(), ctx.elapsed());
        }
        Ok(())
    }

    fn on_agents_ticked(&mut self, _ctx: &SimulationContext, results: &[AgentTickResult]) -> sim_core::Result<()> {
        // Emit events, log state changes, etc.
        println!("Processed {} agent ticks", results.len());
        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!("Simulation ended at {}", ctx.current_time());
        Ok(())
    }
}

runner.set_hooks(Box::new(MyHooks));
runner.run_for_steps(1000)?;
```

## Core Concepts

### Agents

Entities that exist in the simulation and exhibit behavior. Implement the `Agent` trait:

```rust
pub trait Agent: Send + Sync {
    fn id(&self) -> AgentId;
    fn agent_type(&self) -> &str;
    fn status(&self) -> &dyn AgentStatus;
    fn position(&self) -> Option<Point<f64>>;
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult>;
}
```

Each tick returns:
- **Journeys**: Movement requests (origin → destination)
- **Demands**: Resource requests (orders, tasks, etc.)
- **State changes**: Arbitrary JSON metadata

### Resources

Capacity-constrained processors that handle tasks:

```rust
pub trait Resource: Send + Sync + Debug {
    fn id(&self) -> ResourceId;
    fn resource_type(&self) -> &str;
    fn capacity(&self) -> usize;
    fn current_load(&self) -> usize;
}
```

Built-in allocation strategies:
- **RoundRobin** - Distribute evenly across resources
- **LeastLoaded** - Assign to resource with lowest load
- **Random** - Random assignment

### Workflows

Multi-stage processing pipelines:

```rust
pub trait Workflow: Send + Sync + Debug {
    fn id(&self) -> WorkflowId;
    fn workflow_type(&self) -> &str;
    fn steps(&self) -> &[WorkflowStep];
}
```

Each step specifies:
- Required resource type
- Expected duration
- Custom metadata

### SimulationContext

Runtime state passed to all agent ticks:

```rust
pub struct SimulationContext {
    // Current simulation time
    pub fn current_time(&self) -> DateTime<Utc>;

    // Time since simulation started
    pub fn elapsed(&self) -> Duration;

    // Current step number
    pub fn step(&self) -> usize;

    // Time advanced per step
    pub fn time_step(&self) -> Duration;

    // Configuration
    pub fn config(&self) -> &SimulationConfig;
}
```

## SimulationRunner API

### Execution Modes

**Fixed Steps**:
```rust
runner.run_for_steps(100)?;  // Run exactly 100 steps
```

**Duration**:
```rust
runner.run_for_duration(Duration::days(7))?;  // Run for 7 simulated days
```

**Conditional**:
```rust
runner.run_until(|ctx, stats| {
    stats.total_agent_ticks >= 10000 ||
    ctx.elapsed() >= Duration::hours(24)
})?;
```

**Manual**:
```rust
for step in 0..100 {
    runner.tick()?;
    // Custom logic
}
```

### Component Management

```rust
// Optional components
runner.set_population_handler(population);
runner.set_resource_allocator(allocator);
runner.set_workflow_engine(engine);
runner.set_hooks(Box::new(hooks));

// Access during simulation
if let Some(pop) = runner.population() {
    println!("Agent count: {}", pop.count());
}
```

### Statistics

```rust
pub struct SimulationStats {
    pub total_steps: usize,
    pub total_agent_ticks: usize,
    pub total_assignments: usize,
    pub total_workflow_steps: usize,
}

let stats = runner.stats();
println!("Throughput: {} ticks/step",
    stats.total_agent_ticks / stats.total_steps);
```

## Type-Safe Identifiers

All entities have strongly-typed UUIDs:

```rust
use sim_core::{AgentId, ResourceId, TaskId, LocationId, WorkflowId};

// Create new (UUID v7 - time-ordered)
let id = AgentId::new();

// Create deterministic (UUID v5)
let id = AgentId::new_deterministic("customer-123");

// Use in agent
struct MyAgent {
    id: AgentId,  // Not String!
}
```

Benefits:
- Type safety (can't accidentally mix IDs)
- Efficient comparison and hashing
- Time-ordered (v7) for better indexing
- Deterministic (v5) for reproducibility

## Advanced Usage

### Custom Configuration

```rust
let mut config = SimulationConfig::default();
config.custom.insert(
    "spawn_rate".to_string(),
    serde_json::json!(5),  // 5 agents per step
);

// Access in agents
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let spawn_rate = ctx.config().custom
        .get("spawn_rate")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    // ...
}
```

### Resource Allocation

```rust
use sim_core::{ResourceAllocator, LeastLoaded};

let mut allocator = ResourceAllocator::new();
allocator.add_resource(kitchen1);
allocator.add_resource(kitchen2);

allocator.add_task(order1);
allocator.add_task(order2);

// Allocate using strategy
let assignments = allocator.assign_tasks(&LeastLoaded)?;

for assignment in assignments {
    println!("Task {} → Resource {}", assignment.task_id, assignment.resource_id);
}
```

### Custom Allocation Strategy

```rust
use sim_core::{AllocationStrategy, Resource, Task, ResourceId};

struct PriorityStrategy;

impl AllocationStrategy<MyResource, MyTask> for PriorityStrategy {
    fn assign(&self, task: &MyTask, available: &[&MyResource]) -> Option<ResourceId> {
        available
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .max_by_key(|r| task.priority())
            .map(|r| r.id())
    }
}

runner.allocate_resources(&PriorityStrategy)?;
```

## Integration

### With sim-geo

For spatial simulations:

```rust
use sim_geo::{LocationHandler, MovementHandler};

// Add to hooks
impl SimulationHooks for GeoHooks {
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

### With sim-events

For event emission:

```rust
use sim_events::{EventDispatcher, Event, EventPayload};

struct EventHooks {
    dispatcher: EventDispatcher,
}

impl SimulationHooks for EventHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        for result in results {
            for demand in &result.demands {
                let event = Event::new(Box::new(DemandCreatedEvent {
                    demand_type: demand.demand_type.clone(),
                    timestamp: ctx.current_time(),
                }));
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

## Examples

See tests in each module for working examples:
- `population::handler::tests` - Agent management
- `resources::allocator::tests` - Resource allocation
- `workflows::engine::tests` - Workflow execution
- `runner::tests` - Full simulation orchestration

## Architecture

```
sim-core
├── population/     Agent abstractions and management
├── resources/      Resource allocation and strategies
├── workflows/      Multi-stage process orchestration
├── runner/         Main simulation orchestrator
├── context/        Time management and configuration
└── utils/          Type-safe identifiers and helpers
```

## Dependencies

- `chrono` - Time handling
- `geo` - Geospatial types (Point)
- `uuid` - Unique identifiers
- `serde_json` - Metadata serialization
- `indexmap` - Ordered maps
- `rand` - Random number generation

## Related Crates

- `sim-geo` - Geospatial components (locations, movement, H3)
- `sim-events` - CloudEvents-compliant event infrastructure
- `sim-telemetry` - Metrics and tracing (planned)

## License

Apache-2.0
