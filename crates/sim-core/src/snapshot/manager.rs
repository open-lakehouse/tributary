//! Snapshot manager for saving and restoring simulation state
//!
//! This module provides the core snapshot functionality, allowing the simulation to
//! save its entire state to a backend (memory or Parquet files) and restore it later.
//!
//! # Architecture
//!
//! The snapshot system consists of three main components:
//!
//! - **SnapshotBackend**: Enum defining where to store snapshots (Memory or Parquet)
//! - **SnapshotData**: Struct containing RecordBatches for all entity types
//! - **SnapshotManager**: Manages saving/restoring via DataFusion SessionContext
//!
//! # Storage Backends
//!
//! ## Memory Backend
//!
//! Uses DataFusion's MemTable for in-memory storage. Fast but ephemeral.
//! Suitable for testing, debugging, and short-lived simulations.
//!
//! ## Parquet Backend
//!
//! Uses DataFusion's ListingTable for persistent Parquet storage.
//! Snapshots are written to: `snapshots/{snapshot_id}/` with separate files:
//! - `agents.parquet`
//! - `resources.parquet`
//! - `tasks.parquet`
//! - `workflow_instances.parquet`
//! - `locations.parquet` (when location tracking is enabled)
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_core::snapshot::{SnapshotManager, SnapshotBackend, SnapshotData};
//! use arrow::array::RecordBatch;
//!
//! // Create manager with Parquet backend
//! let manager = SnapshotManager::new(
//!     SnapshotBackend::Parquet("./snapshots".to_string())
//! ).await?;
//!
//! // Collect state from handlers
//! let snapshot_data = SnapshotData {
//!     agents: population_handler.export_batch(),
//!     resources: resource_allocator.export_resources(),
//!     tasks: resource_allocator.export_tasks(),
//!     workflow_instances: workflow_engine.export_instances(),
//!     locations: None,
//! };
//!
//! // Save snapshot
//! manager.save_snapshot("checkpoint_1", snapshot_data).await?;
//!
//! // Restore snapshot
//! let restored = manager.restore_snapshot("checkpoint_1").await?;
//! population_handler.import_batch(restored.agents)?;
//! ```

use crate::{Error, Result};
use arrow::array::RecordBatch;
use datafusion::catalog::{MemTable, TableProvider};
use datafusion::execution::context::SessionContext;
use datafusion::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

/// Storage backend for snapshots
///
/// Determines where snapshot data is stored and how it's accessed.
///
/// # Variants
///
/// - **Memory**: In-memory storage using MemTable (non-persistent)
/// - **Parquet**: Persistent storage using Parquet files with directory structure
///
/// # Example
///
/// ```rust,ignore
/// // In-memory for testing
/// let backend = SnapshotBackend::Memory;
///
/// // Persistent storage
/// let backend = SnapshotBackend::Parquet("./my_snapshots".to_string());
/// ```
#[derive(Debug, Clone)]
pub enum SnapshotBackend {
    /// Store snapshots in memory (fast, non-persistent)
    Memory,
    /// Store snapshots as Parquet files (persistent, queryable)
    /// The string is the base directory path
    Parquet(String),
}

/// Container for snapshot data
///
/// Holds RecordBatches for all entity types in the simulation.
/// Each field corresponds to a handler's export_batch() output.
///
/// # Fields
///
/// - `agents`: Agent population state
/// - `resources`: Resource capacity and availability
/// - `tasks`: Task allocation state
/// - `workflow_instances`: Workflow execution state
/// - `locations`: Optional location/spatial data
///
/// # Example
///
/// ```rust,ignore
/// let snapshot = SnapshotData {
///     agents: population_handler.export_batch(),
///     resources: resource_allocator.export_resources(),
///     tasks: resource_allocator.export_tasks(),
///     workflow_instances: workflow_engine.export_instances(),
///     locations: None,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct SnapshotData {
    /// Agent population state (from PopulationHandler)
    pub agents: RecordBatch,
    /// Resource state (from ResourceAllocator)
    pub resources: RecordBatch,
    /// Task state (from ResourceAllocator)
    pub tasks: RecordBatch,
    /// Workflow instance state (from WorkflowEngine)
    pub workflow_instances: RecordBatch,
    /// Optional location data for spatial simulations
    pub locations: Option<RecordBatch>,
}

