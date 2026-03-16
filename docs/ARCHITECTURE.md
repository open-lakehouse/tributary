# Generic Agent Simulation Framework - Architecture

**Last Updated:** March 16, 2026

---

## Overview

The Generic Agent Simulation Framework is a Rust-based system for building large-scale agent-based simulations with focus on performance, maintainability, and domain flexibility.

---

## Core Design Decisions

### 1. Arrow RecordBatch + HashMap Dual Index

**Pattern:** Store all state in Arrow RecordBatch with HashMap index for fast lookup

**Rationale:**
- RecordBatch provides columnar storage (10-100x faster than row-oriented)
- HashMap index enables O(1) lookups by entity ID
- Enables DataFusion SQL queries on simulation state
- Natural serialization to Parquet for snapshots

**Implementation:**
```rust
pub struct WorkflowEngine {
    instances: RecordBatch,  // Columnar storage
    index: HashMap<WorkflowInstanceId, usize>,  // Fast lookup
}
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/workflows/engine.rs`
- `/Users/robert.pack/code/management/crates/sim-geo/src/locations/handler.rs`
- `/Users/robert.pack/code/management/crates/sim-core/src/resources/allocator.rs`

---

### 2. TaskFactory Pattern

**Pattern:** Domain-specific task creation from generic workflow steps

**Rationale:**
- Workflows define abstract business processes
- TaskFactory converts steps into domain-specific tasks
- Enables workflow reuse across different domains
- Decouples workflow engine from domain logic

**Implementation:**
```rust
pub trait TaskFactory: Send + Sync {
    fn create_task(
        &self,
        instance: &WorkflowInstance,
        step: &WorkflowStep,
        ctx: &SimulationContext,
    ) -> Result<Task>;
}
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/workflows/task_factory.rs`
- `/Users/robert.pack/code/management/crates/sim-kitchen/src/workflows.rs` (example)

---

### 3. Demand-Driven Workflow Allocation

**Pattern:** Workflows create tasks → allocator matches to resources

**Rationale:**
- Separates "what needs to be done" from "who does it"
- Resource allocation becomes generic matching problem
- Supports dynamic resource availability
- Handles capacity constraints naturally

**Flow:**
```
WorkflowEngine → TaskFactory → Task → ResourceAllocator → Resource
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/workflows/engine.rs`
- `/Users/robert.pack/code/management/crates/sim-core/src/resources/allocator.rs`

---

### 4. Snapshot Manager Dual Backend

**Pattern:** Memory backend for speed, Parquet backend for persistence

**Rationale:**
- Memory backend: zero-copy snapshots for testing/debugging
- Parquet backend: compressed columnar storage for large simulations
- Both backends implement same interface
- Transparent backend selection based on use case

**Implementation:**
```rust
pub enum SnapshotBackend {
    Memory,
    Parquet { path: PathBuf },
}

pub struct SnapshotManager {
    backend: SnapshotBackend,
    // ...
}
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/snapshots/mod.rs`
- `/Users/robert.pack/code/management/crates/sim-core/src/snapshots/backend.rs`

---

## Framework Architecture

### Crate Organization

```
crates/
├── sim-core/          Core simulation engine
│   ├── runner/        Tick loop coordination
│   ├── workflows/     Multi-step business processes
│   ├── resources/     Task-resource allocation
│   └── snapshots/     State save/restore
│
├── sim-geo/           Geospatial operations
│   ├── locations/     Point location tracking
│   ├── movement/      Journey and velocity models
│   ├── spatial/       H3 spatial indexing
│   └── graph/         Road network routing (Phase 7)
│
├── sim-events/        Event infrastructure
│   ├── events/        CloudEvents 1.0 types
│   ├── dispatcher/    Event buffering and emission
│   └── storage/       Parquet event storage
│
├── sim-telemetry/     Observability (Phase 8)
│   ├── generators/    Log/trace/metric generation
│   └── formats/       OTLP, JSON, Prometheus export
│
└── sim-kitchen/       Reference implementation
    ├── agents/        Customer and kitchen agents
    ├── workflows/     Food preparation workflow
    └── simulation.rs  Domain-specific hooks
```

---

## Integration Patterns

### 1. SimulationRunner Hooks

**Pattern:** Domain-specific logic injected via trait

```rust
pub trait SimulationHooks {
    fn on_tick_start(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_tick_end(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_resources_updated(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_workflows_advanced(&mut self, ctx: &SimulationContext) -> Result<()>;
}
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/runner/mod.rs`
- `/Users/robert.pack/code/management/crates/sim-kitchen/src/simulation.rs`

---

### 2. Event-Driven Architecture

**Pattern:** CloudEvents 1.0 for all simulation events

**Rationale:**
- Standard format enables interoperability
- Event metadata supports filtering/routing
- Enables replay and time-travel debugging
- Natural fit for telemetry generation

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-events/src/events/mod.rs`
- `/Users/robert.pack/code/management/crates/sim-events/src/dispatcher/mod.rs`

---

### 3. VelocityModel Abstraction

**Pattern:** Pluggable velocity calculation for movement

```rust
pub trait VelocityModel: Send + Sync {
    fn calculate_velocity(
        &self,
        ctx: &SimulationContext,
        location: Point<f64>,
    ) -> f64;  // meters per second
}
```

**Implementations:**
- `ConstantVelocity` - Fixed speed
- `TimeOfDayVelocity` - Rush hour patterns (Phase 7)

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-geo/src/movement/velocity.rs`
- `/Users/robert.pack/code/management/crates/sim-geo/src/movement/time_of_day.rs`

