//! SimulationManager — catalog layer for tracking multiple simulation runs
//!
//! Maintains two DataFusion-registered tables:
//! - `system.simulations` — one row per simulation run
//! - `system.snapshots` — one row per saved snapshot
//!
//! Domain-agnostic by design; lives in `sim-core` so it can reference
//! `SnapshotManager` and `SnapshotMetadata` directly.

use crate::snapshot::{SnapshotBackend, SnapshotManager, SnapshotMetadata};
use crate::{Error, Result};
use arrow::array::{Array, Int64Array, StringArray, TimestampMillisecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use arrow::record_batch::RecordBatch;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::catalog::MemTable;
use datafusion::execution::context::SessionContext;
use std::sync::Arc;
use uuid::Uuid;

// ─── Public plain-data structs ────────────────────────────────────────────────

/// Record returned by [`SimulationManager::list_simulations`].
#[derive(Debug, Clone, PartialEq)]
pub struct SimulationRecord {
    pub id: Uuid,
    pub label: Option<String>,
    pub created_at: DateTime<Utc>,
    pub properties: Option<serde_json::Value>,
}

/// Record returned by [`SimulationManager::list_snapshots`].
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotRecord {
    pub id: Uuid,
    pub simulation_id: Uuid,
    pub label: Option<String>,
    pub sim_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub agent_count: usize,
    pub resource_count: usize,
    pub task_count: usize,
    pub workflow_instance_count: usize,
    pub location_count: usize,
}

// ─── Backend ──────────────────────────────────────────────────────────────────

/// Storage backend for the manager's own persistence.
#[derive(Debug, Clone)]
pub enum SimulationManagerBackend {
    /// All tables are in-memory (non-persistent).
    Memory,
    /// Simulation data stored under `{base_path}/{simulation_id}/`.
    Parquet(String),
}

// ─── Schemas ──────────────────────────────────────────────────────────────────

fn simulations_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("label", DataType::Utf8, true),
        Field::new(
            "created_at",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            true,
        ),
        Field::new("properties", DataType::Utf8, true),
    ]))
}

fn snapshots_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("simulation_id", DataType::Utf8, false),
        Field::new("label", DataType::Utf8, true),
        Field::new(
            "sim_time",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            true,
        ),
        Field::new(
            "created_at",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            true,
        ),
        Field::new("agent_count", DataType::Int64, true),
        Field::new("resource_count", DataType::Int64, true),
        Field::new("task_count", DataType::Int64, true),
        Field::new("workflow_instance_count", DataType::Int64, true),
        Field::new("location_count", DataType::Int64, true),
    ]))
}

// ─── Manager ──────────────────────────────────────────────────────────────────

/// Catalog layer that tracks multiple simulation runs and their snapshots.
pub struct SimulationManager {
    ctx: SessionContext,
    backend: SimulationManagerBackend,
}

impl SimulationManager {
    /// Create a new `SimulationManager` with empty system tables.
    pub async fn new(backend: SimulationManagerBackend) -> Result<Self> {
        let ctx = SessionContext::new();

        // Register empty system tables
        let sims_table = MemTable::try_new(simulations_schema(), vec![vec![]])?;
        ctx.register_table("system_simulations", Arc::new(sims_table))?;

        let snaps_table = MemTable::try_new(snapshots_schema(), vec![vec![]])?;
        ctx.register_table("system_snapshots", Arc::new(snaps_table))?;

        Ok(Self { ctx, backend })
    }

    /// Register a new simulation run. Returns the generated simulation ID (Uuid v7).
    pub async fn create_simulation(
        &mut self,
        label: Option<&str>,
        properties: Option<serde_json::Value>,
    ) -> Result<Uuid> {
        let id = Uuid::now_v7();
        let now = Utc::now();

        let batch = RecordBatch::try_new(
            simulations_schema(),
            vec![
                Arc::new(StringArray::from(vec![id.to_string()])),
                Arc::new(StringArray::from(vec![label.map(str::to_string)])),
                Arc::new(
                    TimestampMillisecondArray::from(vec![now.timestamp_millis()])
                        .with_timezone_opt(Some("UTC")),
                ),
                Arc::new(StringArray::from(vec![
                    properties.as_ref().map(|v| v.to_string()),
                ])),
            ],
        )
        .map_err(Error::Arrow)?;

        self.append_to_table("system_simulations", batch, simulations_schema())
            .await?;

        Ok(id)
    }