/// Manages snapshot save/restore operations
///
/// Uses DataFusion's SessionContext to handle reading/writing RecordBatches
/// to various backends (memory or Parquet files).
///
/// # Architecture
///
/// The manager maintains a SessionContext and registers tables for each entity type:
/// - `agents` table
/// - `resources` table
/// - `tasks` table
/// - `workflow_instances` table
/// - `locations` table (optional)
///
/// # Thread Safety
///
/// The SessionContext is internally thread-safe (uses Arc), so SnapshotManager
/// can be safely shared across threads or async tasks.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::snapshot::{SnapshotManager, SnapshotBackend};
///
/// // Create manager
/// let manager = SnapshotManager::new(
///     SnapshotBackend::Parquet("./snapshots".to_string())
/// ).await?;
///
/// // Save and restore
/// manager.save_snapshot("my_snapshot", data).await?;
/// let restored = manager.restore_snapshot("my_snapshot").await?;
/// ```
pub struct SnapshotManager {
    /// DataFusion session context for table operations
    ctx: SessionContext,
    /// Storage backend configuration
    backend: SnapshotBackend,
}

impl SnapshotManager {
    /// Create a new snapshot manager
    ///
    /// Initializes a DataFusion SessionContext for managing snapshot data.
    ///
    /// # Arguments
    ///
    /// * `backend` - Storage backend (Memory or Parquet)
    ///
    /// # Returns
    ///
    /// A new SnapshotManager instance
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let manager = SnapshotManager::new(SnapshotBackend::Memory).await?;
    /// ```
    pub async fn new(backend: SnapshotBackend) -> Result<Self> {
        let ctx = SessionContext::new();
        Ok(Self { ctx, backend })
    }

    /// Save a snapshot to the configured backend
    ///
    /// Writes all RecordBatches in the snapshot data to the backend storage.
    /// For Parquet backend, creates a directory structure:
    /// `{base_path}/{snapshot_id}/agents.parquet`, etc.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - Unique identifier for this snapshot
    /// * `data` - Snapshot data containing all entity RecordBatches
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Error on failure
    ///
    /// # Errors
    ///
    /// - Returns Error::DataFusion if writing to backend fails
    /// - Returns Error::Io if file/directory operations fail (Parquet backend)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let data = SnapshotData {
    ///     agents: handler.export_batch(),
    ///     // ... other fields
    /// };
    /// manager.save_snapshot("checkpoint_1", data).await?;
    /// ```
    pub async fn save_snapshot(&self, snapshot_id: &str, data: SnapshotData) -> Result<()> {
        match &self.backend {
            SnapshotBackend::Memory => self.save_to_memory(snapshot_id, data).await,
            SnapshotBackend::Parquet(base_path) => {
                self.save_to_parquet(base_path, snapshot_id, data).await
            }
        }
    }

    /// Restore a snapshot from the configured backend
    ///
    /// Reads all RecordBatches for the specified snapshot from backend storage.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - Identifier of the snapshot to restore
    ///
    /// # Returns
    ///
    /// SnapshotData containing all restored RecordBatches
    ///
    /// # Errors
    ///
    /// - Returns Error::DataFusion if reading from backend fails
    /// - Returns Error::Io if snapshot directory doesn't exist (Parquet backend)
    /// - Returns Error::InvalidState if snapshot data is corrupted
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let snapshot = manager.restore_snapshot("checkpoint_1").await?;
    /// population_handler.import_batch(snapshot.agents)?;
    /// ```
    pub async fn restore_snapshot(&self, snapshot_id: &str) -> Result<SnapshotData> {
        match &self.backend {
            SnapshotBackend::Memory => self.restore_from_memory(snapshot_id).await,
            SnapshotBackend::Parquet(base_path) => {
                self.restore_from_parquet(base_path, snapshot_id).await
            }
        }
    }

    /// Save snapshot to in-memory tables
    ///
    /// Registers each RecordBatch as a MemTable in the SessionContext.
    /// Table names are prefixed with the snapshot_id to allow multiple snapshots.
    async fn save_to_memory(&self, snapshot_id: &str, data: SnapshotData) -> Result<()> {
        // Register agents table
        let agents_table = Self::create_mem_table(data.agents)?;
        self.ctx
            .register_table(format!("{}_agents", snapshot_id), agents_table)?;

        // Register resources table
        let resources_table = Self::create_mem_table(data.resources)?;
        self.ctx
            .register_table(format!("{}_resources", snapshot_id), resources_table)?;

        // Register tasks table
        let tasks_table = Self::create_mem_table(data.tasks)?;
        self.ctx
            .register_table(format!("{}_tasks", snapshot_id), tasks_table)?;

        // Register workflow_instances table
        let instances_table = Self::create_mem_table(data.workflow_instances)?;
        self.ctx.register_table(
            format!("{}_workflow_instances", snapshot_id),
            instances_table,
        )?;

        // Register locations table if present
        if let Some(locations) = data.locations {
            let locations_table = Self::create_mem_table(locations)?;
            self.ctx
                .register_table(format!("{}_locations", snapshot_id), locations_table)?;
        }

        Ok(())
    }