---

## Performance Considerations

### 1. Batch Operations

**Strategy:** Batch RecordBatch updates to avoid O(N²) rebuilds

**Impact:** 7-785x speedup for workflow updates

**Reference:**
- `/Users/robert.pack/code/management/crates/sim-core/benches/batch_operations.rs`
- See benchmark results in Phase 3 completion doc

---

### 2. FastGraph Caching

**Strategy:** Cache Contraction Hierarchies preprocessing with OnceLock

**Impact:** Routing queries reduced from 1.86ms to 330ns (5645x speedup)

**Reference:**
- `/Users/robert.pack/code/management/crates/sim-geo/src/graph/routing.rs:136`
- See P0 fixes documentation

---

### 3. H3 Spatial Indexing

**Strategy:** Use Uber H3 hexagonal hierarchical indexing

**Impact:** Sub-millisecond proximity queries for large agent populations

**Reference:**
- `/Users/robert.pack/code/management/crates/sim-geo/src/spatial/mod.rs`

---

### 4. Arrow Columnar Storage

**Strategy:** Store all state in Arrow RecordBatch

**Impact:**
- 10-100x faster than HashMap for batch operations
- Enables SQL queries via DataFusion
- Natural Parquet serialization
- Memory efficient for large simulations

**Reference:**
- All handler implementations use RecordBatch
- See Phase 6 Week 2 completion for migration details

---

## Extension Points

### Adding a New Domain

1. Create domain crate (e.g., `sim-logistics/`)
2. Implement domain-specific agents
3. Create TaskFactory for workflow → task conversion
4. Implement SimulationHooks for lifecycle management
5. Define domain events extending base CloudEvents

**Example:** See `sim-kitchen/` crate structure

---

### Adding a New Workflow

1. Define WorkflowDefinition with steps
2. Implement TaskFactory to convert steps → domain tasks
3. Register workflow with WorkflowEngine
4. Create instances via `engine.start_workflow()`

**Example:**
- `/Users/robert.pack/code/management/crates/sim-kitchen/src/workflows.rs`
- FoodPrepWorkflow implementation

---

### Adding a New Resource Type

1. Create struct implementing resource fields
2. Add via `allocator.add_resource()`
3. Define matching logic in allocator
4. Handle capacity constraints

**Example:**
- `/Users/robert.pack/code/management/crates/sim-kitchen/src/resources.rs`
- Kitchen resource implementation

---

## Testing Strategy

### 1. Unit Tests

- Test individual components in isolation
- Mock dependencies where needed
- Target: >85% coverage per crate

### 2. Integration Tests

- Test cross-component interactions
- Validate workflow → task → resource flow
- Test event emission sequence
- Test snapshot save/restore roundtrip

### 3. Benchmarks

- Criterion benchmarks for critical paths
- Compare against performance targets
- Catch regressions in CI

**Reference:**
- `/Users/robert.pack/code/management/crates/sim-core/benches/`
- Phase 3 completion doc for benchmark results

---

## Future Architecture

### Planned Features (Phases 9-14)

1. **Multi-Workflow Support** - Multiple workflow types running concurrently
2. **Additional Domains** - Logistics, delivery, supply chain examples
3. **Python Bindings** - PyO3 bindings for Python integration
4. **CLI Tool** - Command-line interface for running simulations
5. **Performance Optimization** - Target: 10k agents @ <100ms per tick

---

## References

### Key Implementation Files

- **Core Engine:** `/Users/robert.pack/code/management/crates/sim-core/src/runner/mod.rs`
- **Workflows:** `/Users/robert.pack/code/management/crates/sim-core/src/workflows/engine.rs`
- **Resources:** `/Users/robert.pack/code/management/crates/sim-core/src/resources/allocator.rs`
- **Snapshots:** `/Users/robert.pack/code/management/crates/sim-core/src/snapshots/mod.rs`
- **Movement:** `/Users/robert.pack/code/management/crates/sim-geo/src/movement/handler.rs`
- **Routing:** `/Users/robert.pack/code/management/crates/sim-geo/src/graph/routing.rs`
- **Events:** `/Users/robert.pack/code/management/crates/sim-events/src/dispatcher/mod.rs`

### Documentation

- **Phase Plans:** `/Users/robert.pack/code/management/migration/phases/`
- **Completed Work:** `/Users/robert.pack/code/management/migration/completed/`
- **Main README:** `/Users/robert.pack/code/management/migration/README.md`

---

## Contributing

When extending the framework:

1. Follow existing patterns (RecordBatch + HashMap, TaskFactory, etc.)
2. Write tests alongside implementation (TDD approach)
3. Add benchmarks for performance-critical paths
4. Document all public APIs with examples
5. Maintain zero clippy warnings on library code

---

**Questions?** See `/Users/robert.pack/code/management/migration/README.md` for navigation guidance.
