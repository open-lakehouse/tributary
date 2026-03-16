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

## Demand Generation Architecture (Phase 10)

### Overview

The demand generation subsystem enables probabilistic simulation of agent creation, order arrivals, and other stochastic events using statistical distributions and configurable patterns.

### DemandGenerator Trait

**Pattern:** Pluggable demand generation strategies

```rust
pub trait DemandGenerator: Send + Sync {
    fn should_generate(&mut self, current_time: DateTime<Utc>) -> bool;
    fn generate_demands(&mut self, current_time: DateTime<Utc>) -> Vec<Demand>;
    fn reset(&mut self);
}

pub struct Demand {
    pub id: DemandId,
    pub demand_type: String,
    pub location: Option<Location>,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}
```

**Reference Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/demand/mod.rs`

---

### Demand Generator Types

#### 1. PoissonDemandGenerator

**Purpose:** Realistic random arrivals using Poisson process

**Use Cases:**
- Customer order arrivals
- Service request generation
- Any unpredictable event stream

**Implementation:**
```rust
pub struct PoissonDemandGenerator {
    base_rate: f64,  // λ (events per time unit)
    rng: StdRng,     // Seeded RNG for reproducibility
}
```

**Characteristics:**
- Inter-arrival times follow exponential distribution
- Mean arrival rate = λ (configurable)
- Statistically validated output (chi-squared test)
- Performance: 0.15ms per generation (85% better than 1ms target)

**Reference:** `/Users/robert.pack/code/management/crates/sim-core/src/demand/poisson.rs`

---

#### 2. TimeOfDayDemandGenerator

**Purpose:** Time-based demand patterns with configurable peaks

**Use Cases:**
- Restaurant order patterns (breakfast, lunch, dinner)
- Office hour modeling
- Traffic rush hour simulation

**Implementation:**
```rust
pub struct TimeOfDayDemandGenerator {
    peak_hours: Vec<PeakPeriod>,
    base_generator: PoissonDemandGenerator,
}

pub struct PeakPeriod {
    start_hour: u32,
    end_hour: u32,
    rate_multiplier: f64,  // Multiply base rate during peak
}
```

**Example Configuration:**
```rust
// Breakfast, lunch, dinner rush
vec![
    PeakPeriod { start_hour: 7, end_hour: 9, rate_multiplier: 2.0 },
    PeakPeriod { start_hour: 12, end_hour: 14, rate_multiplier: 3.0 },
    PeakPeriod { start_hour: 18, end_hour: 20, rate_multiplier: 2.5 },
]
```

**Performance:** 0.03ms per check (70% better than 0.1ms target)

**Reference:** `/Users/robert.pack/code/management/crates/sim-core/src/demand/time_of_day.rs`

---

#### 3. SpatialDemandGenerator

**Purpose:** Location-based demand with H3 spatial clustering

**Use Cases:**
- Delivery service areas (downtown vs suburbs)
- Geographic demand variation
- Urban vs suburban modeling

**Implementation:**
```rust
pub struct SpatialDemandGenerator {
    zones: HashMap<u64, ZoneDemand>,  // H3 cell -> demand params
    base_generator: PoissonDemandGenerator,
}

pub struct ZoneDemand {
    h3_cell: u64,
    density_multiplier: f64,  // Zone-specific multiplier
    capacity: Option<u32>,    // Optional capacity limit
}
```

**Optimization:** H3 cell → zone mapping pre-computed and cached

**Performance:** 0.12ms per lookup (76% better than 0.5ms target)

**Reference:** `/Users/robert.pack/code/management/crates/sim-core/src/demand/spatial.rs`

---

#### 4. CompositeDemandGenerator

**Purpose:** Combine multiple generators for complex scenarios

**Use Cases:**
- Multi-factor demand (time + location)
- A/B testing scenarios
- Complex business rules requiring multiple patterns

**Implementation:**
```rust
pub struct CompositeDemandGenerator {
    generators: Vec<Box<dyn DemandGenerator>>,
    strategy: CompositionStrategy,
}

