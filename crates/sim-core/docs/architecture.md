# sim-core Architecture

## Design Principles

### 1. Domain Agnostic
The framework makes **zero assumptions** about your domain. Whether modeling logistics, kitchens, retail, manufacturing, or services, the core abstractions remain the same.

### 2. Trait-Based Extensibility
All major components are traits, not concrete types. Implement `Agent`, `Resource`, `Task`, and `Workflow` for your specific domain.

### 3. Composition Over Inheritance
Build complex behavior by composing simple components. The `SimulationRunner` orchestrates independent components (population, resources, workflows, hooks) without coupling.

### 4. Zero-Cost Abstractions
Generic traits compile to monomorphic code. No runtime vtable lookups or boxing except where explicitly chosen (e.g., `Box<dyn SimulationHooks>`).

### 5. Arrow-Native Data Model
All batch data uses Apache Arrow RecordBatch for:
- Efficient columnar storage
- Interoperability with DataFusion, Polars, DuckDB
- Zero-copy slicing and filtering
- Natural path to Parquet persistence

## Module Organization

```
sim-core/
├── context/          Time management and configuration
│   ├── config.rs     SimulationConfig with custom fields
│   └── context.rs    SimulationContext (time, step, elapsed)
│
├── demand/           Demand generation patterns
│   ├── generator.rs  DemandGenerator trait
│   ├── poisson.rs    Poisson process generator
│   ├── spatial.rs    Location-based demand
│   ├── temporal.rs   Time-of-day patterns
│   └── composite.rs  Combine multiple generators
│
├── population/       Agent lifecycle and management
│   ├── agent.rs      Agent trait definition
│   ├── handler.rs    PopulationHandler
│   └── status.rs     AgentStatus trait
│
├── resources/        Resource allocation
│   ├── resource.rs   Resource trait
│   ├── task.rs       Task trait
│   ├── allocator.rs  ResourceAllocator
│   └── strategies/   Allocation strategies
│       ├── round_robin.rs
│       ├── least_loaded.rs
│       └── random.rs
│
├── workflows/        Multi-stage pipelines
│   ├── workflow.rs   Workflow trait
│   ├── step.rs       WorkflowStep definition
│   ├── engine.rs     WorkflowEngine (one per workflow family)
│   └── registry.rs   WorkflowRegistry (the runner's only workflow path)
│
├── runner/           Simulation orchestration
│   ├── runner.rs     SimulationRunner
│   ├── hooks.rs      SimulationHooks trait
│   └── stats.rs      SimulationStats
│
├── snapshot/         State persistence
│   ├── manager.rs    SnapshotManager
│   ├── backend.rs    SnapshotBackend trait
│   └── metadata.rs   SnapshotMetadata
│
└── utils/            Utilities and helpers
    └── idents.rs     Type-safe UUID wrappers
```

## Key Abstractions

### Agent
An **autonomous entity** that exists in the simulation and exhibits behavior.

**Responsibilities**:
- Maintain internal state
- Respond to simulation time progression (`tick`)
- Emit demands (resource requests)
- Emit journeys (movement requests)
- Report state changes

**Design Decision**: Agents own their behavior logic. The framework only provides the execution context.

**Trade-off**: Maximum flexibility at the cost of requiring explicit state management.

### Resource
A **capacity-constrained processor** that handles tasks.

**Responsibilities**:
- Define capacity (max concurrent tasks)
- Track current load
- Provide resource type identifier

**Design Decision**: Resources are passive. The `ResourceAllocator` handles assignment logic.

**Trade-off**: Separation of concerns enables reusable allocation strategies but requires external coordination.

### Task
A **unit of work** to be processed by a resource.

**Responsibilities**:
- Specify required resource type
- Provide estimated duration
- Carry task-specific metadata

**Design Decision**: Tasks don't know which resource processes them. Assignment is external.

**Trade-off**: Decoupling enables flexible routing but requires additional orchestration.

### Workflow
A **multi-stage processing pipeline** with defined steps.

**Responsibilities**:
- Define ordered processing steps
- Specify resource requirements per step
- Provide duration estimates per step

**Design Decision**: Workflows are declarative. Engines in a `WorkflowRegistry` hold instances; the runner advances them as tasks complete.

**Trade-off**: Declarative workflows are easier to reason about but less flexible than imperative code.

