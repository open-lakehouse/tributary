# Phase 6 Week 2: Snapshots & Persistence - Implementation Plan

**Status**: Planning
**Estimated Duration**: 2-3 days
**Dependencies**: Phase 6 Week 1 Complete ✅, Phase 6 Week 1 Refinement Complete ✅

---

## Executive Summary

Implement snapshot save/restore functionality using DataFusion TableProvider architecture to enable simulation persistence and resumption. This builds on the Arrow RecordBatch foundation from Week 1 and leverages proven patterns from the universe crate.

### Key Objectives
1. Save simulation state to Parquet files via DataFusion
2. Restore simulation state from Parquet files
3. Support both MemTable (testing) and ListingTable (production) backends
4. Round-trip verification (save → restore → equality)
5. Version compatibility and metadata tracking

---

## Architecture Overview

### Design Philosophy: Centralized SnapshotManager

**Chosen Approach**: Single SnapshotManager module that handles all I/O
- Handlers remain lightweight (no SessionContext added to PopulationHandler)
- I/O logic concentrated in one place
- Easy to test and maintain
- Clear separation of concerns

**Alternative Rejected**: SessionContext per handler
- Adds weight to every handler instance
- Duplicates DataFusion setup
- More complex testing
- Less clear ownership of I/O

### Reference Implementation

The **universe crate** provides a mature, production-ready snapshot system we can adapt:

**Key Files** (for reference):
- `crates/universe/src/context/snapshots.rs` - Snapshot creation/restoration
- `crates/universe/src/context/storage.rs` - Parquet persistence (ListingTable)
- `crates/universe/src/context/memory.rs` - In-memory testing (MemTable)

**Pattern to Adapt**:
```rust
// Universe pattern (simplified)
pub async fn create_snapshot_from_batches(
    ctx: &SimulationContext,
    agents: &[RecordBatch],
    resources: &[RecordBatch],
    workflows: &[RecordBatch],
) -> Result<SnapshotId> {
    let df = ctx.create_dataframe(agents)?;
    df.write_parquet(&path, None, None).await?;
    // ... similar for other tables
}
```

---

## Implementation Plan

### Phase 1: Handler Export/Import Methods (3-4 hours)

Add batch export/import methods to each handler to expose RecordBatch data.

#### PopulationHandler
**File**: `crates/sim-core/src/population/handler.rs`

```rust
impl<A: Agent> PopulationHandler<A> {
    /// Export all agents as RecordBatch for persistence
    pub fn export_batch(&self) -> RecordBatch {
        self.agents.clone()
    }

    /// Import agents from RecordBatch (rebuilds index)
    pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()> {
        // Validate schema matches expected
        Self::validate_agent_schema(&batch)?;

        // Rebuild id_to_row index
        self.id_to_row.clear();
        for row_idx in 0..batch.num_rows() {
            let row = ArrowAgentRow::from_batch(&batch, row_idx)?;
            let id = AgentId::from_bytes(&row.id)?;
            self.id_to_row.insert(id, row_idx);
        }

        self.agents = batch;
        self.next_row = self.agents.num_rows();
        Ok(())
    }
}
```

#### WorkflowEngine & ResourceAllocator
Similar pattern for:
- `WorkflowEngine::export_instances_batch()` / `import_instances_batch()`
- `ResourceAllocator::export_resources_batch()` / `import_resources_batch()`
- `ResourceAllocator::export_tasks_batch()` / `import_tasks_batch()`

**Note**: WorkflowEngine and ResourceAllocator already have SessionContext fields! They can be reused.

---

### Phase 2: SnapshotManager Module (4-5 hours)

Create centralized snapshot management with TableProvider abstraction.

#### File Structure
```
crates/sim-core/src/
├── snapshot/
│   ├── mod.rs           (public API)
│   ├── manager.rs       (SnapshotManager struct)
│   ├── metadata.rs      (SnapshotMetadata struct)
│   └── provider.rs      (TableProvider trait abstraction)
```

#### Core Implementation
**File**: `crates/sim-core/src/snapshot/manager.rs`