pub enum CompositionStrategy {
    All,           // Combine outputs from all generators
    First,         // Use first generator with demands
    Weighted,      // Random selection based on weights
}
```

**Example:**
```rust
let generator = CompositeDemandGenerator::new(
    vec![
        Box::new(time_of_day_gen),  // Rush hour patterns
        Box::new(spatial_gen),       // Geographic clustering
    ],
    CompositionStrategy::All,  // Combine both
);
```

**Performance:** 0.45ms per evaluation (78% better than 2ms target)

**Reference:** `/Users/robert.pack/code/management/crates/sim-core/src/demand/composite.rs`

---

### Integration with SimulationRunner

**Pattern:** Pre-tick demand generation phase

```rust
impl SimulationRunner {
    pub fn tick(&mut self) -> Result<Vec<Event>> {
        let current_time = self.current_time();

        // Phase 1: Generate demands
        if let Some(generator) = &mut self.demand_generator {
            if generator.should_generate(current_time) {
                let demands = generator.generate_demands(current_time);
                self.process_demands(demands)?;  // Convert to workflows
            }
        }

        // Phase 2: Update agents
        // Phase 3: Allocate resources
        // Phase 4: Advance workflows
        // ... (existing tick logic)
    }

    fn process_demands(&mut self, demands: Vec<Demand>) -> Result<()> {
        for demand in demands {
            // Domain-specific: Convert demand to workflow instance
            let workflow = self.demand_to_workflow(&demand)?;
            self.workflow_registry.start_workflow(workflow)?;
        }
        Ok(())
    }
}
```

**Performance Impact:**
- Base simulation (Phase 9): 0.06ms/tick
- With demand generation: 0.81ms/tick (+0.75ms overhead)
- **Still 123x better than 100ms target** ✅

---

## Multi-Domain Architecture (Phase 11)

### Overview

Phase 11 proved the framework's generality by implementing a complete second domain (logistics) with **ZERO framework modifications**. This validates that the core abstractions are truly generic and ready for diverse domains.

### Framework Generality Proof

**Framework Crates Modified:** 0 out of 4
- ❌ sim-core - No changes
- ❌ sim-geo - No changes
- ❌ sim-events - No changes
- ❌ sim-telemetry - No changes

**All Implementation in Domain Crate:**
- ✅ sim-logistics - New domain crate (14 files, 3,539 lines)

---

### Domain Architecture Pattern

**Each domain follows this structure:**

```
crates/sim-{domain}/
├── src/
│   ├── agents/           Domain-specific agent implementations
│   │   ├── mod.rs
│   │   ├── agent_type_1.rs
│   │   └── agent_type_2.rs
│   │
│   ├── resources/        Domain-specific resource implementations
│   │   ├── mod.rs
│   │   ├── resource_type_1.rs
│   │   └── resource_type_2.rs
│   │
│   ├── workflows/        Domain-specific workflow definitions
│   │   ├── mod.rs
│   │   ├── workflow_1.rs
│   │   └── task_factory.rs
│   │
│   ├── locations/        Domain-specific location types (optional)
│   │   ├── mod.rs
│   │   └── location_type.rs
│   │
│   └── simulation/       SimulationRunner integration
│       ├── mod.rs
│       └── runner.rs
│
└── tests/                Integration tests
    └── integration_test.rs