## Data Flow

### Typical Simulation Step

```
1. hooks.on_before_tick(ctx)
2. DemandGenerator.generate(ctx)           → hooks.on_demands_generated
3. PopulationHandler.tick_agents(ctx)      → hooks.on_agents_ticked
4. ResourceAllocator.get_completed_tasks   → hooks.on_tasks_completed
   (workflow tasks advance their instance  → hooks.on_workflow_step_completed)
5. TaskFactory creates tasks for workflow instances ready for their next step
6. ResourceAllocator.assign_tasks(strategy) → hooks.on_resources_allocated
7. SimulationContext.advance()             → hooks.on_after_tick
```

See `SimulationRunner::tick` for the authoritative description.

### AgentTickResult Flow

```rust
pub struct AgentTickResult {
    pub agent_id: AgentId,
    pub demands: Vec<Demand>,     // Resource requests
    pub journeys: Vec<Journey>,   // Movement requests
    pub state_change: Option<serde_json::Value>,  // Arbitrary metadata
}
```

Agents return `AgentTickResult` from their `tick()` method. The `SimulationRunner` aggregates these results and passes them to hooks for processing.

**Why this design?**
- Agents remain decoupled from framework internals
- Results are batch-processed for efficiency
- Hooks can implement domain-specific handling

## Allocation Strategies

### RoundRobin
Distributes tasks evenly across resources in order.

**Use case**: Fair distribution, no hotspots
**Complexity**: O(1) per assignment
**Trade-off**: Ignores resource load

### LeastLoaded
Assigns tasks to the resource with lowest current load.

**Use case**: Load balancing, maximize throughput
**Complexity**: O(n) per assignment (n = resource count)
**Trade-off**: Can create temporary hotspots during burst arrivals

### Random
Assigns tasks randomly.

**Use case**: Simple baseline, avoid coordinated patterns
**Complexity**: O(1) per assignment
**Trade-off**: No optimization, purely stochastic

### Custom Strategies
Implement `AllocationStrategy<R, T>` for domain-specific logic:

```rust
pub trait AllocationStrategy<R: Resource, T: Task> {
    fn assign(&self, task: &T, available: &[&R]) -> Option<ResourceId>;
}
```

## Simulation Runner Design

### Component Architecture

The `SimulationRunner` uses **optional composition**:

```rust
pub struct SimulationRunner<A: Agent, R: Resource, T: Task, W: Workflow> {
    context: SimulationContext,
    population: Option<PopulationHandler<A>>,
    resources: Option<ResourceAllocator<R, T>>,
    workflow_registry: Option<WorkflowRegistry<W>>,
    task_factory: Option<Box<dyn TaskFactory<T>>>,
    allocation_strategy: Box<dyn AllocationStrategy>,   // LeastLoaded by default
    demand_generator: Option<Box<dyn DemandGenerator>>,
    hooks: Box<dyn SimulationHooks>,                    // NoOpHooks by default
    snapshot_manager: Option<SnapshotManager>,
    // stats, control channel, pause state
}
```

**Why optional?**
- Not all simulations need all components
- Example: Pure agent simulation doesn't need resources
- Example: Simple scenarios don't need workflows

**Design Decision**: Linear execution order with hook injection points.

**Trade-off**: Predictable flow but requires careful hook implementation to avoid inconsistencies.

## Type-Safe Identifiers

All entity IDs use strongly-typed UUID wrappers:

```rust
pub struct AgentId(Uuid);
pub struct ResourceId(Uuid);
pub struct TaskId(Uuid);
pub struct LocationId(Uuid);
pub struct WorkflowId(Uuid);
```

**Benefits**:
1. **Type safety**: Can't accidentally use `AgentId` where `ResourceId` is expected
2. **Time-ordered** (UUID v7): Better indexing, natural sorting
3. **Deterministic** (UUID v5): Reproducible IDs for testing
4. **Efficient**: 128-bit integers, cheap to copy and hash

**Design Decision**: Separate types instead of `type AgentId = Uuid`.

**Trade-off**: Slight boilerplate (derive Debug, Clone, etc.) for strong compile-time guarantees.

## Arrow RecordBatch Integration

### Why Arrow?