```rust
use datafusion::prelude::*;
use datafusion::datasource::{MemTable, TableProvider, listing::ListingTable};

pub struct SnapshotManager {
    backend: SnapshotBackend,
    base_path: Option<PathBuf>,
}

pub enum SnapshotBackend {
    Memory,      // MemTable for testing
    Parquet,     // ListingTable for production
}

impl SnapshotManager {
    /// Create in-memory snapshot manager (for tests)
    pub fn memory() -> Self {
        Self {
            backend: SnapshotBackend::Memory,
            base_path: None,
        }
    }

    /// Create Parquet-backed snapshot manager
    pub fn parquet(base_path: PathBuf) -> Self {
        Self {
            backend: SnapshotBackend::Parquet,
            base_path: Some(base_path),
        }
    }

    /// Save complete simulation snapshot
    pub async fn save_snapshot(
        &self,
        snapshot_id: &str,
        agents: RecordBatch,
        resources: RecordBatch,
        tasks: RecordBatch,
        workflow_instances: RecordBatch,
        metadata: SnapshotMetadata,
    ) -> Result<()> {
        match self.backend {
            SnapshotBackend::Memory => {
                self.save_to_memory(snapshot_id, agents, resources, tasks, workflow_instances, metadata).await
            }
            SnapshotBackend::Parquet => {
                self.save_to_parquet(snapshot_id, agents, resources, tasks, workflow_instances, metadata).await
            }
        }
    }

    /// Restore simulation snapshot
    pub async fn restore_snapshot(
        &self,
        snapshot_id: &str,
    ) -> Result<SnapshotData> {
        match self.backend {
            SnapshotBackend::Memory => self.restore_from_memory(snapshot_id).await,
            SnapshotBackend::Parquet => self.restore_from_parquet(snapshot_id).await,
        }
    }

    // Private implementation methods
    async fn save_to_parquet(
        &self,
        snapshot_id: &str,
        agents: RecordBatch,
        resources: RecordBatch,
        tasks: RecordBatch,
        workflow_instances: RecordBatch,
        metadata: SnapshotMetadata,
    ) -> Result<()> {
        let base = self.base_path.as_ref()
            .ok_or_else(|| Error::Snapshot("No base path for Parquet backend".into()))?;

        let snapshot_dir = base.join(snapshot_id);
        std::fs::create_dir_all(&snapshot_dir)?;

        // Create DataFusion context
        let ctx = SessionContext::new();

        // Save each table to Parquet
        let agents_df = ctx.read_batch(agents)?;
        agents_df.write_parquet(
            snapshot_dir.join("agents.parquet").to_str().unwrap(),
            DataFrameWriteOptions::new(),
            None,
        ).await?;

        // ... similar for resources, tasks, workflow_instances

        // Save metadata as JSON
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        std::fs::write(snapshot_dir.join("metadata.json"), metadata_json)?;

        Ok(())
    }

    async fn restore_from_parquet(&self, snapshot_id: &str) -> Result<SnapshotData> {
        let base = self.base_path.as_ref()
            .ok_or_else(|| Error::Snapshot("No base path for Parquet backend".into()))?;

        let snapshot_dir = base.join(snapshot_id);

        // Read metadata
        let metadata_json = std::fs::read_to_string(snapshot_dir.join("metadata.json"))?;
        let metadata: SnapshotMetadata = serde_json::from_str(&metadata_json)?;

        // Create DataFusion context
        let ctx = SessionContext::new();

        // Read each Parquet file
        let agents_df = ctx.read_parquet(
            snapshot_dir.join("agents.parquet").to_str().unwrap(),
            ParquetReadOptions::default(),
        ).await?;
        let agents = agents_df.collect().await?;

        // ... similar for other tables

        Ok(SnapshotData {
            metadata,
            agents: agents.into_iter().next().unwrap_or_else(|| RecordBatch::new_empty(/* schema */)),
            resources: /* ... */,
            tasks: /* ... */,
            workflow_instances: /* ... */,
        })
    }

    async fn save_to_memory(/* ... */) -> Result<()> {
        // Store in static HashMap for testing
        // Implementation similar to universe/src/context/memory.rs
    }

    async fn restore_from_memory(/* ... */) -> Result<SnapshotData> {
        // Retrieve from static HashMap
    }
}

pub struct SnapshotData {
    pub metadata: SnapshotMetadata,
    pub agents: RecordBatch,
    pub resources: RecordBatch,
    pub tasks: RecordBatch,
    pub workflow_instances: RecordBatch,
}
```