```

---

### Cross-Domain Comparison

| Aspect | Kitchen Domain | Logistics Domain | Pattern Reused? |
|--------|---------------|------------------|-----------------|
| **Agents** | Customer, Courier, SupplyDriver | Package, Driver | ✅ Agent trait |
| **States** | Idle/Ordering/Eating (Customer) | Created/Sorting/Delivered (Package) | ✅ State machines |
| **Resources** | Stove, Table | SortingStation, LoadingDock | ✅ Resource trait |
| **Workflows** | FoodPrep, Courier, SupplyChain | PackageShipment | ✅ Workflow trait |
| **Locations** | Kitchen, DistributionSite | Warehouse | ✅ H3 spatial indexing |
| **Movement** | Couriers at 12 m/s | Drivers at variable speed | ✅ VelocityModel |
| **Events** | OrderCreated, InventoryLow | PackageReceived, DriverAssigned | ✅ CloudEvents |
| **Telemetry** | OTLP logs, traces, metrics | OTLP logs, traces, metrics | ✅ TelemetryGenerator |

**Key Finding:** All framework patterns successfully reused across domains with zero modifications.

---

### Multi-Domain Architecture Diagram (Text-Based)

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Framework Layer                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │  sim-core    │  │  sim-geo     │  │  sim-events  │              │
│  │              │  │              │  │              │              │
│  │ - Runner     │  │ - Location   │  │ - CloudEvents│              │
│  │ - Workflows  │  │ - Movement   │  │ - Dispatcher │              │
│  │ - Resources  │  │ - Routing    │  │ - Storage    │              │
│  │ - Population │  │ - Spatial    │  │              │              │
│  │ - Demand     │  │              │  │              │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│                                                                       │
│  ┌──────────────┐                                                    │
│  │sim-telemetry │                                                    │
│  │              │                                                    │
│  │ - Generators │                                                    │
│  │ - OTLP       │                                                    │
│  └──────────────┘                                                    │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │ Trait Implementations
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Domain Layer                                  │
│                                                                       │
│  ┌─────────────────────┐              ┌─────────────────────┐       │
│  │   sim-kitchen       │              │   sim-logistics     │       │
│  │                     │              │                     │       │
│  │ Agents:             │              │ Agents:             │       │
│  │ - Customer          │              │ - Package           │       │
│  │ - Courier           │              │ - Driver            │       │
│  │ - SupplyDriver      │              │                     │       │
│  │                     │              │ Resources:          │       │
│  │ Resources:          │              │ - SortingStation    │       │
│  │ - Stove             │              │ - LoadingDock       │       │
│  │ - Table             │              │                     │       │
│  │ - Inventory         │              │ Workflows:          │       │
│  │                     │              │ - PackageShipment   │       │
│  │ Workflows:          │              │                     │       │
│  │ - FoodPrep          │              │ Locations:          │       │
│  │ - CourierDelivery   │              │ - Warehouse         │       │
│  │ - SupplyChain       │              │                     │       │
│  │                     │              │                     │       │
│  │ Locations:          │              │                     │       │
│  │ - DistributionSite  │              │                     │       │
│  └─────────────────────┘              └─────────────────────┘       │
│                                                                       │
│           ┌─────────────────────┐                                    │
│           │   sim-{future}      │    Future domains:                │
│           │                     │    - Healthcare                   │
│           │   (Zero framework   │    - Manufacturing                │
│           │    changes needed)  │    - Retail                       │
│           │                     │    - Transportation               │
│           └─────────────────────┘    - ANY agent-based domain       │
└─────────────────────────────────────────────────────────────────────┘
```

---

### Multi-Workflow Coordination Pattern

**Architecture:** WorkflowRegistry orchestrates concurrent workflows

```rust
pub struct WorkflowRegistry {
    engines: HashMap<String, WorkflowEngine>,  // workflow_type -> engine
}

impl WorkflowRegistry {
    /// Tick all workflows (independent state isolation)
    pub fn tick_all(&mut self, time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        let mut all_events = Vec::new();

        for (name, engine) in &mut self.engines {
            let events = engine.tick(time)?;
            all_events.extend(events);
        }

        Ok(all_events)
    }
}
```

**Benefits:**
- Concurrent workflow execution
- Independent state isolation (no cross-contamination)
- Easy addition of new workflow types
- Centralized workflow coordination

**Performance:** O(1) HashMap lookups, <0.02ms overhead for 3 workflow types

**Reference:** `/Users/robert.pack/code/management/crates/sim-core/src/workflows/registry.rs`

---

### Cross-Domain Integration Patterns

#### Pattern 1: Shared Resources Across Domains

**Scenario:** Kitchen and logistics sharing delivery drivers

```rust
// Both domains use the same Resource trait
impl Resource for DeliveryDriver {
    fn can_accept_task(&self, task: &Task) -> bool {
        match task.task_type.as_str() {
            "food_delivery" => self.has_insulated_bag,     // Kitchen domain
            "package_delivery" => self.has_cargo_space,   // Logistics domain
            _ => false,
        }
    }
}
```

#### Pattern 2: Cross-Domain Events

**Scenario:** Logistics package arrival triggers kitchen supply replenishment

```rust
// Domain-agnostic event handling
pub fn on_event(&mut self, event: &Event) -> Result<()> {
    match event.event_type.as_str() {
        "logistics.package.delivered" => {
            if event.metadata.get("destination") == Some("kitchen") {
                self.replenish_inventory(event)?;  // Kitchen domain action
            }
        },
        _ => {},
    }
    Ok(())
}
```

#### Pattern 3: Multi-Domain Simulation

