# sim-core Usage Patterns

## Common Patterns

### Pattern 1: Simple Agent Simulation

**Use Case**: Model autonomous agents without resources or workflows.

```rust
use sim_core::{
    Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext,
    PopulationHandler, SimulationRunner, SimulationConfig,
};
use geo::Point;
use chrono::Duration;

#[derive(Debug)]
struct Wanderer {
    id: AgentId,
    position: Point<f64>,
    status: WandererStatus,
}

#[derive(Debug)]
enum WandererStatus {
    Idle,
    Moving,
}

impl AgentStatus for WandererStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Moving => "moving",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

impl Agent for Wanderer {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "wanderer"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.position)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        // Simple behavior: toggle between idle and moving
        self.status = match self.status {
            WandererStatus::Idle => WandererStatus::Moving,
            WandererStatus::Moving => WandererStatus::Idle,
        };

        Ok(result)
    }
}

fn main() -> sim_core::Result<()> {
    let config = SimulationConfig {
        name: "wanderers".to_string(),
        time_step: Duration::seconds(60),
        verbose: false,
        custom: Default::default(),
    };

    let mut runner = SimulationRunner::new(config);

    let mut population = PopulationHandler::new();
    for i in 0..10 {
        population.add_agent(Wanderer {
            id: AgentId::new(),
            position: Point::new(i as f64, i as f64),
            status: WandererStatus::Idle,
        })?;
    }

    runner.set_population_handler(population);
    runner.run_for_steps(100)?;

    println!("Simulation complete: {} steps", runner.stats().total_steps);
    Ok(())
}
```

**Key Points**:
- Agents manage their own state
- No resources or workflows needed
- Minimal setup

### Pattern 2: Resource Allocation

**Use Case**: Assign tasks to capacity-constrained resources.

```rust
use sim_core::{
    Resource, ResourceId, Task, TaskId, ResourceAllocator,
    AllocationStrategy, LeastLoaded,
};
use chrono::{Duration, Utc};

#[derive(Debug)]
struct Server {
    id: ResourceId,
    capacity: usize,
    current_load: usize,
}

impl Resource for Server {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "server"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.current_load
    }
}

#[derive(Debug)]
struct Request {
    id: TaskId,
}

impl Task for Request {
    fn id(&self) -> TaskId {
        self.id
    }

    fn task_type(&self) -> &str {
        "request"
    }

    fn required_resource_type(&self) -> &str {
        "server"
    }

    fn estimated_duration(&self) -> Duration {
        Duration::seconds(5)
    }
}

fn main() -> sim_core::Result<()> {
    let mut allocator = ResourceAllocator::new();

    // Add 3 servers with 10 capacity each
    for i in 0..3 {
        allocator.add_resource(Server {
            id: ResourceId::new(),
            capacity: 10,
            current_load: 0,
        })?;
    }

    // Add 25 requests
    for _ in 0..25 {
        allocator.add_task(Request {
            id: TaskId::new(),
        })?;
    }

    // Allocate using LeastLoaded strategy
    let assignments = allocator.assign_tasks(&LeastLoaded, Utc::now())?;

    println!("Assigned {} tasks to servers", assignments.len());

    for assignment in assignments {
        println!("Task {} -> Resource {}", assignment.task_id, assignment.resource_id);
    }

    Ok(())
}
```

**Key Points**:
- Resources define capacity
- Tasks specify requirements
- Strategy determines assignment
- Allocator tracks capacity automatically

### Pattern 3: Multi-Stage Workflows

**Use Case**: Tasks that require multiple processing steps.

```rust
use sim_core::{
    Workflow, WorkflowId, WorkflowStep, WorkflowEngine,
    ResourceAllocator, Resource, Task,
};
use chrono::Duration;
use std::collections::HashMap;

#[derive(Debug)]
struct ManufacturingWorkflow {
    id: WorkflowId,
    product_name: String,
}

impl Workflow for ManufacturingWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "manufacturing"
    }

    fn steps(&self) -> &[WorkflowStep] {
        static STEPS: &[WorkflowStep] = &[
            WorkflowStep {
                step_index: 0,
                required_resource_type: "assembly".to_string(),
                expected_duration: Duration::minutes(10),
                metadata: HashMap::new(),
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "testing".to_string(),
                expected_duration: Duration::minutes(5),
                metadata: HashMap::new(),
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "packaging".to_string(),
                expected_duration: Duration::minutes(3),
                metadata: HashMap::new(),
            },
        ];
        STEPS
    }
}

fn main() -> sim_core::Result<()> {
    let mut engine = WorkflowEngine::new();

    // Add workflow instances
    for i in 0..5 {
        engine.add_workflow(ManufacturingWorkflow {
            id: WorkflowId::new(),
            product_name: format!("Product-{}", i),
        })?;
    }

    // Simulate time progression
    engine.advance_workflows(Duration::minutes(10))?;

    let active_count = engine.active_workflows().len();
    println!("Active workflows: {}", active_count);

    Ok(())
}
```

