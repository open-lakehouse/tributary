# Phase 6: Arrow RecordBatch Migration & Snapshots

**Status**: NOT STARTED
**Priority**: 🔴 CRITICAL PATH
**Estimated Effort**: 3 weeks
**Dependencies**: None (can start immediately, parallel to Phase 5)
**Blocks**: Phase 14 (Performance Benchmarks), Large-scale simulations

---

## Objectives

1. Replace HashMap storage with Arrow RecordBatch in all handlers
2. Enable DataFusion queries over simulation state
3. Implement snapshot save/restore system
4. Achieve 10-100x performance improvement for large populations
5. Unlock columnar analytics during simulation

---

## Prerequisites Checklist

- [x] Arrow/DataFusion dependencies in Cargo.toml
- [x] Handler abstractions exist (PopulationHandler, ResourceAllocator, etc.)
- [x] Universe crate has reference implementation (`universe/src/builders/state_population.rs`)
- [x] Basic understanding of Arrow schemas and RecordBatch

---

## Current State Analysis

### Storage Pattern (Current)
```rust
// crates/sim-core/src/population/handler.rs:15
pub struct PopulationHandler<A: Agent> {
    agents: HashMap<AgentId, A>,  // ← HashMap storage
    _marker: PhantomData<A>,
}
```

### Storage Pattern (Target)
```rust
pub struct PopulationHandler<A: Agent> {
    agents: RecordBatch,              // ← Arrow columnar storage
    schema: SchemaRef,
    id_to_row: HashMap<AgentId, usize>, // Index for fast lookups
    _marker: PhantomData<A>,
}
```

### Performance Impact
- **Current**: O(n) iteration for queries, scattered memory
- **Target**: O(n/vectorization_factor) with SIMD, columnar compression
- **Expected Speedup**: 10-100x for bulk operations

---

## Parallelization Strategy

This phase has **excellent** parallelization potential:

### Parallel Track A: PopulationHandler Migration (Agent 1)
- Task 6.1: Define Arrow schema for agents
- Task 6.2: Implement RecordBatch storage
- Task 6.3: Add DataFusion query support
- **Files**: `crates/sim-core/src/population/*`

### Parallel Track B: ResourceAllocator Migration (Agent 2)
- Task 6.4: Define Arrow schema for resources/tasks
- Task 6.5: Implement RecordBatch storage
- Task 6.6: Add allocation queries via DataFusion
- **Files**: `crates/sim-core/src/resources/*`

### Parallel Track C: WorkflowEngine Migration (Agent 3)
- Task 6.7: Define Arrow schema for workflow instances
- Task 6.8: Implement RecordBatch storage
- Task 6.9: Add instance queries
- **Files**: `crates/sim-core/src/workflows/*`

### Parallel Track D: LocationHandler Migration (Agent 4)
- Task 6.10: Define GeoArrow schema for locations
- Task 6.11: Implement RecordBatch with geometry column
- Task 6.12: Update spatial queries
- **Files**: `crates/sim-geo/src/locations/*`

### Sequential Track (After A-D Complete): Snapshot System (Agent 1)
- Task 6.13: Implement snapshot save
- Task 6.14: Implement snapshot restore
- Task 6.15: Add Delta Lake support (optional)
- **Files**: `crates/sim-core/src/runner/snapshots.rs` (new)

**Coordination**: Minimal dependencies, can work in parallel. Sync point before snapshots.

---

## Tasks

### Task 6.1: Define Arrow Schema for Agents (Track A)

**File**: `crates/sim-core/src/population/schema.rs` (new file)

**Implementation**:
```rust
use arrow::datatypes::{Schema, Field, DataType};
use std::sync::Arc;

/// Create Arrow schema for agent population
pub fn agent_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),  // UUID as bytes
        Field::new("agent_type", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("position_x", DataType::Float64, true),  // Optional
        Field::new("position_y", DataType::Float64, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("metadata", DataType::Utf8, true),  // JSON serialized
    ]))
}
```

**Reference**: See `universe/src/builders/state_population.rs:74` for pattern.

**Acceptance Criteria**:
- [ ] Schema defined with all Agent trait fields
- [ ] Schema supports optional fields (position)
- [ ] Schema includes metadata for domain-specific fields
- [ ] Documentation explains schema design

---

### Task 6.2: Implement RecordBatch Storage (Track A)

**File**: `crates/sim-core/src/population/handler.rs`

**Migration Steps**:

1. **Update Struct**:
```rust
pub struct PopulationHandler<A: Agent> {
    agents: RecordBatch,
    schema: SchemaRef,
    id_to_row: HashMap<AgentId, usize>,
    next_row: usize,
    _marker: PhantomData<A>,
}
```

2. **Update spawn() Method**:
```rust
pub fn spawn(&mut self, new_agents: Vec<A>) -> Result<()> {
    // Build arrays for each column
    let mut id_builder = BinaryBuilder::new();
    let mut type_builder = StringBuilder::new();
    let mut status_builder = StringBuilder::new();
    let mut pos_x_builder = Float64Builder::new();
    let mut pos_y_builder = Float64Builder::new();
    // ... other builders

    for agent in &new_agents {
        id_builder.append_value(agent.id().as_bytes());
        type_builder.append_value(agent.agent_type());
        status_builder.append_value(agent.status().as_str());

        if let Some(pos) = agent.position() {
            pos_x_builder.append_value(pos.x());
            pos_y_builder.append_value(pos.y());
        } else {
            pos_x_builder.append_null();
            pos_y_builder.append_null();
        }
    }

    // Build arrays
    let id_array = Arc::new(id_builder.finish());
    let type_array = Arc::new(type_builder.finish());
    // ... finish other builders

    // Create new batch
    let new_batch = RecordBatch::try_new(
        self.schema.clone(),
        vec![id_array, type_array, /* ... */]
    )?;

    // Append to existing batch (or replace if first)
    self.agents = concat_batches(&self.schema, &[&self.agents, &new_batch])?;

    // Update index
    for (i, agent) in new_agents.iter().enumerate() {
        self.id_to_row.insert(agent.id(), self.next_row + i);
    }
    self.next_row += new_agents.len();

    Ok(())
}
```

3. **Update get() Method**:
```rust
pub fn get(&self, id: &AgentId) -> Option<A> {
    let row_idx = self.id_to_row.get(id)?;

    // Extract values from RecordBatch at row_idx
    let id_array = self.agents.column(0).as_any().downcast_ref::<BinaryArray>()?;
    let type_array = self.agents.column(1).as_any().downcast_ref::<StringArray>()?;
    // ... other columns

    // Reconstruct agent from row data
    // This requires Agent trait to have a from_arrow_row() method
    // See Task 6.3 for trait update

    Some(A::from_arrow_row(*row_idx, &self.agents)?)
}
```

4. **Update tick_agents() Method**:
```rust
pub fn tick_agents(&mut self, ctx: &SimulationContext) -> Result<Vec<AgentTickResult>> {
    let mut results = Vec::new();

    // Convert RecordBatch to agents (batch operation)
    let agents: Vec<A> = self.batch_to_agents()?;

    for agent in agents {
        results.push(agent.tick(ctx)?);
    }

    // Apply updates back to RecordBatch
    self.apply_agent_updates(&agents)?;

    Ok(results)
}
```

**Acceptance Criteria**:
- [ ] PopulationHandler uses RecordBatch storage
- [ ] spawn(), get(), tick_agents() methods updated
- [ ] id_to_row index maintained for fast lookups
- [ ] All unit tests pass
- [ ] No performance regression on small populations (<1000 agents)

---

### Task 6.3: Add Agent Trait Methods for Arrow Serialization (Track A)

**File**: `crates/sim-core/src/population/traits.rs`

**Add Methods to Agent Trait**:
```rust
pub trait Agent: Send + Sync {
    // ... existing methods ...

    /// Serialize agent to Arrow row (returns values for each column)
    fn to_arrow_row(&self) -> ArrowRow {
        ArrowRow {
            id: self.id().as_bytes().to_vec(),
            agent_type: self.agent_type().to_string(),
            status: self.status().as_str().to_string(),
            position_x: self.position().map(|p| p.x()),
            position_y: self.position().map(|p| p.y()),
            metadata: self.serialize_metadata(),
        }
    }

    /// Deserialize agent from Arrow row
    fn from_arrow_row(row_idx: usize, batch: &RecordBatch) -> Result<Self>
    where
        Self: Sized;

    /// Serialize domain-specific metadata to JSON
    fn serialize_metadata(&self) -> String {
        "{}".to_string()  // Default: empty JSON
    }
}

pub struct ArrowRow {
    pub id: Vec<u8>,
    pub agent_type: String,
    pub status: String,
    pub position_x: Option<f64>,
    pub position_y: Option<f64>,
    pub metadata: String,
}
```

**Acceptance Criteria**:
- [ ] Agent trait has to_arrow_row() method
- [ ] Agent trait has from_arrow_row() method
- [ ] Default implementations provided
- [ ] Documentation explains metadata serialization
- [ ] Backward compatible (default impls)

