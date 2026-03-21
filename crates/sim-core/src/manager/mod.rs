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

// ─── Serde helpers for JSON catalog records ──────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct SimulationCatalogRecord {
    id: String,
    label: Option<String>,
    created_at: String,
    properties: Option<serde_json::Value>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SnapshotCatalogRecord {
    id: String,
    simulation_id: String,
    label: Option<String>,
    sim_time: String,
    created_at: String,
    agent_count: usize,
    resource_count: usize,
    task_count: usize,
    workflow_instance_count: usize,
    location_count: usize,
}

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

        self.persist_simulation(id, label, properties.as_ref(), now)?;

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
            .await?;

        self.persist_snapshot(metadata)?;

        Ok(())
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

    // ─── Persistence ──────────────────────────────────────────────────────────

    /// Create a manager and load any previously persisted catalog from disk.
    ///
    /// With a `Memory` backend this is identical to `new()`. With a `Parquet`
    /// backend it scans `{base}/system/simulations/` and
    /// `{base}/system/snapshots/` for JSON records written by earlier runs.
    pub async fn load(backend: SimulationManagerBackend) -> Result<Self> {
        let mut mgr = Self::new(backend).await?;
        if let SimulationManagerBackend::Parquet(ref base) = mgr.backend.clone() {
            mgr.load_catalog_from_disk(base).await?;
        }
        Ok(mgr)
    }

    /// Convenience constructor: `Parquet(base)` backend + catalog loaded from disk.
    pub async fn new_with_persistence(base: &str) -> Result<Self> {
        Self::load(SimulationManagerBackend::Parquet(base.to_string())).await
    }

    /// Scan disk catalog directories and populate in-memory tables.
    async fn load_catalog_from_disk(&mut self, base: &str) -> Result<()> {
        let sims_dir = std::path::Path::new(base)
            .join("system")
            .join("simulations");
        let snaps_dir = std::path::Path::new(base).join("system").join("snapshots");

        if sims_dir.exists() {
            let entries = std::fs::read_dir(&sims_dir)
                .map_err(|e| Error::Custom(format!("read_dir simulations: {e}")))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json")
                    && let Ok(contents) = std::fs::read_to_string(&path)
                    && let Ok(rec) = serde_json::from_str::<SimulationCatalogRecord>(&contents)
                {
                    let id: Uuid = rec
                        .id
                        .parse()
                        .map_err(|_| Error::InvalidState("bad sim uuid".into()))?;
                    let created_at: DateTime<Utc> = rec.created_at.parse().unwrap_or_default();
                    let batch = RecordBatch::try_new(
                        simulations_schema(),
                        vec![
                            Arc::new(StringArray::from(vec![rec.id.clone()])),
                            Arc::new(StringArray::from(vec![rec.label.clone()])),
                            Arc::new(
                                TimestampMillisecondArray::from(vec![
                                    created_at.timestamp_millis(),
                                ])
                                .with_timezone_opt(Some("UTC")),
                            ),
                            Arc::new(StringArray::from(vec![
                                rec.properties.as_ref().map(|v| v.to_string()),
                            ])),
                        ],
                    )
                    .map_err(Error::Arrow)?;
                    self.append_to_table("system_simulations", batch, simulations_schema())
                        .await?;
                    let _ = id; // used above
                }
            }
        }

        if snaps_dir.exists() {
            let entries = std::fs::read_dir(&snaps_dir)
                .map_err(|e| Error::Custom(format!("read_dir snapshots: {e}")))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json")
                    && let Ok(contents) = std::fs::read_to_string(&path)
                    && let Ok(rec) = serde_json::from_str::<SnapshotCatalogRecord>(&contents)
                {
                    let sim_time: DateTime<Utc> = rec.sim_time.parse().unwrap_or_default();
                    let created_at: DateTime<Utc> = rec.created_at.parse().unwrap_or_default();
                    let batch = RecordBatch::try_new(
                        snapshots_schema(),
                        vec![
                            Arc::new(StringArray::from(vec![rec.id.clone()])),
                            Arc::new(StringArray::from(vec![rec.simulation_id.clone()])),
                            Arc::new(StringArray::from(vec![rec.label.clone()])),
                            Arc::new(
                                TimestampMillisecondArray::from(vec![sim_time.timestamp_millis()])
                                    .with_timezone_opt(Some("UTC")),
                            ),
                            Arc::new(
                                TimestampMillisecondArray::from(vec![
                                    created_at.timestamp_millis(),
                                ])
                                .with_timezone_opt(Some("UTC")),
                            ),
                            Arc::new(Int64Array::from(vec![rec.agent_count as i64])),
                            Arc::new(Int64Array::from(vec![rec.resource_count as i64])),
                            Arc::new(Int64Array::from(vec![rec.task_count as i64])),
                            Arc::new(Int64Array::from(vec![rec.workflow_instance_count as i64])),
                            Arc::new(Int64Array::from(vec![rec.location_count as i64])),
                        ],
                    )
                    .map_err(Error::Arrow)?;
                    self.append_to_table("system_snapshots", batch, snapshots_schema())
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Write a simulation record to disk as JSON.
    fn persist_simulation(
        &self,
        id: Uuid,
        label: Option<&str>,
        properties: Option<&serde_json::Value>,
        created_at: DateTime<Utc>,
    ) -> Result<()> {
        let SimulationManagerBackend::Parquet(ref base) = self.backend else {
            return Ok(());
        };
        let dir = std::path::Path::new(base)
            .join("system")
            .join("simulations");
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::Custom(format!("create_dir simulations: {e}")))?;
        let record = SimulationCatalogRecord {
            id: id.to_string(),
            label: label.map(str::to_string),
            created_at: created_at.to_rfc3339(),
            properties: properties.cloned(),
        };
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| Error::Custom(format!("serialize simulation: {e}")))?;
        std::fs::write(dir.join(format!("{id}.json")), json)
            .map_err(|e| Error::Custom(format!("write simulation json: {e}")))?;
        Ok(())
    }

    /// Write a snapshot record to disk as JSON.
    fn persist_snapshot(&self, metadata: &SnapshotMetadata) -> Result<()> {
        let SimulationManagerBackend::Parquet(ref base) = self.backend else {
            return Ok(());
        };
        let dir = std::path::Path::new(base).join("system").join("snapshots");
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::Custom(format!("create_dir snapshots: {e}")))?;
        let record = SnapshotCatalogRecord {
            id: metadata.snapshot_id.to_string(),
            simulation_id: metadata.simulation_id.to_string(),
            label: metadata.label.clone(),
            sim_time: metadata.sim_time.to_rfc3339(),
            created_at: metadata.timestamp.to_rfc3339(),
            agent_count: metadata.agent_count,
            resource_count: metadata.resource_count,
            task_count: metadata.task_count,
            workflow_instance_count: metadata.workflow_instance_count,
            location_count: metadata.location_count,
        };
        let json = serde_json::to_string_pretty(&record)
            .map_err(|e| Error::Custom(format!("serialize snapshot: {e}")))?;
        std::fs::write(dir.join(format!("{}.json", metadata.snapshot_id)), json)
            .map_err(|e| Error::Custom(format!("write snapshot json: {e}")))?;
        Ok(())
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