**Key Points**:
- Workflows define ordered steps
- Each step specifies resource type and duration
- Engine manages workflow state transitions
- Automatic progression based on time

### Pattern 4: Simulation Hooks for Event Emission

**Use Case**: Record all simulation events for analysis.

```rust
use sim_core::{
    SimulationHooks, SimulationContext, AgentTickResult, Assignment,
};
use sim_events::{Event, EventDispatcher, EventPayload};
use serde_json::json;

struct EventEmittingHooks {
    dispatcher: EventDispatcher,
}

impl EventEmittingHooks {
    fn new() -> Self {
        Self {
            dispatcher: EventDispatcher::new(),
        }
    }

    fn flush_events(&mut self) -> sim_core::Result<usize> {
        let count = self.dispatcher.flush_to_parquet("simulation_events.parquet")?;
        Ok(count)
    }
}

#[derive(Debug)]
struct AgentTickedEvent {
    agent_id: String,
    step: usize,
}

impl EventPayload for AgentTickedEvent {
    fn event_type(&self) -> &str {
        "io.simulation.agent.ticked"
    }

    fn source(&self) -> String {
        format!("/agents/{}", self.agent_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "agent_id": self.agent_id,
            "step": self.step,
        })
    }
}

impl SimulationHooks for EventEmittingHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!("Simulation starting at {}", ctx.current_time());
        Ok(())
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        for result in results {
            let event = Event::new(Box::new(AgentTickedEvent {
                agent_id: result.agent_id.to_string(),
                step: ctx.step(),
            }));
            self.dispatcher.emit(event);
        }
        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!("Simulation ended at {}", ctx.current_time());
        let count = self.flush_events()?;
        println!("Wrote {} events to Parquet", count);
        Ok(())
    }
}
```

**Key Points**:
- Hooks provide lifecycle callbacks
- Events emitted during simulation
- Batch flush at end for efficiency

### Pattern 5: Conditional Execution

**Use Case**: Run simulation until a condition is met.

```rust
use sim_core::{SimulationRunner, SimulationConfig, SimulationContext, SimulationStats};
use chrono::Duration;

fn main() -> sim_core::Result<()> {
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);

    // Setup population, resources, etc.
    // ...

    // Run until 1000 agent ticks OR 24 simulated hours
    runner.run_until(|ctx: &SimulationContext, stats: &SimulationStats| {
        stats.total_agent_ticks >= 1000 || ctx.elapsed() >= Duration::hours(24)
    })?;

    println!("Stopped after {} steps", runner.stats().total_steps);
    println!("Agent ticks: {}", runner.stats().total_agent_ticks);
    println!("Elapsed time: {:?}", runner.context().elapsed());

    Ok(())
}
```

**Key Points**:
- Flexible stopping conditions
- Access to context and statistics
- Clean early termination

### Pattern 6: Custom Allocation Strategy

**Use Case**: Domain-specific task routing logic.

```rust
use sim_core::{AllocationStrategy, Resource, Task, ResourceId};

struct PriorityStrategy {
    priority_weights: std::collections::HashMap<String, f64>,
}

impl<R: Resource, T: Task> AllocationStrategy<R, T> for PriorityStrategy {
    fn assign(&self, task: &T, available: &[&R]) -> Option<ResourceId> {
        available
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .max_by(|a, b| {
                let a_weight = self.priority_weights
                    .get(a.resource_type())
                    .unwrap_or(&1.0);
                let b_weight = self.priority_weights
                    .get(b.resource_type())
                    .unwrap_or(&1.0);
                a_weight.partial_cmp(b_weight).unwrap()
            })
            .map(|r| r.id())
    }
}

fn main() -> sim_core::Result<()> {
    let mut weights = std::collections::HashMap::new();
    weights.insert("premium".to_string(), 2.0);
    weights.insert("standard".to_string(), 1.0);

    let strategy = PriorityStrategy {
        priority_weights: weights,
    };

    // Use strategy with ResourceAllocator
    // allocator.assign_tasks(&strategy, current_time)?;

    Ok(())
}
```

**Key Points**:
- Implement `AllocationStrategy<R, T>`
- Custom logic in `assign()` method
- Pluggable into existing infrastructure