---

### Task 6.4-6.6: ResourceAllocator Migration (Track B)

**Files**:
- `crates/sim-core/src/resources/schema.rs` (new)
- `crates/sim-core/src/resources/allocator.rs`

**Pattern**: Follow same pattern as PopulationHandler (Tasks 6.1-6.3)

**Schema Design**:
```rust
// Resources schema
pub fn resource_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),
        Field::new("resource_type", DataType::Utf8, false),
        Field::new("capacity", DataType::UInt32, false),
        Field::new("current_load", DataType::UInt32, false),
        Field::new("metadata", DataType::Utf8, true),
    ]))
}

// Tasks schema
pub fn task_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),
        Field::new("required_resource_type", DataType::Utf8, false),
        Field::new("workflow_instance_id", DataType::Binary, true),
        Field::new("estimated_duration_secs", DataType::UInt64, false),
        Field::new("allocated_resource_id", DataType::Binary, true),
        Field::new("allocated_at", DataType::Timestamp(TimeUnit::Microsecond, None), true),
        Field::new("metadata", DataType::Utf8, true),
    ]))
}
```

**Key Methods to Update**:
- `add_resource()` - Append to resources RecordBatch
- `add_task()` - Append to tasks RecordBatch
- `allocate_tasks()` - Use DataFusion for matching query
- `get_completed_tasks()` - Query tasks where allocated_at + duration <= now

**Acceptance Criteria**:
- [ ] ResourceAllocator uses RecordBatch for resources and tasks
- [ ] Allocation queries use DataFusion
- [ ] All methods updated
- [ ] Unit tests pass

---

### Task 6.7-6.9: WorkflowEngine Migration (Track C)

**Files**:
- `crates/sim-core/src/workflows/schema.rs` (new)
- `crates/sim-core/src/workflows/engine.rs`

**Schema Design**:
```rust
pub fn workflow_instance_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("instance_id", DataType::Binary, false),
        Field::new("workflow_id", DataType::Binary, false),
        Field::new("current_step", DataType::UInt32, false),
        Field::new("status", DataType::Utf8, false),  // "active", "waiting", "completed"
        Field::new("start_time", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("end_time", DataType::Timestamp(TimeUnit::Microsecond, None), true),
        Field::new("metadata", DataType::Utf8, true),
    ]))
}
```

**Key Methods to Update**:
- `start_instance()` - Add row to instances RecordBatch
- `advance_instance_step()` - Update current_step and status columns
- `get_ready_instances()` - Query WHERE status = 'waiting_for_resource'

**Acceptance Criteria**:
- [ ] WorkflowEngine uses RecordBatch storage
- [ ] Instance queries use DataFusion
- [ ] All methods updated
- [ ] Unit tests pass

---

### Task 6.10-6.12: LocationHandler Migration (Track D)

**Files**:
- `crates/sim-geo/src/locations/schema.rs` (new)
- `crates/sim-geo/src/locations/handler.rs`

**Schema Design (GeoArrow)**:
```rust
use geoarrow::GeometryType;

pub fn location_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Binary, false),
        Field::new("location_type", DataType::Utf8, false),
        Field::new("geometry", GeometryType::Point.to_data_type(), false),  // GeoArrow point
        Field::new("h3_cell", DataType::UInt64, false),  // H3 cell at resolution 9
        Field::new("capacity", DataType::UInt32, true),
        Field::new("metadata", DataType::Utf8, true),
    ]))
}
```

**Key Methods to Update**:
- `add_location()` - Append with GeoArrow geometry
- `nearest()` - Spatial query using H3 index + geometry distance
- `within_radius()` - Filter by H3 cells then distance

**Acceptance Criteria**:
- [ ] LocationHandler uses RecordBatch with GeoArrow
- [ ] Geometry column properly encoded
- [ ] Spatial queries work
- [ ] H3 index still used for acceleration
- [ ] Unit tests pass

---

### Task 6.13: Implement Snapshot Save System

**File**: `crates/sim-core/src/runner/snapshots.rs` (new file)