#### Metadata Structure
**File**: `crates/sim-core/src/snapshot/metadata.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub snapshot_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub simulation_step: u64,
    pub simulation_time: f64,
    pub version: String,  // Schema version for compatibility
    pub agent_count: usize,
    pub resource_count: usize,
    pub task_count: usize,
    pub workflow_instance_count: usize,
}

impl SnapshotMetadata {
    pub fn new(snapshot_id: String, simulation_step: u64, simulation_time: f64) -> Self {
        Self {
            snapshot_id,
            created_at: chrono::Utc::now(),
            simulation_step,
            simulation_time,
            version: "1.0.0".to_string(),
            agent_count: 0,
            resource_count: 0,
            task_count: 0,
            workflow_instance_count: 0,
        }
    }
}
```

---

### Phase 3: Integration with Runner (1-2 hours)

Add snapshot save/restore to simulation runner.

**File**: `crates/sim-core/src/runner/mod.rs`

```rust
use crate::snapshot::{SnapshotManager, SnapshotMetadata};

impl SimulationRunner {
    /// Save current simulation state to snapshot
    pub async fn save_snapshot(&self, snapshot_id: &str) -> Result<()> {
        let manager = SnapshotManager::parquet(PathBuf::from("./snapshots"));

        let metadata = SnapshotMetadata::new(
            snapshot_id.to_string(),
            self.current_step,
            self.current_time,
        );

        manager.save_snapshot(
            snapshot_id,
            self.population.export_batch(),
            self.resources.export_resources_batch(),
            self.resources.export_tasks_batch(),
            self.workflows.export_instances_batch(),
            metadata,
        ).await
    }

    /// Restore simulation state from snapshot
    pub async fn restore_snapshot(&mut self, snapshot_id: &str) -> Result<()> {
        let manager = SnapshotManager::parquet(PathBuf::from("./snapshots"));

        let data = manager.restore_snapshot(snapshot_id).await?;

        // Restore handlers
        self.population.import_batch(data.agents)?;
        self.resources.import_resources_batch(data.resources)?;
        self.resources.import_tasks_batch(data.tasks)?;
        self.workflows.import_instances_batch(data.workflow_instances)?;

        // Restore simulation state
        self.current_step = data.metadata.simulation_step;
        self.current_time = data.metadata.simulation_time;

        Ok(())
    }
}
```

---

### Phase 4: Testing (2-3 hours)

Comprehensive round-trip tests.

**File**: `crates/sim-core/tests/snapshot_tests.rs`

```rust
use sim_core::snapshot::{SnapshotManager, SnapshotMetadata};

#[tokio::test]
async fn test_memory_backend_roundtrip() {
    let manager = SnapshotManager::memory();

    // Create test data
    let mut population = PopulationHandler::new();
    let agents = create_test_agents(100);
    population.add_agents(agents.clone()).unwrap();

    let metadata = SnapshotMetadata::new("test".to_string(), 42, 100.0);

    // Save
    manager.save_snapshot(
        "test",
        population.export_batch(),
        RecordBatch::new_empty(resource_schema()),
        RecordBatch::new_empty(task_schema()),
        RecordBatch::new_empty(workflow_schema()),
        metadata.clone(),
    ).await.unwrap();

    // Restore
    let data = manager.restore_snapshot("test").await.unwrap();

    // Verify
    assert_eq!(data.metadata.simulation_step, 42);
    assert_eq!(data.agents.num_rows(), 100);

    // Verify agent data integrity
    let mut restored = PopulationHandler::new();
    restored.import_batch(data.agents).unwrap();

    for agent in agents {
        let restored_agent = restored.get_agent(&agent.id()).unwrap();
        assert_eq!(agent.id(), restored_agent.id());
    }
}

#[tokio::test]
async fn test_parquet_backend_roundtrip() {
    use tempfile::tempdir;

    let temp_dir = tempdir().unwrap();
    let manager = SnapshotManager::parquet(temp_dir.path().to_path_buf());

    // Similar test to memory backend
    // Verify Parquet files created
    // Verify round-trip equality
}

#[tokio::test]
async fn test_large_simulation_snapshot() {
    // Test with 10k agents
    // Verify save completes in <1 second
    // Verify restore completes in <2 seconds
}

#[tokio::test]
async fn test_version_compatibility() {
    // Test that metadata version is checked
    // Test graceful handling of version mismatches
}
```

---

### Phase 5: Documentation (1 hour)