### Pattern 7: Demand Generation

**Use Case**: Generate agent requests based on patterns.

```rust
use sim_core::{
    DemandGenerator, PoissonDemandGenerator, TimeOfDayDemandGenerator,
    CompositeDemandGenerator, Demand, AgentId, ResourceId,
};
use chrono::{Duration, NaiveTime, Utc};

fn main() -> sim_core::Result<()> {
    // Poisson arrivals (average 5 per hour)
    let poisson = PoissonDemandGenerator::new(
        5.0,
        "customer",
        "order",
    );

    // Time-of-day patterns (lunch rush at 12pm, dinner rush at 6pm)
    let mut peak_hours = std::collections::HashMap::new();
    peak_hours.insert(NaiveTime::from_hms_opt(12, 0, 0).unwrap(), 10.0);  // 10x multiplier
    peak_hours.insert(NaiveTime::from_hms_opt(18, 0, 0).unwrap(), 8.0);   // 8x multiplier

    let time_of_day = TimeOfDayDemandGenerator::new(
        3.0,  // Base rate
        peak_hours,
        "customer",
        "order",
    );

    // Combine generators
    let mut composite = CompositeDemandGenerator::new();
    composite.add_generator(Box::new(poisson));
    composite.add_generator(Box::new(time_of_day));

    // Generate demands
    let demands = composite.generate(
        Utc::now(),
        Duration::minutes(30),
        Default::default(),
    )?;

    println!("Generated {} demands", demands.len());

    Ok(())
}
```

**Key Points**:
- Multiple generator types for different patterns
- Composable via `CompositeDemandGenerator`
- Time-aware generation

### Pattern 8: Snapshot and Restore

**Use Case**: Save and resume simulation state.

```rust
use sim_core::{
    SnapshotManager, SnapshotData, SnapshotBackend,
    SimulationRunner, SimulationContext,
};
use std::path::PathBuf;

fn main() -> sim_core::Result<()> {
    let snapshot_dir = PathBuf::from("./snapshots");
    let manager = SnapshotManager::new(snapshot_dir);

    // Create and run simulation
    let mut runner = SimulationRunner::new(Default::default());
    // Setup agents, resources, etc.

    // Run for 100 steps
    runner.run_for_steps(100)?;

    // Save snapshot
    let snapshot_data = SnapshotData {
        step: runner.context().step(),
        agents: vec![],  // Serialize your agent data
        resources: vec![],
        metadata: Default::default(),
    };

    let snapshot_path = manager.save_snapshot(snapshot_data)?;
    println!("Saved snapshot to {:?}", snapshot_path);

    // Later: restore and continue
    let loaded_data = manager.load_snapshot(&snapshot_path)?;
    println!("Restored from step {}", loaded_data.step);

    // Reconstruct simulation state from loaded_data
    // Continue simulation...

    Ok(())
}
```

**Key Points**:
- Snapshot at any point
- Arrow-based serialization
- Resume from checkpoint

## Best Practices

### 1. Agent State Management
**Do**: Keep agent state in the agent struct
```rust
struct MyAgent {
    id: AgentId,
    state: MyState,  // State machine enum
    counters: MyCounters,
}
```

**Don't**: Store agent state externally in maps
```rust
// Avoid this pattern
struct Simulation {
    agents: Vec<AgentId>,
    agent_states: HashMap<AgentId, MyState>,  // Fragmented state
}
```

### 2. Resource Capacity Tracking
**Do**: Let `ResourceAllocator` manage capacity
```rust
allocator.add_task(task)?;
let assignments = allocator.assign_tasks(&strategy, current_time)?;
// Capacity automatically tracked
```

**Don't**: Manually track resource load
```rust
// Avoid this pattern
resource.current_load += 1;  // Easy to forget or desync
```

### 3. Event Emission
**Do**: Emit events in hooks
```rust
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) {
        for result in results {
            self.emit_event(...);
        }
    }
}
```

**Don't**: Emit events directly from agents
```rust
// Avoid this pattern
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        global_dispatcher.emit(...);  // Tight coupling
    }
}
```

### 4. Time Step Selection
**Do**: Choose appropriate granularity
```rust
// Retail simulation: 1-5 minute steps
let config = SimulationConfig {
    time_step: Duration::minutes(1),
    ..Default::default()
};

// Logistics simulation: 5-30 second steps
let config = SimulationConfig {
    time_step: Duration::seconds(30),
    ..Default::default()
};
```

**Don't**: Use excessively small steps
```rust
// Avoid unless necessary
let config = SimulationConfig {
    time_step: Duration::milliseconds(100),  // Usually too granular
    ..Default::default()
};
```