**Implementation**:
```rust
use parquet::file::writer::SerializedFileWriter;
use std::path::Path;

impl<A, R, T, W> SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    /// Save current simulation state to disk
    pub fn save_snapshot(&self, path: &Path) -> Result<()> {
        // Create snapshot directory
        std::fs::create_dir_all(path)?;

        // Save each component as Parquet
        if let Some(population) = &self.population {
            let agents_path = path.join("agents.parquet");
            write_parquet(&agents_path, population.as_record_batch())?;
        }

        if let Some(resources) = &self.resources {
            let resources_path = path.join("resources.parquet");
            write_parquet(&resources_path, resources.resources_batch())?;

            let tasks_path = path.join("tasks.parquet");
            write_parquet(&tasks_path, resources.tasks_batch())?;
        }

        if let Some(workflows) = &self.workflows {
            let instances_path = path.join("workflow_instances.parquet");
            write_parquet(&instances_path, workflows.instances_batch())?;
        }

        if let Some(locations) = &self.locations {
            let locations_path = path.join("locations.parquet");
            write_parquet(&locations_path, locations.as_record_batch())?;
        }

        if let Some(movement) = &self.movement {
            let journeys_path = path.join("journeys.parquet");
            write_parquet(&journeys_path, movement.journeys_batch())?;
        }

        // Save context (time, config, etc.)
        let context_path = path.join("context.json");
        std::fs::write(context_path, serde_json::to_string_pretty(&self.context)?)?;

        // Save metadata
        let meta = SnapshotMetadata {
            timestamp: chrono::Utc::now(),
            simulation_time: self.context.current_time(),
            step_count: self.stats.total_steps,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        let meta_path = path.join("metadata.json");
        std::fs::write(meta_path, serde_json::to_string_pretty(&meta)?)?;

        Ok(())
    }
}

fn write_parquet(path: &Path, batch: &RecordBatch) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut writer = SerializedFileWriter::new(
        file,
        batch.schema(),
        Default::default(),
    )?;
    writer.write(batch)?;
    writer.close()?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct SnapshotMetadata {
    timestamp: DateTime<Utc>,
    simulation_time: DateTime<Utc>,
    step_count: usize,
    version: String,
}
```

**Acceptance Criteria**:
- [ ] save_snapshot() method saves all components
- [ ] Each component saved as separate Parquet file
- [ ] Metadata file with timestamp and version
- [ ] Context saved as JSON
- [ ] Directory structure well-organized

---

### Task 6.14: Implement Snapshot Restore System

**File**: `crates/sim-core/src/runner/snapshots.rs`

**Implementation**:
```rust
impl<A, R, T, W> SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    /// Restore simulation from snapshot
    pub fn restore_from_snapshot(path: &Path) -> Result<Self> {
        // Load metadata
        let meta_path = path.join("metadata.json");
        let meta: SnapshotMetadata = serde_json::from_str(&std::fs::read_to_string(meta_path)?)?;

        // Load context
        let context_path = path.join("context.json");
        let context: SimulationContext = serde_json::from_str(&std::fs::read_to_string(context_path)?)?;

        // Load each component from Parquet
        let population = if path.join("agents.parquet").exists() {
            let batch = read_parquet(&path.join("agents.parquet"))?;
            Some(PopulationHandler::from_record_batch(batch)?)
        } else {
            None
        };

        let resources = if path.join("resources.parquet").exists() {
            let resources_batch = read_parquet(&path.join("resources.parquet"))?;
            let tasks_batch = read_parquet(&path.join("tasks.parquet"))?;
            Some(ResourceAllocator::from_record_batches(resources_batch, tasks_batch)?)
        } else {
            None
        };

        // ... load other components ...

        Ok(Self {
            context,
            population,
            resources,
            workflows,
            locations,
            movement,
            events: None,  // Events not restored (append-only log)
            hooks: None,   // Hooks must be re-registered
            stats: SimulationStats {
                total_steps: meta.step_count,
                ..Default::default()
            },
        })
    }
}

fn read_parquet(path: &Path) -> Result<RecordBatch> {
    let file = std::fs::File::open(path)?;
    let reader = SerializedFileReader::new(file)?;
    let mut arrow_reader = ParquetFileArrowReader::new(Arc::new(reader));
    let batch_reader = arrow_reader.get_record_reader(1024)?;

    // Read all batches and concatenate
    let batches: Vec<RecordBatch> = batch_reader.collect::<Result<Vec<_>, _>>()?;
    let schema = batches[0].schema();
    concat_batches(&schema, &batches.iter().collect::<Vec<_>>())
}
```

**Acceptance Criteria**:
- [ ] restore_from_snapshot() loads all components
- [ ] RecordBatches reconstructed from Parquet
- [ ] Handlers initialized from batches
- [ ] Simulation time restored
- [ ] Integration test: save → restore → verify state identical

---

### Task 6.15: Add Handler Methods for Batch Export (All Tracks)