    /// Restore snapshot from in-memory tables
    ///
    /// Reads RecordBatches from MemTables registered in the SessionContext.
    async fn restore_from_memory(&self, snapshot_id: &str) -> Result<SnapshotData> {
        // Read agents
        let agents = self.read_table(&format!("{}_agents", snapshot_id)).await?;

        // Read resources
        let resources = self
            .read_table(&format!("{}_resources", snapshot_id))
            .await?;

        // Read tasks
        let tasks = self.read_table(&format!("{}_tasks", snapshot_id)).await?;

        // Read workflow_instances
        let workflow_instances = self
            .read_table(&format!("{}_workflow_instances", snapshot_id))
            .await?;

        // Read locations if present
        let locations = if self.ctx.table_exist(format!("{}_locations", snapshot_id))? {
            Some(
                self.read_table(&format!("{}_locations", snapshot_id))
                    .await?,
            )
        } else {
            None
        };

        Ok(SnapshotData {
            agents,
            resources,
            tasks,
            workflow_instances,
            locations,
        })
    }

    /// Save snapshot to Parquet files
    ///
    /// Creates a directory structure: {base_path}/{snapshot_id}/
    /// and writes each RecordBatch as a separate Parquet file.
    async fn save_to_parquet(
        &self,
        base_path: &str,
        snapshot_id: &str,
        data: SnapshotData,
    ) -> Result<()> {
        use std::fs;

        // Create snapshot directory
        let snapshot_dir = PathBuf::from(base_path).join(snapshot_id);
        fs::create_dir_all(&snapshot_dir)?;

        // Write agents
        self.write_batch_to_parquet(&snapshot_dir, snapshot_id, "agents", data.agents)
            .await?;

        // Write resources
        self.write_batch_to_parquet(&snapshot_dir, snapshot_id, "resources", data.resources)
            .await?;

        // Write tasks
        self.write_batch_to_parquet(&snapshot_dir, snapshot_id, "tasks", data.tasks)
            .await?;

        // Write workflow_instances
        self.write_batch_to_parquet(
            &snapshot_dir,
            snapshot_id,
            "workflow_instances",
            data.workflow_instances,
        )
        .await?;

        // Write locations if present
        if let Some(locations) = data.locations {
            self.write_batch_to_parquet(&snapshot_dir, snapshot_id, "locations", locations)
                .await?;
        }

        Ok(())
    }

    /// Helper to write a single RecordBatch to a Parquet file
    async fn write_batch_to_parquet(
        &self,
        snapshot_dir: &PathBuf,
        snapshot_id: &str,
        name: &str,
        batch: RecordBatch,
    ) -> Result<()> {
        use datafusion::dataframe::DataFrameWriteOptions;

        // Register as temporary table
        let temp_table_name = format!("temp_{}_{}", snapshot_id, name);
        let table = Self::create_mem_table(batch)?;
        self.ctx.register_table(&temp_table_name, table)?;

        // Write to Parquet
        let df = self.ctx.table(&temp_table_name).await?;
        let file_path = snapshot_dir.join(format!("{}.parquet", name));

        df.write_parquet(
            file_path
                .to_str()
                .ok_or_else(|| Error::InvalidState("Invalid file path".to_string()))?,
            DataFrameWriteOptions::new(),
            None,
        )
        .await?;

        // Deregister temporary table
        self.ctx.deregister_table(&temp_table_name)?;

        Ok(())
    }

    /// Restore snapshot from Parquet files
    ///
    /// Reads RecordBatches from Parquet files in: {base_path}/{snapshot_id}/
    async fn restore_from_parquet(
        &self,
        base_path: &str,
        snapshot_id: &str,
    ) -> Result<SnapshotData> {
        let snapshot_dir = PathBuf::from(base_path).join(snapshot_id);

        // Verify snapshot directory exists
        if !snapshot_dir.exists() {
            return Err(Error::InvalidState(format!(
                "Snapshot directory not found: {}",
                snapshot_dir.display()
            )));
        }

        // Read all batches
        let agents = self
            .read_batch_from_parquet(&snapshot_dir, snapshot_id, "agents")
            .await?;
        let resources = self
            .read_batch_from_parquet(&snapshot_dir, snapshot_id, "resources")
            .await?;
        let tasks = self
            .read_batch_from_parquet(&snapshot_dir, snapshot_id, "tasks")
            .await?;
        let workflow_instances = self
            .read_batch_from_parquet(&snapshot_dir, snapshot_id, "workflow_instances")
            .await?;

        // Check if locations file exists
        let locations_path = snapshot_dir.join("locations.parquet");
        let locations = if locations_path.exists() {
            Some(
                self.read_batch_from_parquet(&snapshot_dir, snapshot_id, "locations")
                    .await?,
            )
        } else {
            None
        };

        Ok(SnapshotData {
            agents,
            resources,
            tasks,
            workflow_instances,
            locations,
        })
    }

