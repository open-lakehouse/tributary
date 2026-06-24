# Snapshots

**Last Updated:** March 16, 2026

## Overview

The snapshot system provides state persistence for Tributary simulations. It supports save/restore of complete simulation state using Arrow RecordBatch storage with Parquet compression, enabling long-running simulations, checkpointing, and A/B testing.

## Objective

Learn how to:
- Save and restore simulation state
- Use Parquet backend for persistent storage
- Query snapshot data with DataFusion SQL
- Implement state branching for experiments
- Optimize snapshot performance

## Prerequisites

- Understanding of [Agents](agents.md) and [Resources](resources.md)
- [Getting Started](../GETTING_STARTED.md)
- Basic knowledge of Apache Arrow and Parquet

## SnapshotManager

The SnapshotManager coordinates snapshot operations:

```rust
use sim_core::snapshot::{SnapshotManager, SnapshotBackend, SnapshotData};

// Create manager with Parquet backend
let manager = SnapshotManager::new(
    SnapshotBackend::Parquet("./snapshots".to_string())
).await?;

// Save snapshot
let data = SnapshotData {
    agents: population.export_batch(),
    resources: allocator.export_resources(),
    tasks: allocator.export_tasks(),
    workflow_instances: engine.export_instances(),
    locations: location_handler.export_batch(),
};

manager.save_snapshot("checkpoint_100", data).await?;

// Restore snapshot
let restored = manager.restore_snapshot("checkpoint_100").await?;

// Import into handlers
population.import_batch(restored.agents)?;
allocator.import_batch(restored.resources, restored.tasks)?;
engine.import_instances(restored.workflow_instances)?;
```

## Snapshot Backends

### Memory Backend

For testing and short-term storage:

```rust
let manager = SnapshotManager::new(SnapshotBackend::Memory).await?;

// Data stored in-memory (ephemeral)
// Fast but not persistent across runs
```

### Parquet Backend

For production and persistent storage:

```rust
let manager = SnapshotManager::new(
    SnapshotBackend::Parquet("./snapshots".to_string())
).await?;

// Data written to Parquet files on disk
// 9x compression with Snappy codec
// SQL queryable with DataFusion
```

## Snapshot Structure

### Directory Layout

```
snapshots/
├── checkpoint_100/
│   ├── agents.parquet
│   ├── resources.parquet
│   ├── tasks.parquet
│   ├── workflow_instances.parquet
│   ├── locations.parquet
│   └── metadata.json
├── checkpoint_200/
│   └── ...
└── experiment_a/
    └── ...
```

### Metadata File

```json
{
  "version": "1.0",
  "timestamp": "2026-03-16T10:00:00Z",
  "agent_count": 1500,
  "resource_count": 50,
  "task_count": 120,
  "workflow_instance_count": 85,
  "location_count": 25
}
```

## SnapshotData Structure

```rust
pub struct SnapshotData {
    pub agents: RecordBatch,
    pub resources: RecordBatch,
    pub tasks: RecordBatch,
    pub workflow_instances: RecordBatch,
    pub locations: RecordBatch,
}
```

Each RecordBatch contains:
- **agents**: id, agent_type, status, position_x, position_y, metadata
- **resources**: id, resource_type, capacity, current_load, metadata
- **tasks**: id, task_type, required_resource_type, estimated_duration, metadata
- **workflow_instances**: instance_id, workflow_type, current_step, status, metadata
- **locations**: id, location_type, position_x, position_y, h3_cell, metadata

## Integration with SimulationRunner

### Periodic Checkpointing

```rust
impl SimulationHooks for CheckpointHooks {
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Checkpoint every 100 steps
        if ctx.step() % 100 == 0 {
            let snapshot_id = format!("checkpoint_{}", ctx.step());

            let data = SnapshotData {
                agents: self.population.export_batch(),
                resources: self.allocator.export_resources(),
                tasks: self.allocator.export_tasks(),
                workflow_instances: self.engine.export_instances(),
                locations: self.location_handler.export_batch(),
            };

            tokio::runtime::Runtime::new()?.block_on(async {
                self.snapshot_manager.save_snapshot(&snapshot_id, data).await
            })?;

            println!("Saved checkpoint at step {}", ctx.step());
        }

        Ok(())
    }
}
```