Each handler needs methods to export/import RecordBatches:

**PopulationHandler**:
```rust
impl<A: Agent> PopulationHandler<A> {
    pub fn as_record_batch(&self) -> &RecordBatch {
        &self.agents
    }

    pub fn from_record_batch(batch: RecordBatch) -> Result<Self> {
        // Reconstruct id_to_row index
        let id_array = batch.column(0).as_any().downcast_ref::<BinaryArray>()
            .ok_or_else(|| anyhow!("Invalid schema"))?;

        let mut id_to_row = HashMap::new();
        for i in 0..batch.num_rows() {
            let id_bytes = id_array.value(i);
            let id = AgentId::from_bytes(id_bytes)?;
            id_to_row.insert(id, i);
        }

        Ok(Self {
            agents: batch.clone(),
            schema: batch.schema(),
            id_to_row,
            next_row: batch.num_rows(),
            _marker: PhantomData,
        })
    }
}
```

**Similar for**: ResourceAllocator, WorkflowEngine, LocationHandler

**Acceptance Criteria**:
- [ ] All handlers have as_record_batch() method
- [ ] All handlers have from_record_batch() method
- [ ] Indexes reconstructed correctly
- [ ] Round-trip test: export → import → verify equality

---

## Deliverables

### Code Changes
- [ ] PopulationHandler migrated to RecordBatch
- [ ] ResourceAllocator migrated to RecordBatch
- [ ] WorkflowEngine migrated to RecordBatch
- [ ] LocationHandler migrated to GeoArrow
- [ ] Agent trait has Arrow serialization methods
- [ ] Resource, Task, Workflow traits have Arrow serialization
- [ ] Snapshot save/restore system complete
- [ ] DataFusion queries enabled in allocators

### Tests
- [ ] Unit tests for each handler migration (20+ tests)
- [ ] Round-trip tests for serialization (5 tests)
- [ ] Snapshot save/restore integration test (1 test)
- [ ] Performance benchmark comparing HashMap vs Arrow (1 benchmark)
- [ ] All existing tests pass

### Documentation
- [ ] Arrow schema design documented
- [ ] Snapshot format documented (directory structure, file formats)
- [ ] Migration guide for domain developers
- [ ] Performance comparison documented

---

## Testing Strategy

### Unit Tests Per Handler
```bash
cargo test --lib population::handler::tests
cargo test --lib resources::allocator::tests
cargo test --lib workflows::engine::tests
cargo test --lib locations::handler::tests
```

### Integration Test
```bash
cargo test --test snapshot_integration_test
```

### Performance Benchmark
```bash
cargo bench --bench arrow_vs_hashmap
```

**Expected Results**:
- 10x speedup for 10k agents
- 50x speedup for 100k agents
- Linear memory scaling

---

## Acceptance Criteria (Phase Complete)

- [ ] **All Handlers Migrated**: RecordBatch storage in all 4 handlers
- [ ] **Snapshot System Works**: Can save and restore complete simulation state
- [ ] **Performance Improved**: 10x+ speedup measured on benchmarks
- [ ] **DataFusion Enabled**: Can run SQL queries on simulation state
- [ ] **Tests Pass**: 100+ total tests passing
- [ ] **No Regressions**: Existing functionality preserved
- [ ] **Documentation Complete**: Migration guide and schema docs written

---

## Performance Targets

| Operation | Before (HashMap) | After (Arrow) | Speedup |
|-----------|------------------|---------------|---------|
| 10k agent tick | ~1000ms | <100ms | 10x |
| 100k agent spawn | ~500ms | <50ms | 10x |
| Query by type (10k) | ~50ms | <5ms | 10x |
| Memory (100k agents) | ~500MB | ~50MB | 10x |

---

## Coordination Points (Parallel Execution)

### Week 1-2: Independent Work
- **Track A** (Agent 1): PopulationHandler migration
- **Track B** (Agent 2): ResourceAllocator migration
- **Track C** (Agent 3): WorkflowEngine migration
- **Track D** (Agent 4): LocationHandler migration

**No conflicts expected** - different files

### Week 3: Integration
- **All Agents**: Sync up, merge branches
- **Agent 1**: Implement snapshot system (uses all migrated handlers)
- **All Agents**: Run integration tests

### Week 3 End: Validation
- **All Agents**: Performance benchmarks, documentation

---

**Estimated Effort**: 3 weeks (1 week parallel, 1 week integration, 1 week polish)
**Can Start**: Immediately
**Blocks**: Performance optimization, large-scale sims
