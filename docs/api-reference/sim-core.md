# sim-core API Reference

`sim-core` is the foundational crate providing the core abstractions and runtime for building agent-based simulations. It's designed to be domain-agnostic, enabling simulations across logistics, retail, manufacturing, services, and more.

## Table of Contents

- [Core Traits](#core-traits)
- [Handlers](#handlers)
- [Simulation Runtime](#simulation-runtime)
- [Demand Generation](#demand-generation)
- [Workflows](#workflows)
- [Snapshot Management](#snapshot-management)
- [Utilities](#utilities)

---

## Core Traits

### Agent

The fundamental trait for entities with autonomous behavior.

```rust
pub trait Agent: Clone + Send + Sync + 'static {
    fn id(&self) -> AgentId;
    fn agent_type(&self) -> &str;
    fn status(&self) -> &dyn AgentStatus;
    fn position(&self) -> Option<Point<f64>>;
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult>;
    fn state_data(&self) -> Value { /* ... */ }
    fn to_arrow_row(&self) -> ArrowAgentRow { /* ... */ }
    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self>;
}
```

**Key Methods:**

- `id()` - Returns unique identifier for this agent
- `agent_type()` - Returns type string for grouping/filtering (e.g., "customer", "courier")
- `status()` - Returns current status (domain-specific enum implementing `AgentStatus`)
- `position()` - Returns spatial location if applicable, None for non-spatial agents
- `tick()` - Core behavior method called each simulation step
- `state_data()` - Optional: Custom state for persistence
- `to_arrow_row()` / `from_arrow_row()` - Serialization for Arrow storage

**Example:**

```rust
use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};
use geo_types::Point;

#[derive(Clone, Debug)]
enum CustomerStatus {
    Idle,
    Ordering,
    WaitingForDelivery,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            CustomerStatus::Idle => "idle",
            CustomerStatus::Ordering => "ordering",
            CustomerStatus::WaitingForDelivery => "waiting",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, CustomerStatus::Idle)
    }
}

#[derive(Clone)]
struct Customer {
    id: AgentId,
    status: CustomerStatus,
    position: Point<f64>,
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
        Some(self.position)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Update agent state based on simulation time
        let mut result = AgentTickResult::default();

        match self.status {
            CustomerStatus::Idle => {
                // Decide to order (probabilistic)
                if rand::random::<f64>() < 0.1 {
                    self.status = CustomerStatus::Ordering;
                }
            }
            CustomerStatus::Ordering => {
                // Create demand for food order
                result.demands.push(Demand {
                    demand_type: "food_order".to_string(),
                    location: Some(self.position),
                    metadata: HashMap::new(),
                });
                self.status = CustomerStatus::WaitingForDelivery;
            }
            _ => {}
        }

        Ok(result)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        // Deserialize from Arrow storage
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?
        );

        Ok(Customer {
            id,
            status: CustomerStatus::Idle,
            position: Point::new(
                row.position_x.unwrap_or(0.0),
                row.position_y.unwrap_or(0.0),
            ),
        })
    }
}
```

### AgentStatus

Domain-specific status enums must implement this trait.

```rust
pub trait AgentStatus: Send + Sync + fmt::Debug {
    fn as_str(&self) -> &str;
    fn is_idle(&self) -> bool { false }
    fn is_active(&self) -> bool { !self.is_idle() }
}
```

### Resource

Represents capacity-constrained processors (machines, stations, workers).

```rust
pub trait Resource: Send + Sync + fmt::Debug {
    fn id(&self) -> ResourceId;
    fn resource_type(&self) -> &str;
    fn capacity(&self) -> usize;
    fn current_load(&self) -> usize;
    fn is_available(&self) -> bool { /* ... */ }
    fn available_capacity(&self) -> usize { /* ... */ }
}
```

**Example:**

```rust
#[derive(Debug)]
struct KitchenStation {
    id: ResourceId,
    capacity: usize,
    current_load: usize,
}

impl Resource for KitchenStation {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "kitchen_station"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.current_load
    }
}
```

### Task

Represents work items that require processing by resources.

```rust
pub trait Task: Send + Sync + fmt::Debug {
    fn id(&self) -> TaskId;
    fn required_resource_type(&self) -> &str;
    fn estimated_duration(&self) -> Duration;
    fn priority(&self) -> i32 { 0 }
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> { None }
}
```

**Example:**

```rust
#[derive(Debug)]
struct FoodOrder {
    id: TaskId,
    items: Vec<String>,
    prep_time: Duration,
    workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for FoodOrder {
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
```

### Workflow

Multi-stage processing pipelines.

```rust
pub trait Workflow: Send + Sync + fmt::Debug {
    fn id(&self) -> WorkflowId;
    fn workflow_type(&self) -> &str;
    fn steps(&self) -> &[WorkflowStep];
    fn step_count(&self) -> usize { /* ... */ }
    fn get_step(&self, index: usize) -> Option<&WorkflowStep> { /* ... */ }
}
```

---

## Handlers

### PopulationHandler

Manages agent lifecycle and state.

```rust
pub struct PopulationHandler<A: Agent> { /* ... */ }

impl<A: Agent> PopulationHandler<A> {
    pub fn new() -> Self;
    pub fn add_agent(&mut self, agent: A);
    pub fn remove_agent(&mut self, id: AgentId) -> Option<A>;
    pub fn get_agent(&self, id: AgentId) -> Option<A>;            // decoded copy
    pub fn update_agent(&mut self, agent: A) -> Result<()>;       // write a modified copy back
    pub fn count(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = &A>;
    pub fn tick_agents(&mut self, ctx: &SimulationContext) -> Result<Vec<AgentTickResult>>;
    pub fn export_batch(&self) -> RecordBatch;
    pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()>;
}
```

**Example:**

```rust
use sim_core::PopulationHandler;

let mut population = PopulationHandler::new();

// Add agents
population.add_agent(Customer { /* ... */ });
population.add_agent(Customer { /* ... */ });

// Tick all agents
let results = population.tick_agents(&ctx)?;

// Query agents
println!("Total agents: {}", population.count());
```

### ResourceAllocator

Tracks resource capacity and the tasks queued for it. Every task takes one unit
of capacity. Pending tasks are allocated highest priority first (ties in
insertion order); an allocated task finishes once `estimated_duration` has
elapsed, which releases its capacity and removes it.

```rust
pub struct ResourceAllocator<R: Resource, T: Task> { /* ... */ }

impl<R: Resource, T: Task> ResourceAllocator<R, T> {
    pub fn new() -> Self;

    // Resources
    pub fn add_resource(&mut self, resource: R) -> Result<()>;      // error on duplicate ID
    pub fn remove_resource(&mut self, id: ResourceId) -> Result<bool>; // error while tasks are allocated to it
    pub fn resource_count(&self) -> usize;
    pub fn resource_capacity(&self, id: ResourceId) -> Option<usize>;
    pub fn resource_available(&self, id: ResourceId) -> Option<usize>;
    pub fn find_available_resources_by_type(&self, resource_type: &str) -> Vec<ResourceId>;

    // Tasks
    pub fn add_task(&mut self, task: T) -> Result<()>;              // error on duplicate ID
    pub fn remove_task(&mut self, id: TaskId) -> Result<bool>;      // releases capacity if allocated
    pub fn complete_task(&mut self, id: TaskId) -> Result<CompletedTask>; // finish early
    pub fn pending_task_count(&self) -> usize;
    pub fn allocated_task_count(&self) -> usize;
    pub fn pending_tasks(&self) -> impl Iterator<Item = TaskId>;
    pub fn get_assignment(&self, id: TaskId) -> Option<ResourceId>;
    pub fn assignments(&self) -> Vec<Assignment>;

    // Allocation and completion
    pub fn assign_tasks(&mut self, strategy: &dyn AllocationStrategy, now: DateTime<Utc>)
        -> Result<Vec<Assignment>>;
    pub fn allocate_batch(&mut self, ids: impl IntoIterator<Item = TaskId>,
        strategy: &dyn AllocationStrategy, now: DateTime<Utc>) -> Result<Vec<Assignment>>;
    pub fn get_completed_tasks(&mut self, now: DateTime<Utc>) -> Result<Vec<CompletedTask>>;

    // Snapshots
    pub fn export_batch(&self) -> Result<(RecordBatch, RecordBatch)>;
    pub fn import_batch(&mut self, resources: RecordBatch, tasks: RecordBatch) -> Result<()>;
}

pub struct CompletedTask {
    pub task_id: TaskId,
    pub resource_id: ResourceId,
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}
```

Inside a `SimulationRunner` you rarely call these directly: each `tick()`
completes due tasks, creates workflow tasks and allocates.

### AllocationStrategy

Chooses a resource for one pending task. The allocator passes only resources
of the required type with free capacity, in the order they were added.

```rust
pub trait AllocationStrategy: Send + Sync {
    fn assign(&self, task: &TaskCandidate<'_>, candidates: &[ResourceCandidate<'_>])
        -> Option<ResourceId>;   // None leaves the task pending
}

pub struct TaskCandidate<'a> {
    pub id: TaskId,
    pub required_resource_type: &'a str,
    pub priority: i32,
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

pub struct ResourceCandidate<'a> {
    pub id: ResourceId,
    pub resource_type: &'a str,
    pub capacity: usize,
    pub available: usize,       // current_load() = capacity - available
}
```

**Built-in strategies** (`sim_core::resources`):

- `LeastLoaded` (runner default): lowest current load, ties to the first added
- `FirstFit`: first candidate
- `RoundRobin`: rotates through the candidates
- `Random`: uniform; use `Random::with_seed(derive_seed(seed, "allocation"))` for reproducible runs

---

## Simulation Runtime

### SimulationRunner

Main orchestrator coordinating all simulation components.

```rust
pub struct SimulationRunner<A, R, T, W> { /* ... */ }

impl<A, R, T, W> SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    pub fn new(config: SimulationConfig) -> Self;
    pub fn with_start_time(start_time: DateTime<Utc>, config: SimulationConfig) -> Self;

    // Component registration
    pub fn set_population_handler(&mut self, handler: PopulationHandler<A>);
    pub fn set_resource_allocator(&mut self, allocator: ResourceAllocator<R, T>);
    pub fn set_workflow_registry(&mut self, registry: WorkflowRegistry<W>);  // use this, not set_workflow_engine
    pub fn set_task_factory(&mut self, factory: Box<dyn TaskFactory<T>>);
    pub fn set_demand_generator(&mut self, generator: Box<dyn DemandGenerator>);
    pub fn set_allocation_strategy(&mut self, strategy: Box<dyn AllocationStrategy>); // default LeastLoaded
    pub fn set_hooks(&mut self, hooks: Box<dyn SimulationHooks>);
    pub fn set_time_mode(&mut self, mode: TimeMode);

    // Control channel (bidirectional; attach before calling run_controlled / run_controlled_for)
    pub fn attach_control(&mut self, rx: ControlReceiver);
    pub fn process_control_commands(&mut self) -> Result<bool>;  // true = Stop received
    pub fn is_paused(&self) -> bool;
    pub fn emit_event(&self, event: SimEvent);

    // Execution
    pub fn tick(&mut self) -> Result<TickResult>;
    pub fn run_for_steps(&mut self, steps: usize) -> Result<()>;
    pub fn run_for_duration(&mut self, duration: Duration) -> Result<()>;
    pub fn run_until<F>(&mut self, condition: F) -> Result<()>
        where F: FnMut(&SimulationContext, &SimulationStats) -> bool;
    pub fn run_controlled(&mut self) -> Result<()>;          // runs until Stop
    pub fn run_controlled_for(&mut self, max_steps: usize) -> Result<()>;
    pub fn allocate_resources(&mut self) -> Result<usize>;  // tick() already does this; for tasks added between ticks

    // State management
    pub async fn init_snapshot_manager(&mut self) -> Result<()>;
    pub async fn set_snapshot_backend(&mut self, backend: SnapshotBackend) -> Result<()>;
    pub async fn save_snapshot(&self, name: &str) -> Result<SnapshotMetadata>;
    pub async fn restore_snapshot(&mut self, name: &str) -> Result<SnapshotMetadata>;  // all-or-nothing

    // Querying
    pub async fn query(&self, sql: &str) -> Result<Vec<RecordBatch>>;

    // Accessors
    pub fn context(&self) -> &SimulationContext;
    pub fn context_mut(&mut self) -> &mut SimulationContext;
    pub fn stats(&self) -> &SimulationStats;
}
```

**Tick order.** Each `tick()` runs:

1. `on_before_tick`
2. demand generation, then `on_demands_generated`
3. agent ticks, then `on_agents_ticked`
4. task completion: allocated tasks whose duration has elapsed release their
   resource (`on_tasks_completed`); workflow tasks advance their instance, and
   each step event goes to `on_workflow_step_completed`
5. the `TaskFactory` creates a task for every workflow instance ready for its
   next step
6. pending tasks are allocated with the runner's strategy, then
   `on_resources_allocated`
7. time advances, then `on_after_tick`

**Example:**

```rust
use sim_core::{SimulationRunner, SimulationConfig};

// Create runner
let config = SimulationConfig {
    time_step: Duration::seconds(30),
    ..Default::default()
};
let mut runner = SimulationRunner::new(config);

// Register components
runner.set_population_handler(population);
runner.set_resource_allocator(allocator);
let mut registry = WorkflowRegistry::new();
registry.register("orders".to_string(), workflow_engine)?;
runner.set_workflow_registry(registry);

// Run simulation
runner.run_for_steps(100)?;

// Check stats
println!("Total steps: {}", runner.stats().total_steps);
println!("Agent ticks: {}", runner.stats().total_agent_ticks);
```

### SimulationHooks

Lifecycle callbacks for custom logic.

```rust
pub trait SimulationHooks: Send + Sync {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()> { Ok(()) }
    fn on_before_tick(&mut self, ctx: &SimulationContext) -> Result<()> { Ok(()) }
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> { Ok(()) }
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> { Ok(()) }
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> { Ok(()) }
    fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) -> Result<()> { Ok(()) }
    fn on_tasks_completed(&mut self, ctx: &SimulationContext, completed: &[CompletedTask]) -> Result<()> { Ok(()) }
    fn on_workflow_step_completed(&mut self, ctx: &SimulationContext, event: &WorkflowStepEvent) -> Result<()> { Ok(()) }
    fn on_demands_generated(&mut self, ctx: &SimulationContext, demands: &[Demand]) -> Result<()> { Ok(()) }
}
```

**Example:**

```rust
struct MySimulationHooks {
    event_dispatcher: EventDispatcher,
}

impl SimulationHooks for MySimulationHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        // Process demands from agents
        for result in results {
            for demand in &result.demands {
                // Convert demand to task
                // Emit event
            }
        }
        Ok(())
    }

    fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) -> Result<()> {
        // Emit assignment events
        for assignment in assignments {
            let event = Event::new(Box::new(TaskAssignedEvent { /* ... */ }), ctx.current_time());
            self.event_dispatcher.emit(event);
        }
        Ok(())
    }
}
```

### Control Channel

Bidirectional channel for controlling a simulation from an external caller (TUI, test harness, HTTP handler).

```rust
// Factory — returns matched (caller-side, runner-side) pair
pub fn control_channel(bound: usize) -> (SimulationController, ControlReceiver);

/// Commands sent from caller → runner
pub enum ControlCommand {
    Start,
    Stop,
    Pause,
    Resume,
    StepN(usize),
    SetMode(TimeMode),
}

/// Events emitted by runner → caller
pub enum SimEvent {
    TickCompleted { step: usize, sim_time: DateTime<Utc>, event_counts: HashMap<String, u64> },
    ModeChanged { mode: String },
    Paused { at_step: usize },
    SimulationEnded { total_steps: usize, final_time: DateTime<Utc> },
    Error(String),
}

impl SimulationController {
    pub fn send(&self, cmd: ControlCommand) -> Result<()>;
    pub fn recv(&self) -> Result<SimEvent>;     // blocking
    pub fn try_recv(&self) -> Option<SimEvent>; // non-blocking
}
```

**Usage pattern:**

```rust
let (controller, receiver) = control_channel(128);

std::thread::spawn(move || {
    runner.attach_control(receiver);
    runner.run_controlled_for(max_steps).unwrap();
});

// TUI / caller side
controller.send(ControlCommand::Pause).unwrap();
while let Some(event) = controller.try_recv() {
    match event {
        SimEvent::TickCompleted { step, sim_time, event_counts } => { /* update UI */ }
        SimEvent::Paused { .. } => { /* show paused state */ }
        SimEvent::SimulationEnded { .. } => break,
        _ => {}
    }
}
```

For domain simulations that need per-tick bookkeeping (e.g. `KitchenSimulation`), call `run_controlled_for()` on the domain wrapper rather than the raw runner — it wraps `step()` in the same control loop and emits accurate `event_counts`.

### SimulationContext

Provides time and configuration to all components.

```rust
pub struct SimulationContext {
    pub fn current_time(&self) -> DateTime<Utc>;
    pub fn step(&self) -> usize;
    pub fn config(&self) -> &SimulationConfig;
    pub fn advance_time(&mut self);
}

pub struct SimulationConfig {
    pub time_step: Duration,        // Default: 30 seconds
    pub enable_telemetry: bool,     // Default: false
}
```

---

## Demand Generation

### DemandGenerator

Trait for generating external demand.

```rust
pub trait DemandGenerator: Send + Sync {
    fn generate(&self, ctx: &SimulationContext) -> Vec<Demand>;
}
```

**Built-in Generators:**

#### PoissonDemandGenerator

Generates demands at random intervals following Poisson distribution.

```rust
pub struct PoissonDemandGenerator {
    pub fn new(demand_type: String, rate_per_hour: f64) -> Self;
}
```

**Example:**

```rust
// Generate ~10 orders per hour
let generator = PoissonDemandGenerator::new("food_order".to_string(), 10.0);
```

#### TimeOfDayDemandGenerator

Varies demand rate by time of day.

```rust
pub struct TimeOfDayDemandGenerator {
    pub fn new(demand_type: String, schedule: Vec<(u32, f64)>) -> Self;
}
```

**Example:**

```rust
// Higher demand at lunch and dinner
let generator = TimeOfDayDemandGenerator::new(
    "food_order".to_string(),
    vec![
        (11, 20.0),  // 11:00 - 20 orders/hour
        (12, 30.0),  // 12:00 - 30 orders/hour
        (18, 25.0),  // 18:00 - 25 orders/hour
        (19, 35.0),  // 19:00 - 35 orders/hour
    ]
);
```

#### SpatialDemandGenerator

Generates demands at specific locations.

```rust
pub struct SpatialDemandGenerator {
    pub fn new(demand_type: String, locations: Vec<Point<f64>>, rate_per_hour: f64) -> Self;
}
```

#### CompositeDemandGenerator

Combines multiple generators.

```rust
pub struct CompositeDemandGenerator {
    pub fn new() -> Self;
    pub fn add_generator(&mut self, generator: Box<dyn DemandGenerator>);
}
```

---

## Workflows

### WorkflowEngine

Manages workflow instances and progression.

```rust
pub struct WorkflowEngine<W: Workflow> {
    pub fn new() -> Self;
    pub fn register_workflow(&mut self, workflow: W);
    pub fn start_instance(&mut self, workflow_id: WorkflowId) -> Result<WorkflowInstance>;
    pub fn advance_instance_step(
        &mut self,
        instance_id: WorkflowInstanceId,
        current_time: DateTime<Utc>
    ) -> Result<Option<WorkflowStepEvent>>;
    pub fn mark_task_assigned(&mut self, instance_id: WorkflowInstanceId) -> Result<()>;
}
```

### WorkflowRegistry

Named workflow engines. Engines iterate in name order, so task creation and
event order are deterministic.

```rust
pub struct WorkflowRegistry<W: Workflow> {
    pub fn new() -> Self;
    pub fn register(&mut self, name: String, engine: WorkflowEngine<W>) -> Result<()>;
    pub fn get(&self, name: &str) -> Option<&WorkflowEngine<W>>;
    pub fn get_mut(&mut self, name: &str) -> Option<&mut WorkflowEngine<W>>;
    pub fn engine_for_instance_mut(&mut self, id: WorkflowInstanceId) -> Option<&mut WorkflowEngine<W>>;
    pub fn engines(&self) -> impl Iterator<Item = &WorkflowEngine<W>>;
    pub fn total_instance_count(&self) -> usize;
}
```

### TaskFactory

Creates tasks from workflow steps.

```rust
pub trait TaskFactory<T: Task>: Send + Sync {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>
    ) -> T;
}
```

---

## Snapshot Management

### SnapshotManager

Persists and restores simulation state.

```rust
pub struct SnapshotManager {
    pub async fn new(backend: SnapshotBackend) -> Result<Self>;
    pub async fn save_snapshot(&self, snapshot_id: &str, data: SnapshotData) -> Result<()>;
    pub async fn restore_snapshot(&self, snapshot_id: &str) -> Result<SnapshotData>;
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotMetadata>>;
}

pub struct SnapshotData {
    pub agents: RecordBatch,
    pub resources: RecordBatch,
    pub tasks: RecordBatch,
    pub workflow_instances: RecordBatch,
    pub locations: Option<RecordBatch>,
    pub clock: Option<SnapshotClock>,   // sim time + step
}

pub enum SnapshotBackend {
    Memory,
    Parquet(String),  // Directory path
}
```

**Example:**

```rust
// Initialize with Parquet backend
runner.set_snapshot_backend(SnapshotBackend::Parquet("./snapshots".to_string())).await?;

// Save checkpoint (agents, resources, tasks, workflow instances, clock)
runner.save_snapshot("checkpoint_100").await?;

// Restore later. The runner needs the same workflow engines/definitions
// registered; restore fails without changing anything otherwise.
runner.restore_snapshot("checkpoint_100").await?;
```

---

## Utilities

### Identifiers

Type-safe UUID-based identifiers.

```rust
pub struct AgentId(Uuid);
pub struct ResourceId(Uuid);
pub struct TaskId(Uuid);
pub struct WorkflowId(Uuid);
pub struct WorkflowInstanceId(Uuid);
pub struct LocationId(Uuid);
pub struct JourneyId(Uuid);

impl AgentId {
    pub fn new() -> Self;
    pub fn from_uuid(uuid: Uuid) -> Self;
    pub fn as_bytes(&self) -> &[u8];
}
// Same API for all ID types
```

### Error Handling

```rust
pub type Result<T> = std::result::Result<T, Error>;

pub enum Error {
    Arrow(arrow::error::ArrowError),
    DataFusion(datafusion::error::DataFusionError),
    Io(std::io::Error),
    Serialization(serde_json::Error),
    Agent(String),
    Resource(String),
    Workflow(String),
    InvalidState(String),
    Query(String),
    Custom(String),
}
```

---

## SQL Querying

The SimulationRunner provides SQL querying across all entities using DataFusion.

**Available Tables:**

- `agents` - Agent population
- `resources` - Resource state
- `tasks` - Task queue
- `workflow_instances` - Workflow execution state

**Example Queries:**

```rust
// Find busy resources
let results = runner.query("
    SELECT resource_type, COUNT(*) as busy_count
    FROM resources
    WHERE capacity - available < capacity
    GROUP BY resource_type
").await?;

// Join agents with tasks
let results = runner.query("
    SELECT a.agent_type, COUNT(t.id) as active_tasks
    FROM agents a
    LEFT JOIN tasks t ON a.id = t.agent_id
    GROUP BY a.agent_type
").await?;

// Workflow progress
let results = runner.query("
    SELECT
        workflow_id,
        AVG(current_step * 100.0 / total_steps) as avg_progress
    FROM workflow_instances
    WHERE is_complete = false
    GROUP BY workflow_id
").await?;
```

---

## Best Practices

### 1. Agent Design

- Keep agent state minimal
- Use enums for status to enable pattern matching
- Emit demands rather than creating tasks directly
- Return state changes in `AgentTickResult` for persistence

### 2. Resource Management

- Set realistic capacity limits
- Use appropriate allocation strategies
- Monitor resource utilization through queries

### 3. Workflow Design

- Break complex processes into clear steps
- Set realistic step durations
- Use metadata for step-specific configuration

### 4. Performance

- Batch operations when possible
- Use Arrow storage for large populations
- Snapshot periodically for recovery
- Query with appropriate indexes

### 5. Testing

- Test agents in isolation with mock context
- Verify resource allocation strategies
- Test workflow progression independently
- Use snapshots for deterministic testing

---

## See Also

- [sim-geo API Reference](./sim-geo.md) - Geospatial capabilities
- [sim-events API Reference](./sim-events.md) - Event infrastructure
- [sim-telemetry API Reference](./sim-telemetry.md) - Observability
- [Integration Patterns](./integration.md) - Cross-crate patterns