### Restore and Resume

```rust
// Restore from checkpoint
let restored = snapshot_manager.restore_snapshot("checkpoint_500").await?;

// Import state
population.import_batch(restored.agents)?;
allocator.import_batch(restored.resources, restored.tasks)?;
engine.import_instances(restored.workflow_instances)?;

// Resume simulation from step 500
runner.set_start_step(500);
runner.run_for_steps(500)?;  // Run to step 1000
```

## State Branching

Create experimental branches from snapshots:

```rust
// Save baseline
manager.save_snapshot("baseline", baseline_data).await?;

// Create experiment branches
let baseline = manager.restore_snapshot("baseline").await?;

// Branch A: High demand
let mut branch_a = baseline.clone();
// Modify branch_a state...
manager.save_snapshot("experiment_a", branch_a).await?;

// Branch B: Low demand
let mut branch_b = baseline.clone();
// Modify branch_b state...
manager.save_snapshot("experiment_b", branch_b).await?;

// Run experiments
run_simulation("experiment_a")?;
run_simulation("experiment_b")?;

// Compare results
compare_snapshots("experiment_a", "experiment_b")?;
```

## Querying Snapshots

### DataFusion SQL Queries

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();

// Load snapshot
let agents = ctx.read_parquet(
    "snapshots/checkpoint_100/agents.parquet",
    ParquetReadOptions::default()
).await?;

// Query agent distribution by type
let df = agents
    .aggregate(
        vec![col("agent_type")],
        vec![
            count(col("id")).alias("count"),
            avg(col("position_x")).alias("avg_x"),
            avg(col("position_y")).alias("avg_y"),
        ],
    )?
    .collect()
    .await?;

// Query agent status distribution
let df = agents
    .aggregate(
        vec![col("status")],
        vec![count(col("id")).alias("count")],
    )?
    .sort(vec![col("count").sort(false, false)])?
    .collect()
    .await?;

// Join agents with locations
let locations = ctx.read_parquet(
    "snapshots/checkpoint_100/locations.parquet",
    ParquetReadOptions::default()
).await?;

let df = agents
    .join(
        locations,
        JoinType::Inner,
        &["location_id"],
        &["id"],
        None,
    )?
    .select(vec![
        col("agents.id"),
        col("agents.agent_type"),
        col("locations.location_type"),
    ])?
    .collect()
    .await?;
```

## Performance

Phase 6 snapshot performance:

- **Save time**: Well under the 1s target for 1,500 agents
- **Restore time**: Well under the 2s target for 1,500 agents
- **Round-trip**: Fast end-to-end
- **Compression**: Compact files with the Parquet Snappy codec
- **File size**: Substantially smaller than uncompressed

### Performance Tips

```rust
// ✅ Good - batch operations
let data = SnapshotData {
    agents: population.export_batch(),  // Single operation
    resources: allocator.export_resources(),
    // ...
};

// ❌ Bad - individual exports
let mut agents_vec = vec![];
for agent in population.iter() {
    agents_vec.push(agent.export());  // Slow!
}
```

## Export/Import Methods

All handlers implement export/import:

### PopulationHandler

```rust
impl PopulationHandler {
    pub fn export_batch(&self) -> RecordBatch {
        self.agents.clone()
    }

    pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()> {
        // Rebuild id_to_row index
        self.id_to_row.clear();
        for row_idx in 0..batch.num_rows() {
            let row = ArrowAgentRow::from_batch(&batch, row_idx)?;
            let id = AgentId::from_bytes(&row.id)?;
            self.id_to_row.insert(id, row_idx);
        }
        self.agents = batch;
        Ok(())
    }
}
```

### ResourceAllocator

```rust
impl ResourceAllocator {
    pub fn export_resources(&self) -> RecordBatch {
        self.resources.clone()
    }

