# Cross-Crate Integration Patterns

This document describes common patterns for integrating sim-core, sim-geo, sim-events, and sim-telemetry to build complete simulations.

## Table of Contents

- [SimulationHooks Pattern](#simulationhooks-pattern)
- [TaskFactory Pattern](#taskfactory-pattern)
- [Handler Coordination](#handler-coordination)
- [Complete Simulation Template](#complete-simulation-template)

---

## SimulationHooks Pattern

SimulationHooks provide lifecycle callbacks for coordinating cross-cutting concerns like events, telemetry, and geospatial movement.

### Basic Structure

```rust
use sim_core::{SimulationHooks, SimulationContext, AgentTickResult, Assignment};
use sim_events::EventDispatcher;
use sim_geo::MovementHandler;

struct IntegratedSimulationHooks {
    // Events
    event_dispatcher: EventDispatcher,
    event_writer: ParquetEventWriter,

    // Telemetry
    telemetry_collector: TelemetryCollector,

    // Geospatial
    movement_handler: MovementHandler,
    location_handler: LocationHandler,

    // Domain-specific state
    order_tracker: HashMap<TaskId, OrderInfo>,
}

impl SimulationHooks for IntegratedSimulationHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Initialize state
        Ok(())
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult]
    ) -> Result<()> {
        // 1. Start journeys requested by agents
        for result in results {
            for journey_req in &result.new_journeys {
                self.movement_handler.start_journey(journey_req.clone())?;
            }
        }

        // 2. Process demands (convert to tasks)
        for result in results {
            for demand in &result.demands {
                self.process_demand(demand, ctx)?;
            }
        }

        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment]
    ) -> Result<()> {
        // Emit assignment events
        for assignment in assignments {
            let event = Event::new(Box::new(TaskAssignedEvent {
                task_id: assignment.task_id.to_string(),
                resource_id: assignment.resource_id.to_string(),
                timestamp: ctx.current_time(),
            }));
            self.event_dispatcher.emit(event)?;
        }

        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Update journeys
        let completed_journeys = self.movement_handler.tick(ctx.current_time())?;

        // Handle completed journeys
        for journey_id in completed_journeys {
            self.handle_journey_completion(journey_id, ctx)?;
        }

        // Flush events and telemetry periodically
        if ctx.step() % 100 == 0 {
            self.flush_events_and_telemetry()?;
        }

        Ok(())
    }

    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        // Final flush
        self.flush_events_and_telemetry()?;
        Ok(())
    }
}

impl IntegratedSimulationHooks {
    fn process_demand(&mut self, demand: &Demand, ctx: &SimulationContext) -> Result<()> {
        // Convert demand to domain-specific task
        // Emit creation event
        // Track in domain state
        Ok(())
    }

    fn handle_journey_completion(&mut self, journey_id: JourneyId, ctx: &SimulationContext) -> Result<()> {
        if let Some(journey) = self.movement_handler.get_journey(journey_id) {
            // Update agent position
            // Emit journey completed event
            // Trigger next workflow step
        }
        Ok(())
    }

    fn flush_events_and_telemetry(&mut self) -> Result<()> {
        // Drain events
        let events = self.event_dispatcher.drain();

        // Write to Parquet
        self.event_writer.write_events(&events)?;

        // Generate telemetry
        for event in &events {
            let event_json = serde_json::to_value(event)?;
            self.telemetry_collector.process_event(&event_json)?;
        }

        // Export telemetry
        let telemetry_records = self.telemetry_collector.drain();
        export_json_logs(&telemetry_records, "./telemetry/logs.jsonl")?;

        Ok(())
    }
}
```

---

## TaskFactory Pattern

TaskFactory bridges workflows and resource allocation.

### Interface

```rust
use sim_core::{Task, WorkflowStep, WorkflowInstanceId};

pub trait TaskFactory<T: Task>: Send + Sync {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>
    ) -> T;
}
```

### Implementation Example

```rust
use sim_core::{TaskId, Task, WorkflowStep, WorkflowInstanceId};

#[derive(Debug, Clone)]
struct FoodOrderTask {
    id: TaskId,
    workflow_instance_id: Option<WorkflowInstanceId>,
    items: Vec<String>,
    prep_time: Duration,
}

impl Task for FoodOrderTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "kitchen_station"
    }

    fn estimated_duration(&self) -> Duration {
        self.prep_time
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

struct FoodOrderTaskFactory {
    order_items: Arc<Mutex<HashMap<WorkflowInstanceId, Vec<String>>>>,
}

impl TaskFactory<FoodOrderTask> for FoodOrderTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>
    ) -> FoodOrderTask {
        // Get items from instance metadata
        let items = self.order_items
            .lock()
            .unwrap()
            .get(&instance_id)
            .cloned()
            .unwrap_or_else(|| vec!["default_item".to_string()]);

        // Parse duration from workflow step
        let prep_time = step.expected_duration;

        FoodOrderTask {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            items,
            prep_time,
        }
    }
}
```

### Integration with Runner

```rust
// Create workflow engine
let mut workflow_engine = WorkflowEngine::new();
workflow_engine.register_workflow(food_order_workflow);

// Create task factory
let task_factory = Box::new(FoodOrderTaskFactory {
    order_items: Arc::new(Mutex::new(HashMap::new())),
});

// Register with runner
runner.set_workflow_engine(workflow_engine);
runner.set_task_factory(task_factory);

// The runner will automatically:
// 1. Identify ready workflow instances
// 2. Call task_factory.create_task() for each ready step
// 3. Add tasks to resource allocator
// 4. Mark instance as having task assigned
```

---

## Handler Coordination

### Population + Resources + Workflows

```rust
// Setup handlers
let mut population = PopulationHandler::new();
let mut resources = ResourceAllocator::new();
let mut workflow_engine = WorkflowEngine::new();

// Add entities
population.add_agent(customer);
resources.add_resource(kitchen);
workflow_engine.register_workflow(cooking_workflow);

// Register with runner
runner.set_population_handler(population);
runner.set_resource_allocator(resources);
runner.set_workflow_engine(workflow_engine);

// Tick coordination (handled automatically by runner):
// 1. Agents tick → generate demands
// 2. Demands → create workflow instances
// 3. Workflow instances → create tasks
// 4. Tasks → allocate to resources
// 5. Completed tasks → advance workflow
```

### Population + Geo + Events

```rust
struct GeoEventSimulationHooks {
    movement_handler: MovementHandler,
    location_handler: LocationHandler,
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for GeoEventSimulationHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        for result in results {
            // Start journeys
            for journey_req in &result.new_journeys {
                let journey_id = self.movement_handler.start_journey(journey_req.clone())?;

                // Emit journey started event
                self.event_dispatcher.emit(Event::new(Box::new(JourneyStartedEvent {
                    journey_id: journey_id.to_string(),
                    agent_id: journey_req.agent_id.to_string(),
                    origin: (journey_req.origin.x(), journey_req.origin.y()),
                    destination: (journey_req.destination.x(), journey_req.destination.y()),
                    timestamp: ctx.current_time(),
                })))?;
            }
        }
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Update journeys and emit completion events
        let completed = self.movement_handler.tick(ctx.current_time())?;

        for journey_id in completed {
            if let Some(journey) = self.movement_handler.get_journey(journey_id) {
                self.event_dispatcher.emit(Event::new(Box::new(JourneyCompletedEvent {
                    journey_id: journey.id.to_string(),
                    agent_id: journey.agent_id.to_string(),
                    distance_meters: journey.distance_meters,
                    duration_seconds: journey.actual_end_time
                        .unwrap()
                        .signed_duration_since(journey.start_time)
                        .num_seconds() as f64,
                    timestamp: ctx.current_time(),
                })))?;
            }
        }

        Ok(())
    }
}
```

---

## Complete Simulation Template

### Directory Structure

```
my-simulation/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── agents/
│   │   ├── mod.rs
│   │   └── my_agent.rs
│   ├── resources/
│   │   ├── mod.rs
│   │   ├── my_resource.rs
│   │   └── my_task.rs
│   ├── workflows/
│   │   ├── mod.rs
│   │   └── my_workflow.rs
│   ├── events/
│   │   ├── mod.rs
│   │   └── my_events.rs
│   ├── simulation/
│   │   ├── mod.rs
│   │   └── hooks.rs
│   └── main.rs
└── examples/
    └── basic.rs
```

### Cargo.toml

```toml
[package]
name = "my-simulation"
version = "0.1.0"
edition = "2021"

[dependencies]
sim-core = { path = "../sim-core" }
sim-geo = { path = "../sim-geo" }
sim-events = { path = "../sim-events" }
sim-telemetry = { path = "../sim-telemetry" }

chrono = "0.4"
geo-types = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
uuid = { version = "1.0", features = ["v7"] }
```

### main.rs Template

```rust
use sim_core::{SimulationRunner, SimulationConfig};
use chrono::Duration;

mod agents;
mod resources;
mod workflows;
mod events;
mod simulation;

use simulation::MySimulationHooks;

fn main() -> Result<()> {
    // Configuration
    let config = SimulationConfig {
        time_step: Duration::seconds(30),
        ..Default::default()
    };

    // Create runner
    let mut runner = SimulationRunner::new(config);

    // Setup handlers
    let population = /* setup population */;
    let resources = /* setup resources */;
    let workflow_engine = /* setup workflows */;

    runner.set_population_handler(population);
    runner.set_resource_allocator(resources);
    runner.set_workflow_engine(workflow_engine);

    // Setup hooks
    let hooks = MySimulationHooks::new("./output")?;
    runner.set_hooks(Box::new(hooks));

    // Run simulation
    println!("Starting simulation...");
    runner.run_for_steps(1000)?;

    // Report statistics
    let stats = runner.stats();
    println!("Simulation complete:");
    println!("  Total steps: {}", stats.total_steps);
    println!("  Agent ticks: {}", stats.total_agent_ticks);
    println!("  Assignments: {}", stats.total_assignments);

    Ok(())
}
```

---

## Common Integration Scenarios

### Scenario 1: Basic Agent + Resources

Minimal setup without workflows or geo.

```rust
// Just agents and resources
runner.set_population_handler(population);
runner.set_resource_allocator(resources);

// Simple hooks for events
struct SimpleHooks {
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for SimpleHooks {
    fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) -> Result<()> {
        for assignment in assignments {
            // Emit event
        }
        Ok(())
    }
}
```

### Scenario 2: Agent + Workflows + Resources

Multi-stage processing.

```rust
// Add workflow support
runner.set_workflow_engine(workflow_engine);
runner.set_task_factory(task_factory);

// Hooks bridge agents → workflows → tasks → resources
impl SimulationHooks for WorkflowHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        // Convert demands to workflow instances
        Ok(())
    }
}
```

### Scenario 3: Agent + Geo + Events

Movement simulation.

```rust
// Add geospatial support
struct GeoHooks {
    movement_handler: MovementHandler,
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for GeoHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        // Start journeys from agent requests
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Update journeys, emit events
        Ok(())
    }
}
```

### Scenario 4: Full Integration

All components together.

```rust
struct FullIntegrationHooks {
    // Events & Telemetry
    event_dispatcher: EventDispatcher,
    event_writer: ParquetEventWriter,
    telemetry_collector: TelemetryCollector,

    // Geospatial
    movement_handler: MovementHandler,
    location_handler: LocationHandler,

    // Domain state
    domain_state: MyDomainState,
}

// Implement all hook methods to coordinate
```

---

## See Also

- [sim-core API Reference](./sim-core.md)
- [sim-geo API Reference](./sim-geo.md)
- [sim-events API Reference](./sim-events.md)
- [sim-telemetry API Reference](./sim-telemetry.md)
- [Kitchen Example](../examples/kitchen/multi-workflow.md)
- [Logistics Example](../examples/logistics/package-routing.md)