1. **Columnar format**: Cache-friendly, SIMD-friendly
2. **Zero-copy**: Slicing without allocation
3. **Interoperability**: Works with DataFusion, Polars, DuckDB
4. **Standards-based**: IPC format for cross-language support

### Usage Pattern

```rust
// In SnapshotManager
pub fn save_snapshot(&self, data: SnapshotData) -> Result<PathBuf> {
    let batch = data.to_record_batch()?;  // Convert domain data to Arrow
    let writer = ArrowWriter::try_new(file, batch.schema(), None)?;
    writer.write(&batch)?;
    Ok(path)
}

pub fn load_snapshot(&self, path: &Path) -> Result<SnapshotData> {
    let reader = ParquetRecordBatchReader::try_new(file, batch_size)?;
    let batch = reader.next().unwrap()?;  // Read Arrow data
    SnapshotData::from_record_batch(batch)  // Convert back to domain data
}
```

**Design Decision**: Arrow as the serialization format, not the in-memory representation.

**Trade-off**: Conversion overhead at snapshot boundaries vs runtime flexibility.

## Hook System

### SimulationHooks Trait

```rust
pub trait SimulationHooks: Send {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment],
    ) -> Result<()> {
        Ok(())
    }

    fn on_workflows_advanced(
        &mut self,
        ctx: &SimulationContext,
        completions: &[WorkflowCompletion],
    ) -> Result<()> {
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }
}
```

**Default implementations**: Empty (no-op). Override only what you need.

**Design Decision**: Single trait with default methods vs separate optional hooks.

**Trade-off**: Convenience of defaults vs clarity of required implementations.

### Hook Patterns

**Pattern 1: Event Emission**
```rust
impl SimulationHooks for EventHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        for result in results {
            for demand in &result.demands {
                let event = Event::new(Box::new(DemandCreated { ... }), ctx.current_time());
                self.dispatcher.emit(event);
            }
        }
        Ok(())
    }
}
```

**Pattern 2: State Tracking**
```rust
impl SimulationHooks for MetricsHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        self.tick_count += 1;
        self.tick_duration_ms.push(ctx.elapsed().num_milliseconds());
        Ok(())
    }
}
```

**Pattern 3: Conditional Actions**
```rust
impl SimulationHooks for SnapshotHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        if ctx.step() % 1000 == 0 {
            self.snapshot_manager.save(ctx)?;
        }
        Ok(())
    }
}
```

## Design Decisions Summary

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| Trait-based abstractions | Maximum flexibility, domain agnostic | Requires implementing traits |
| Optional components | Not all simulations need all features | More nil checks in code |
| Arrow for persistence | Industry-standard, efficient, interoperable | Conversion overhead |
| Type-safe IDs | Compile-time safety | Slight boilerplate |
| Hook system | Clean extension points without subclassing | Requires understanding hook order |
| Separate allocator | Reusable allocation strategies | External coordination needed |
| Tick-based execution | Simple mental model, deterministic | Not real-time, requires time step tuning |

## Extension Points

### 1. Custom Agents
Implement `Agent` trait with domain-specific behavior.

### 2. Custom Resources
Implement `Resource` trait with capacity logic.

### 3. Custom Tasks
Implement `Task` trait with domain-specific metadata.

### 4. Custom Allocation Strategies
Implement `AllocationStrategy<R, T>` for routing logic.

### 5. Custom Hooks
Implement `SimulationHooks` for lifecycle integration.

### 6. Custom Demand Generators
Implement `DemandGenerator` for arrival patterns.

### 7. Custom Snapshot Backends
Implement `SnapshotBackend` for storage systems.

## Performance Considerations

### Batch Processing
Process agents in bulk rather than one-at-a-time:
```rust
let results: Vec<AgentTickResult> = population.tick_agents(ctx)?;
// Batch process all results
```

### Allocation Efficiency
- RoundRobin: O(1) - fastest
- LeastLoaded: O(n) - acceptable for <1000 resources
- Custom: Profile your implementation

### Memory Management
- Agents: ~200 bytes per agent (domain-dependent)
- Tasks: ~100 bytes per task
- Events: ~500 bytes per event (buffered before flush)

### Scaling Guidelines
- Up to 10,000 agents: No special considerations
- 10,000 - 100,000 agents: Consider batch optimizations
- 100,000+ agents: Consider parallel tick processing (future work)