**Scenario:** Running kitchen + logistics in single simulation

```rust
let mut runner = SimulationRunner::new();

// Register workflows from multiple domains
runner.workflow_registry.register("food_prep", kitchen_workflow_engine)?;
runner.workflow_registry.register("courier_delivery", kitchen_delivery_engine)?;
runner.workflow_registry.register("package_shipment", logistics_workflow_engine)?;

// Tick advances all domains simultaneously
runner.tick()?;  // Updates kitchen AND logistics state
```

---

## Performance Characteristics by Phase

### Cumulative Performance Impact

| Phase | Optimization | Baseline | After | Improvement | Cumulative |
|-------|--------------|----------|-------|-------------|------------|
| 1-2 | Stabilization | 150ms | 150ms | 1x | 1x |
| 3 | Batch operations | 150ms | 50ms | 3x | 3x |
| 6 | Arrow migration | 50ms | 5ms | 10x | 30x |
| 7 | Cached routing | 5ms | 0.5ms | 10x | 300x |
| 9 | Multi-workflow | 0.5ms | 0.06ms | 8.3x | **2,500x** |
| 10 | Demand generation | 0.06ms | 0.81ms | 0.93x* | 185x* |

*Phase 10 adds new capability (demand generation), slight overhead expected but still 185x better than baseline

### Performance by Operation Type

| Operation Type | Performance | Notes |
|---------------|-------------|-------|
| **Agent Updates** | <0.01ms per agent | State machine efficiency |
| **Workflow Advancement** | <0.02ms per instance | O(1) HashMap lookups |
| **Resource Allocation** | <0.01ms per task | Minimal matching overhead |
| **Spatial Queries** | <0.5ms per query | H3 indexing, cached |
| **Routing Queries** | 330ns per query | Cached Contraction Hierarchies |
| **Batch Updates** | 18µs for 1,000 | Single RecordBatch rebuild |
| **Snapshot Save** | 15ms for 1,500 agents | Parquet compression |
| **SQL Queries** | 0.8ms per aggregation | DataFusion optimization |
| **Demand Generation** | 0.75ms per tick | All generators combined |
| **Telemetry** | <1% overhead | Negligible impact |

---

## Future Architecture Considerations

### Planned Features (Phases 12-14)

1. **Python Bindings** (Phase 12) - PyO3 bindings for Python integration
2. **CLI Tool** (Phase 13) - Command-line interface for running simulations
3. **Comprehensive Benchmarks** (Phase 14) - Expanded benchmark suite for regression detection
4. **Production Deployment Guides** - Cloud deployment templates and best practices

### Post-1.0 Considerations

#### 1. Distributed Simulation (Multi-Node)

**Challenge:** Partition simulation across multiple machines

**Approaches:**
- Spatial partitioning (geographic regions)
- Agent-based partitioning (agent ID ranges)
- Hybrid approach with cross-node coordination

**Framework Impact:** Moderate (networking layer, state synchronization)

---

#### 2. GPU Acceleration

**Challenge:** Leverage GPU for parallel agent updates

**Approaches:**
- CUDA kernels for batch operations
- GPU-accelerated spatial indexing
- Parallel routing queries

**Framework Impact:** Significant (data layout, memory management)

**Priority:** P3 (optimize CPU usage first, current performance excellent)

---

#### 3. Real-Time Visualization Dashboard

**Challenge:** Live monitoring of simulation state

**Approaches:**
- WebSocket event streaming
- React/Vue.js dashboard
- D3.js for real-time charts

**Framework Impact:** Low (event infrastructure already exists)

**Priority:** P2 (useful for debugging and demonstrations)

---

#### 4. Machine Learning Integration

**Challenge:** RL agents learning optimal policies

**Approaches:**
- OpenAI Gym interface
- PyTorch/TensorFlow integration
- Reward signal extraction from events

**Framework Impact:** Low (agents implement custom logic)

**Priority:** P2 (research use case)

---

#### 5. 10,000 Agent Scale

**Current:** 0.06ms/tick for 1,500 agents (linear scaling → 0.4ms for 10K)
**Target:** <100ms per tick
**Headroom:** 250x margin ✅

**Optimizations if needed:**
- Parallel agent updates with rayon
- SIMD for batch operations
- Lock-free data structures

**Priority:** P2 (significant headroom exists)

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
