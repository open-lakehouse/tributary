use std::sync::LazyLock;

use arrow::array::RecordBatch;
use datafusion::dataframe::DataFrameWriteOptions;
use datafusion::logical_expr::dml::InsertOp;
use datafusion::prelude::{DataFrame, lit};
use datafusion::scalar::ScalarValue;
use datafusion::sql::TableReference;
use itertools::Itertools;
use uuid::Uuid;

use crate::Result;
use crate::context::SimulationContext;

use super::system::{SNAPSHOT_META_REF, SnapshotMetaBuilder};

pub struct SnapshotsSchema<'a> {
    ctx: &'a SimulationContext,
}

pub(in crate::context) static SNAPSHOTS_SCHEMA_NAME: &str = "snapshots";
pub(in crate::context) static POPULATION_REF: LazyLock<TableReference> =
    LazyLock::new(|| TableReference::full("caspers", SNAPSHOTS_SCHEMA_NAME, "population"));
pub(in crate::context) static OBJECTS_REF: LazyLock<TableReference> =
    LazyLock::new(|| TableReference::full("caspers", SNAPSHOTS_SCHEMA_NAME, "objects"));
pub(in crate::context) static ORDERS_REF: LazyLock<TableReference> =
    LazyLock::new(|| TableReference::full("caspers", SNAPSHOTS_SCHEMA_NAME, "orders"));
pub(in crate::context) static ORDER_LINES_REF: LazyLock<TableReference> =
    LazyLock::new(|| TableReference::full("caspers", SNAPSHOTS_SCHEMA_NAME, "order_lines"));

impl<'a> SnapshotsSchema<'a> {
    pub(in crate::context) fn new(ctx: &'a SimulationContext) -> Self {
        Self { ctx }
    }

    pub async fn objects(&self) -> Result<DataFrame> {
        static COLUMNS: &[&str; 5] = &["id", "parent_id", "label", "name", "properties"];
        Ok(self
            .ctx
            .scan_scoped(&OBJECTS_REF)
            .await?
            .select_columns(COLUMNS)?)
    }

    pub async fn population(&self) -> Result<DataFrame> {
        static COLUMNS: &[&str; 6] = &["id", "role", "status", "properties", "position", "state"];
        Ok(self
            .ctx
            .scan_scoped(&POPULATION_REF)
            .await?
            .select_columns(COLUMNS)?)
    }

    pub async fn orders(&self) -> Result<DataFrame> {
        static COLUMNS: &[&str] = &[
            "person_id",
            "order_id",
            "site_id",
            "submitted_at",
            "destination",
            "status",
        ];
        Ok(self
            .ctx
            .scan_scoped(&ORDERS_REF)
            .await?
            .select_columns(COLUMNS)?)
    }

    pub async fn order_lines(&self) -> Result<DataFrame> {
        static COLUMNS: &[&str] = &[
            "order_id",
            "order_line_id",
            "menu_item_id",
            "kitchen_id",
            "submitted_at",
            "current_step",
            "total_steps",
            "assigned_to",
            "step_completion_time",
            "is_complete",
        ];
        Ok(self
            .ctx
            .scan_scoped(&ORDER_LINES_REF)
            .await?
            .select_columns(COLUMNS)?)
    }
}

/// Create a snapshot from handler-owned batches.
///
/// When `objects` is `Some`, the provided batch is written directly (used for
/// initial snapshot creation). When `None`, objects are copied from the current
/// snapshot (they are immutable during a run).
pub(crate) async fn create_snapshot_from_batches(
    ctx: &SimulationContext,
    objects: Option<&RecordBatch>,
    population: &[RecordBatch],
    orders: &[RecordBatch],
    order_lines: &[RecordBatch],
) -> Result<Uuid> {
    let snapshot_id = Uuid::now_v7();
    let id_val = ScalarValue::Utf8View(Some(snapshot_id.to_string()));
    let sim_id_val = ScalarValue::Utf8View(Some(ctx.simulation_id().to_string()));

    let append_cols = |df: DataFrame| -> Result<DataFrame> {
        Ok(df
            .with_column("simulation_id", lit(sim_id_val.clone()))?
            .with_column("snapshot_id", lit(id_val.clone()))?)
    };

    let mut tasks_defs = vec![];

    // Objects: use provided batch or copy from current snapshot
    if let Some(batch) = objects {
        if batch.num_rows() > 0 {
            let df = ctx.ctx().read_batch(batch.clone())?;
            tasks_defs.push((OBJECTS_REF.to_string(), append_cols(df)?));
        }
    } else {
        let df_objects = ctx.snapshots().objects().await?;
        if df_objects.clone().count().await? > 0 {
            tasks_defs.push((OBJECTS_REF.to_string(), append_cols(df_objects)?));
        }
    }

    // Population
    let total_pop: usize = population.iter().map(|b| b.num_rows()).sum();
    if total_pop > 0 {
        let df = ctx.ctx().read_batches(population.iter().cloned())?;
        tasks_defs.push((POPULATION_REF.to_string(), append_cols(df)?));
    }

    // Orders
    let total_orders: usize = orders.iter().map(|b| b.num_rows()).sum();
    if total_orders > 0 {
        let df = ctx.ctx().read_batches(orders.iter().cloned())?;
        tasks_defs.push((ORDERS_REF.to_string(), append_cols(df)?));
    }

    // Order lines
    let total_lines: usize = order_lines.iter().map(|b| b.num_rows()).sum();
    if total_lines > 0 {
        let df = ctx.ctx().read_batches(order_lines.iter().cloned())?;
        tasks_defs.push((ORDER_LINES_REF.to_string(), append_cols(df)?));
    }

    // Snapshot metadata
    let mut batch_sn = SnapshotMetaBuilder::new();
    batch_sn.add_snapshot(&snapshot_id, ctx.simulation_id(), *ctx.current_time(), None);
    let df_sn = ctx.ctx().read_batch(batch_sn.build()?)?;
    tasks_defs.push((SNAPSHOT_META_REF.to_string(), df_sn));

    let write_table = async |df: DataFrame, table_name: String| {
        let write_options =
            DataFrameWriteOptions::default().with_insert_operation(InsertOp::Append);
        df.write_table(table_name.as_str(), write_options).await
    };

    let tasks = tasks_defs
        .into_iter()
        .map(|(table_name, df)| write_table(df, table_name))
        .collect::<Vec<_>>();

    let _results: Vec<_> = futures::future::join_all(tasks)
        .await
        .into_iter()
        .try_collect()?;

    Ok(snapshot_id)
}