    pub fn export_tasks(&self) -> RecordBatch {
        self.tasks.clone()
    }

    pub fn import_batch(
        &mut self,
        resources: RecordBatch,
        tasks: RecordBatch,
    ) -> Result<()> {
        // Rebuild indices for both resources and tasks
        self.resource_id_to_row.clear();
        self.task_id_to_row.clear();

        // Import resources
        for row_idx in 0..resources.num_rows() {
            let row = ArrowResourceRow::from_batch(&resources, row_idx)?;
            let id = ResourceId::from_bytes(&row.id)?;
            self.resource_id_to_row.insert(id, row_idx);
        }
        self.resources = resources;

        // Import tasks
        for row_idx in 0..tasks.num_rows() {
            let row = ArrowTaskRow::from_batch(&tasks, row_idx)?;
            let id = TaskId::from_bytes(&row.id)?;
            self.task_id_to_row.insert(id, row_idx);
        }
        self.tasks = tasks;

        Ok(())
    }
}
```

## Common Pitfalls

### Pitfall 1: Not Rebuilding Indices

```rust
// ❌ Wrong - indices not updated
pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()> {
    self.agents = batch;  // HashMap id_to_row still points to old rows!
    Ok(())
}

// ✅ Correct - rebuild indices
pub fn import_batch(&mut self, batch: RecordBatch) -> Result<()> {
    self.id_to_row.clear();
    for row_idx in 0..batch.num_rows() {
        let row = ArrowAgentRow::from_batch(&batch, row_idx)?;
        let id = AgentId::from_bytes(&row.id)?;
        self.id_to_row.insert(id, row_idx);
    }
    self.agents = batch;
    Ok(())
}
```

### Pitfall 2: Partial Snapshots

```rust
// ❌ Wrong - missing workflow state
let data = SnapshotData {
    agents: population.export_batch(),
    resources: allocator.export_resources(),
    tasks: allocator.export_tasks(),
    workflow_instances: RecordBatch::new_empty(...),  // Empty!
    locations: location_handler.export_batch(),
};

// ✅ Correct - complete state
let data = SnapshotData {
    agents: population.export_batch(),
    resources: allocator.export_resources(),
    tasks: allocator.export_tasks(),
    workflow_instances: engine.export_instances(),  // Include workflows!
    locations: location_handler.export_batch(),
};
```

### Pitfall 3: Snapshot Version Mismatch

```rust
// ❌ Wrong - no version checking
let restored = manager.restore_snapshot("old_checkpoint").await?;
population.import_batch(restored.agents)?;  // May have incompatible schema!

// ✅ Correct - check version
let restored = manager.restore_snapshot("old_checkpoint").await?;
if restored.metadata.version != "1.0" {
    return Err("Incompatible snapshot version");
}
population.import_batch(restored.agents)?;
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_roundtrip() {
        let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();

        // Create state
        let mut population = PopulationHandler::new();
        population.add_agent(Box::new(Customer::new(Point::new(0.0, 0.0))));

        // Save
        let data = SnapshotData {
            agents: population.export_batch(),
            resources: RecordBatch::new_empty(...),
            tasks: RecordBatch::new_empty(...),
            workflow_instances: RecordBatch::new_empty(...),
            locations: RecordBatch::new_empty(...),
        };

        manager.save_snapshot("test", data).await.unwrap();

        // Restore
        let restored = manager.restore_snapshot("test").await.unwrap();

        // Verify
        assert_eq!(restored.agents.num_rows(), 1);
    }
}
```

## Related Documentation

- [Agents](agents.md) - Agent state persistence
- [Resources](resources.md) - Resource and task state
- [Workflows](workflows.md) - Workflow instance state

## Summary

Snapshot system provides:

- Complete simulation state persistence
- Parquet backend with 9x compression
- 15ms save / 7ms restore for 1,500 agents
- SQL queries with DataFusion
- State branching for experiments
- Full Arrow RecordBatch integration

---

**Last Updated:** March 16, 2026