    /// Helper to read a single Parquet file into a RecordBatch
    async fn read_batch_from_parquet(
        &self,
        snapshot_dir: &PathBuf,
        snapshot_id: &str,
        name: &str,
    ) -> Result<RecordBatch> {
        let file_path = snapshot_dir.join(format!("{}.parquet", name));

        if !file_path.exists() {
            return Err(Error::InvalidState(format!(
                "Snapshot file not found: {}",
                file_path.display()
            )));
        }

        // Register Parquet file as table
        let table_name = format!("restore_{}_{}", snapshot_id, name);
        self.ctx
            .register_parquet(
                &table_name,
                file_path
                    .to_str()
                    .ok_or_else(|| Error::InvalidState("Invalid file path".to_string()))?,
                ParquetReadOptions::default(),
            )
            .await?;

        // Read into RecordBatch
        let batch = self.read_table(&table_name).await?;

        // Deregister table
        self.ctx.deregister_table(&table_name)?;

        Ok(batch)
    }

    /// Create a MemTable from a RecordBatch
    ///
    /// Helper function to wrap a RecordBatch in a MemTable TableProvider.
    fn create_mem_table(batch: RecordBatch) -> Result<Arc<dyn TableProvider>> {
        let schema = batch.schema();
        let table = MemTable::try_new(schema, vec![vec![batch]])?;
        Ok(Arc::new(table))
    }

    /// Read a table into a single RecordBatch
    ///
    /// Executes a SELECT * query on the table and collects results into one RecordBatch.
    /// This assumes the table fits in memory, which is reasonable for simulation snapshots.
    async fn read_table(&self, table_name: &str) -> Result<RecordBatch> {
        let df = self.ctx.table(table_name).await?;
        let batches = df.collect().await?;

        if batches.is_empty() {
            // Return empty batch with schema from table
            let table = self.ctx.table(table_name).await?;
            let schema = table.schema();
            return Ok(RecordBatch::new_empty(schema.inner().clone()));
        }

        // If single batch, return it
        if batches.len() == 1 {
            return Ok(batches[0].clone());
        }

        // Concatenate multiple batches
        let schema = batches[0].schema();
        arrow::compute::concat_batches(&schema, &batches).map_err(Error::Arrow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::agent_schema;
    use crate::resources::{resource_schema, task_schema};
    use crate::workflows::workflow_instance_schema;
    use arrow::array::{
        BinaryArray, FixedSizeBinaryArray, Float64Array, Int32Array, Int64Array, StringArray,
    };

    fn create_test_agents_batch() -> RecordBatch {
        let schema = agent_schema();
        let id_array = FixedSizeBinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let type_array = StringArray::from(vec!["test_agent"]);
        let status_array = StringArray::from(vec!["active"]);
        let pos_x_array = Float64Array::from(vec![Some(1.0)]);
        let pos_y_array = Float64Array::from(vec![Some(2.0)]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(type_array),
                Arc::new(status_array),
                Arc::new(pos_x_array),
                Arc::new(pos_y_array),
                Arc::new(metadata_array),
            ],
        )
        .unwrap()
    }

    fn create_test_resources_batch() -> RecordBatch {
        let schema = resource_schema();
        let id_array = BinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let type_array = StringArray::from(vec!["test_resource"]);
        let capacity_array = Int64Array::from(vec![10]);
        let available_array = Int64Array::from(vec![5]);
        let location_array = BinaryArray::from(vec![None::<&[u8]>]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(type_array),
                Arc::new(capacity_array),
                Arc::new(available_array),
                Arc::new(location_array),
                Arc::new(metadata_array),
            ],
        )
        .unwrap()
    }