### 5. Component Initialization Order
**Do**: Initialize in this order
```rust
let mut runner = SimulationRunner::new(config);
runner.set_population_handler(population);  // 1. Agents
runner.set_resource_allocator(allocator);   // 2. Resources
runner.set_workflow_engine(engine);         // 3. Workflows
runner.set_hooks(Box::new(hooks));          // 4. Hooks last
```

**Don't**: Set hooks before other components
```rust
// Avoid this order
runner.set_hooks(Box::new(hooks));  // Hooks might expect components
runner.set_population_handler(population);
```

### 6. Error Handling
**Do**: Propagate errors with `?`
```rust
fn run_simulation() -> sim_core::Result<()> {
    let mut runner = SimulationRunner::new(config);
    runner.run_for_steps(100)?;  // Propagate errors
    Ok(())
}
```

**Don't**: Swallow errors silently
```rust
// Avoid this pattern
fn run_simulation() {
    let mut runner = SimulationRunner::new(config);
    let _ = runner.run_for_steps(100);  // Ignores errors!
}
```

## Anti-Patterns to Avoid

### Anti-Pattern 1: Global State
**Problem**: Agents accessing shared mutable state
```rust
// BAD: Global state
static mut GLOBAL_COUNTER: usize = 0;

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        unsafe {
            GLOBAL_COUNTER += 1;  // Race conditions, hard to test
        }
    }
}
```

**Solution**: Use agent-local state or pass via context
```rust
// GOOD: Agent-local state
struct MyAgent {
    local_counter: usize,
}

impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        self.local_counter += 1;  // Safe, testable
        Ok(Default::default())
    }
}
```

### Anti-Pattern 2: Direct Resource Modification
**Problem**: Bypassing the allocator
```rust
// BAD: Direct modification
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let resource = get_resource_somehow();
        resource.current_load += 1;  // Bypasses tracking!
        Ok(Default::default())
    }
}
```

**Solution**: Use ResourceAllocator
```rust
// GOOD: Use allocator
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        result.demands.push(Demand { ... });  // Request resource
        Ok(result)
    }
}
// In hooks: allocator.add_task(...)
```

### Anti-Pattern 3: Tight Coupling to sim-events
**Problem**: Agent directly using EventDispatcher
```rust
// BAD: Tight coupling
struct MyAgent {
    dispatcher: EventDispatcher,  // Agent knows about events
}
```

**Solution**: Emit via hooks
```rust
// GOOD: Decoupled
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();
        result.state_change = Some(json!({ "action": "moved" }));
        Ok(result)
    }
}

impl SimulationHooks for MyHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) {
        // Hook translates state changes to events
    }
}
```

### Anti-Pattern 4: Complex Agent Logic
**Problem**: Agents doing too much
```rust
// BAD: Agent handles routing, allocation, and events
impl Agent for ComplexAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        self.find_best_route();     // Should be in sim-geo
        self.allocate_resources();   // Should be in allocator
        self.emit_event();           // Should be in hooks
        Ok(Default::default())
    }
}
```

**Solution**: Separation of concerns
```rust
// GOOD: Agent focuses on behavior
impl Agent for SimpleAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        // Behavior: decide to request resources
        if self.should_order() {
            result.demands.push(self.create_demand());
        }

        // Framework handles routing, allocation, events
        Ok(result)
    }
}
```

## Integration Patterns

### Pattern: Combining sim-core + sim-geo + sim-events

```rust
use sim_core::{
    Agent, SimulationRunner, SimulationHooks, AgentTickResult,
};
use sim_geo::{LocationHandler, MovementHandler};
use sim_events::{Event, EventDispatcher};

struct IntegratedHooks {
    locations: LocationHandler,
    movement: MovementHandler,
    events: EventDispatcher,
}

impl SimulationHooks for IntegratedHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        // Process journeys with sim-geo
        for result in results {
            for journey in &result.journeys {
                self.movement.start_journey(journey.clone())?;
            }
        }
        self.movement.advance_journeys(ctx.time_step())?;

        // Emit events with sim-events
        let completed = self.movement.get_completed_journeys();
        for journey in completed {
            let event = Event::new(Box::new(JourneyCompletedEvent {
                agent_id: journey.agent_id.to_string(),
                destination: journey.destination,
            }));
            self.events.emit(event);
        }

        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        self.events.flush_to_parquet("events.parquet")?;
        Ok(())
    }
}
```

**Key Points**:
- Single hooks struct manages all integrations
- Geospatial tracking via sim-geo
- Event emission via sim-events
- Clean separation of concerns