    /// List all registered simulation runs.
    pub async fn list_simulations(&self) -> Result<Vec<SimulationRecord>> {
        let batches = self
            .ctx
            .table("system_simulations")
            .await?
            .collect()
            .await?;

        let mut results = Vec::new();
        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| Error::InvalidState("missing id column".into()))?;
            let label_col = batch
                .column_by_name("label")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let created_at_col = batch
                .column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<TimestampMillisecondArray>());
            let props_col = batch
                .column_by_name("properties")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());

            for i in 0..batch.num_rows() {
                let id: Uuid = id_col
                    .value(i)
                    .parse()
                    .map_err(|_| Error::InvalidState("invalid simulation id uuid".into()))?;
                let label = label_col.and_then(|c| {
                    if c.is_null(i) {
                        None
                    } else {
                        Some(c.value(i).to_string())
                    }
                });
                let created_at = created_at_col
                    .map(|c| {
                        Utc.timestamp_millis_opt(c.value(i))
                            .single()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let properties = props_col.and_then(|c| {
                    if c.is_null(i) {
                        None
                    } else {
                        serde_json::from_str(c.value(i)).ok()
                    }
                });

                results.push(SimulationRecord {
                    id,
                    label,
                    created_at,
                    properties,
                });
            }
        }
        Ok(results)
    }

    /// Record a snapshot taken by a runner. Called after the runner saves state.
    pub async fn record_snapshot(&mut self, metadata: &SnapshotMetadata) -> Result<()> {
        let batch = RecordBatch::try_new(
            snapshots_schema(),
            vec![
                Arc::new(StringArray::from(vec![metadata.snapshot_id.to_string()])),
                Arc::new(StringArray::from(vec![metadata.simulation_id.to_string()])),
                Arc::new(StringArray::from(vec![metadata.label.clone()])),
                Arc::new(
                    TimestampMillisecondArray::from(vec![metadata.sim_time.timestamp_millis()])
                        .with_timezone_opt(Some("UTC")),
                ),
                Arc::new(
                    TimestampMillisecondArray::from(vec![metadata.timestamp.timestamp_millis()])
                        .with_timezone_opt(Some("UTC")),
                ),
                Arc::new(Int64Array::from(vec![metadata.agent_count as i64])),
                Arc::new(Int64Array::from(vec![metadata.resource_count as i64])),
                Arc::new(Int64Array::from(vec![metadata.task_count as i64])),
                Arc::new(Int64Array::from(vec![
                    metadata.workflow_instance_count as i64,
                ])),
                Arc::new(Int64Array::from(vec![metadata.location_count as i64])),
            ],
        )
        .map_err(Error::Arrow)?;

        self.append_to_table("system_snapshots", batch, snapshots_schema())
            .await
    }

    /// List all snapshots for a given simulation.
    pub async fn list_snapshots(&self, simulation_id: Uuid) -> Result<Vec<SnapshotRecord>> {
        let batches = self.ctx.table("system_snapshots").await?.collect().await?;

        let sim_id_str = simulation_id.to_string();
        let mut results = Vec::new();

        for batch in &batches {
            let id_col = batch
                .column_by_name("id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| Error::InvalidState("missing id column".into()))?;
            let sim_id_col = batch
                .column_by_name("simulation_id")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>())
                .ok_or_else(|| Error::InvalidState("missing simulation_id column".into()))?;
            let label_col = batch
                .column_by_name("label")
                .and_then(|c| c.as_any().downcast_ref::<StringArray>());
            let sim_time_col = batch
                .column_by_name("sim_time")
                .and_then(|c| c.as_any().downcast_ref::<TimestampMillisecondArray>());
            let created_at_col = batch
                .column_by_name("created_at")
                .and_then(|c| c.as_any().downcast_ref::<TimestampMillisecondArray>());
            let agent_col = batch
                .column_by_name("agent_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let resource_col = batch
                .column_by_name("resource_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let task_col = batch
                .column_by_name("task_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let workflow_col = batch
                .column_by_name("workflow_instance_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());
            let location_col = batch
                .column_by_name("location_count")
                .and_then(|c| c.as_any().downcast_ref::<Int64Array>());

            for i in 0..batch.num_rows() {
                if sim_id_col.value(i) != sim_id_str {
                    continue;
                }
                let id: Uuid = id_col
                    .value(i)
                    .parse()
                    .map_err(|_| Error::InvalidState("invalid snapshot id uuid".into()))?;
                let sim_id: Uuid = sim_id_col
                    .value(i)
                    .parse()
                    .map_err(|_| Error::InvalidState("invalid simulation_id uuid".into()))?;
                let label = label_col.and_then(|c| {
                    if c.is_null(i) {
                        None
                    } else {
                        Some(c.value(i).to_string())
                    }
                });
                let sim_time = sim_time_col
                    .map(|c| {
                        Utc.timestamp_millis_opt(c.value(i))
                            .single()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();
                let created_at = created_at_col
                    .map(|c| {
                        Utc.timestamp_millis_opt(c.value(i))
                            .single()
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                results.push(SnapshotRecord {
                    id,
                    simulation_id: sim_id,
                    label,
                    sim_time,
                    created_at,
                    agent_count: agent_col.map(|c| c.value(i) as usize).unwrap_or(0),
                    resource_count: resource_col.map(|c| c.value(i) as usize).unwrap_or(0),
                    task_count: task_col.map(|c| c.value(i) as usize).unwrap_or(0),
                    workflow_instance_count: workflow_col.map(|c| c.value(i) as usize).unwrap_or(0),
                    location_count: location_col.map(|c| c.value(i) as usize).unwrap_or(0),
                });
            }
        }
        Ok(results)
    }

    /// Create a `SnapshotManager` scoped to a simulation's storage directory.
    pub fn snapshot_manager_for(&self, simulation_id: Uuid) -> Result<SnapshotManager> {
        // SnapshotManager::new is async; wrap with block_on
        let backend = match &self.backend {
            SimulationManagerBackend::Memory => SnapshotBackend::Memory,
            SimulationManagerBackend::Parquet(base) => {
                SnapshotBackend::Parquet(format!("{base}/{simulation_id}"))
            }
        };
        tokio::runtime::Handle::current()
            .block_on(async move { SnapshotManager::new(backend).await })
    }

    // ─── Internal helpers ─────────────────────────────────────────────────────

    /// Append a `RecordBatch` to a registered MemTable by name.
    async fn append_to_table(
        &self,
        table_name: &str,
        batch: RecordBatch,
        schema: Arc<Schema>,
    ) -> Result<()> {
        let existing_df = self.ctx.table(table_name).await?;
        let mut existing_batches = existing_df.collect().await?;
        existing_batches.push(batch);
        let combined =
            arrow::compute::concat_batches(&schema, &existing_batches).map_err(Error::Arrow)?;
        self.ctx.deregister_table(table_name)?;
        let new_table = MemTable::try_new(schema, vec![vec![combined]])?;
        self.ctx.register_table(table_name, Arc::new(new_table))?;
        Ok(())
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod manager_tests;