    fn create_test_tasks_batch() -> RecordBatch {
        let schema = task_schema();
        let id_array = BinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let type_array = StringArray::from(vec!["test_task"]);
        let status_array = StringArray::from(vec!["pending"]);
        let capacity_array = Int64Array::from(vec![5]);
        let resource_array = BinaryArray::from(vec![None::<&[u8]>]);
        let workflow_array = BinaryArray::from(vec![None::<&[u8]>]);
        let priority_array = Int32Array::from(vec![0]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(type_array),
                Arc::new(status_array),
                Arc::new(capacity_array),
                Arc::new(resource_array),
                Arc::new(workflow_array),
                Arc::new(priority_array),
                Arc::new(metadata_array),
            ],
        )
        .unwrap()
    }

    fn create_test_instances_batch() -> RecordBatch {
        let schema = workflow_instance_schema();
        let id_array = BinaryArray::from(vec![vec![0u8; 16].as_slice()]);
        let workflow_id_array = BinaryArray::from(vec![vec![1u8; 16].as_slice()]);
        let status_array = StringArray::from(vec!["Running"]);
        let step_index_array = Int64Array::from(vec![0]);
        let total_steps_array = Int64Array::from(vec![3]);
        let agent_id_array = BinaryArray::from(vec![None::<&[u8]>]);
        let started_at_array = Int64Array::from(vec![0]);
        let completed_at_array = Int64Array::from(vec![None::<i64>]);
        let context_array = StringArray::from(vec![Some("{}")]);
        let metadata_array = StringArray::from(vec![Some("{}")]);

        RecordBatch::try_new(
            schema,
            vec![
                Arc::new(id_array),
                Arc::new(workflow_id_array),
                Arc::new(status_array),
                Arc::new(step_index_array),
                Arc::new(total_steps_array),
                Arc::new(agent_id_array),
                Arc::new(started_at_array),
                Arc::new(completed_at_array),
                Arc::new(context_array),
                Arc::new(metadata_array),
            ],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_memory_backend_save_restore() {
        let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();

        let data = SnapshotData {
            agents: create_test_agents_batch(),
            resources: create_test_resources_batch(),
            tasks: create_test_tasks_batch(),
            workflow_instances: create_test_instances_batch(),
            locations: None,
        };

        // Save snapshot
        manager.save_snapshot("test_1", data.clone()).await.unwrap();

        // Restore snapshot
        let restored = manager.restore_snapshot("test_1").await.unwrap();

        // Verify data
        assert_eq!(restored.agents.num_rows(), 1);
        assert_eq!(restored.resources.num_rows(), 1);
        assert_eq!(restored.tasks.num_rows(), 1);
        assert_eq!(restored.workflow_instances.num_rows(), 1);
        assert!(restored.locations.is_none());
    }

    #[tokio::test]
    async fn test_parquet_backend_save_restore() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_str().unwrap().to_string();

        let manager = SnapshotManager::new(SnapshotBackend::Parquet(base_path.clone()))
            .await
            .unwrap();

        let data = SnapshotData {
            agents: create_test_agents_batch(),
            resources: create_test_resources_batch(),
            tasks: create_test_tasks_batch(),
            workflow_instances: create_test_instances_batch(),
            locations: None,
        };

        // Save snapshot
        manager
            .save_snapshot("test_parquet", data.clone())
            .await
            .unwrap();

        // Verify files were created
        let snapshot_dir = PathBuf::from(&base_path).join("test_parquet");
        assert!(snapshot_dir.exists());
        assert!(snapshot_dir.join("agents.parquet").exists());
        assert!(snapshot_dir.join("resources.parquet").exists());
        assert!(snapshot_dir.join("tasks.parquet").exists());
        assert!(snapshot_dir.join("workflow_instances.parquet").exists());

        // Restore snapshot
        let restored = manager.restore_snapshot("test_parquet").await.unwrap();

        // Verify data
        assert_eq!(restored.agents.num_rows(), 1);
        assert_eq!(restored.resources.num_rows(), 1);
        assert_eq!(restored.tasks.num_rows(), 1);
        assert_eq!(restored.workflow_instances.num_rows(), 1);
        assert!(restored.locations.is_none());
    }

    #[tokio::test]
    async fn test_restore_nonexistent_snapshot() {
        let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();

        let result = manager.restore_snapshot("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_snapshots() {
        let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();

        let data1 = SnapshotData {
            agents: create_test_agents_batch(),
            resources: create_test_resources_batch(),
            tasks: create_test_tasks_batch(),
            workflow_instances: create_test_instances_batch(),
            locations: None,
        };

        let data2 = data1.clone();

        // Save two snapshots
        manager.save_snapshot("snapshot_1", data1).await.unwrap();
        manager.save_snapshot("snapshot_2", data2).await.unwrap();

        // Restore both
        let restored1 = manager.restore_snapshot("snapshot_1").await.unwrap();
        let restored2 = manager.restore_snapshot("snapshot_2").await.unwrap();

        assert_eq!(restored1.agents.num_rows(), 1);
        assert_eq!(restored2.agents.num_rows(), 1);
    }
}