**File**: `migration/PHASE_6_WEEK_2_COMPLETE.md`

Document:
- Architecture decisions (why centralized SnapshotManager)
- Usage examples (save/restore)
- Performance characteristics
- Testing strategy
- Migration guide for users

---

## File Changes Summary

### New Files (5)
```
crates/sim-core/src/snapshot/mod.rs              (~50 lines)
crates/sim-core/src/snapshot/manager.rs          (~200 lines)
crates/sim-core/src/snapshot/metadata.rs         (~50 lines)
crates/sim-core/tests/snapshot_tests.rs          (~300 lines)
migration/PHASE_6_WEEK_2_COMPLETE.md             (~400 lines)
```

### Modified Files (5)
```
crates/sim-core/src/lib.rs                       (add snapshot module)
crates/sim-core/src/population/handler.rs        (add export/import)
crates/sim-core/src/resources/allocator.rs       (add export/import)
crates/sim-core/src/workflows/engine.rs          (add export/import)
crates/sim-core/src/runner/mod.rs                (add save/restore)
```

**Total**: ~1,000 lines of new code

---

## Dependencies

All required dependencies are **already present** in `Cargo.toml`:
- ✅ `datafusion` - TableProvider, SessionContext, Parquet I/O
- ✅ `parquet` - Parquet format support
- ✅ `arrow` - RecordBatch foundation
- ✅ `serde_json` - Metadata serialization
- ✅ `chrono` - Timestamps

**No new dependencies needed!**

---

## Success Criteria

### Performance
- [ ] Save 10k agents snapshot in <1 second
- [ ] Restore 10k agents snapshot in <2 seconds
- [ ] Parquet compression reduces file size by 50%+

### Correctness
- [ ] Round-trip equality verified (save → restore → compare)
- [ ] All handler indices correctly rebuilt on restore
- [ ] Metadata accurately captures simulation state

### Quality
- [ ] 100% test coverage for snapshot module
- [ ] Both MemTable and Parquet backends tested
- [ ] Error handling robust (no panics)
- [ ] Documentation complete

### Integration
- [ ] Works with existing sim-kitchen domain
- [ ] Compatible with Phase 6 Week 1 refinements
- [ ] No breaking changes to handler APIs

---

## Risk Assessment

**Overall Risk**: ✅ LOW

**Risks**:
1. **Schema version compatibility**: MITIGATED by metadata version field
2. **Large file I/O performance**: MITIGATED by Parquet compression
3. **Index rebuild complexity**: MITIGATED by existing patterns in handlers

**Dependencies**:
- ✅ Phase 6 Week 1 complete (RecordBatch foundation)
- ✅ Phase 6 Week 1 Refinement complete (batch operations, error handling)
- ✅ DataFusion already in dependencies
- ✅ Universe crate provides reference implementation

---

## Implementation Timeline

**Total Estimate**: 11-14 hours (2 days focused work)

| Phase | Task | Hours |
|-------|------|-------|
| 1 | Handler export/import methods | 3-4 |
| 2 | SnapshotManager module | 4-5 |
| 3 | Runner integration | 1-2 |
| 4 | Testing | 2-3 |
| 5 | Documentation | 1 |
| **Buffer** | Unexpected issues | +50% |
| **Total** | | **16.5-21 hours** |

---

## Next Steps After Week 2

With snapshot/restore complete, the following become possible:

### Phase 7: Road Network Integration
- Load snapshots with spatial data
- Resume simulations with routing state

### Phase 8: Telemetry Generation
- Snapshot at intervals for time-series analysis
- Export metrics from snapshots

### Phase 9: Multi-Workflow Patterns
- Save/restore complex workflow states
- Resume long-running simulations

---

## References

### Code References (Universe Crate)
- Snapshot creation: `crates/universe/src/context/snapshots.rs`
- Parquet persistence: `crates/universe/src/context/storage.rs`
- Memory backend: `crates/universe/src/context/memory.rs`

### Documentation
- Phase 6 Week 1 Complete: `migration/PHASE_6_WEEK_1_COMPLETE.md`
- Phase 6 Week 1 Refinement: `migration/PHASE_6_WEEK_1_REFINEMENT.md`
- DataFusion guide: https://arrow.apache.org/datafusion/

---

**Status**: Ready for implementation
**Approval**: Awaiting user confirmation to proceed
