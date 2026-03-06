use std::collections::HashMap;

use std::sync::Arc;

use arrow::array::RecordBatch;
use arrow_schema::SchemaRef;
use chrono::{DateTime, Utc};
use datafusion::{
    functions_aggregate::count::count,
    prelude::{DataFrame, SessionContext, col, lit},
};

use crate::Result;

use super::population_event_schemas::EVENT_SCHEMA;

/// Accumulated event log from a simulation run.
///
/// Stores all events as Arrow RecordBatches (one per step that produced events)
/// and provides DataFrame-based inspection APIs.
pub struct EventLog {
    batches: Vec<RecordBatch>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
        }
    }

    pub fn from_batches(batches: Vec<RecordBatch>) -> Self {
        Self { batches }
    }

    pub fn append_batches(&mut self, batches: impl IntoIterator<Item = RecordBatch>) {
        self.batches.extend(batches);
    }

    pub fn num_rows(&self) -> usize {
        self.batches.iter().map(|b| b.num_rows()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.num_rows() == 0
    }

    pub fn to_dataframe(&self, ctx: &SessionContext) -> Result<DataFrame> {
        if self.batches.is_empty() {
            let empty = RecordBatch::new_empty(EVENT_SCHEMA.clone());
            return Ok(ctx.read_batch(empty)?);
        }
        // Batches from different steps may have varying nullability or coerced
        // types (e.g. Int32 vs Decimal128 for step_index). Group by schema and
        // concat within each group, then UNION ALL across groups so DataFusion
        // handles the type coercion.
        let groups = group_by_schema(&self.batches);
        let mut df: Option<DataFrame> = None;
        for (_, batches) in groups {
            let group_df = ctx.read_batches(batches.iter().cloned())?;
            df = Some(match df {
                Some(existing) => existing.union(group_df)?,
                None => group_df,
            });
        }
        Ok(df.unwrap())
    }

    pub fn by_type(&self, ctx: &SessionContext, event_type: &str) -> Result<DataFrame> {
        Ok(self
            .to_dataframe(ctx)?
            .filter(col("type").eq(lit(event_type)))?)
    }

    pub async fn count_by_type(
        &self,
        ctx: &SessionContext,
    ) -> Result<HashMap<String, u64>> {
        let df = self
            .to_dataframe(ctx)?
            .aggregate(vec![col("type")], vec![count(col("type")).alias("cnt")])?;

        let batches = df.collect().await?;
        let mut map = HashMap::new();
        for batch in &batches {
            let types = batch
                .column_by_name("type")
                .expect("type column")
                .as_any()
                .downcast_ref::<arrow::array::StringArray>()
                .expect("type is StringArray");
            let counts = batch
                .column_by_name("cnt")
                .expect("cnt column")
                .as_any()
                .downcast_ref::<arrow::array::Int64Array>()
                .expect("cnt is Int64Array");
            for i in 0..batch.num_rows() {
                map.insert(types.value(i).to_string(), counts.value(i) as u64);
            }
        }
        Ok(map)
    }
}

fn group_by_schema(batches: &[RecordBatch]) -> Vec<(SchemaRef, Vec<RecordBatch>)> {
    let mut groups: Vec<(SchemaRef, Vec<RecordBatch>)> = Vec::new();
    for batch in batches {
        let schema = batch.schema();
        if let Some(group) = groups.iter_mut().find(|(s, _)| Arc::ptr_eq(s, &schema)) {
            group.1.push(batch.clone());
        } else {
            groups.push((schema, vec![batch.clone()]));
        }
    }
    groups
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a simulation run, containing the accumulated event log and metadata.
pub struct SimulationRunResult {
    event_log: EventLog,
    steps: usize,
    final_time: DateTime<Utc>,
}

impl SimulationRunResult {
    pub(super) fn new(event_log: EventLog, steps: usize, final_time: DateTime<Utc>) -> Self {
        Self {
            event_log,
            steps,
            final_time,
        }
    }

    pub fn event_log(&self) -> &EventLog {
        &self.event_log
    }

    pub fn steps(&self) -> usize {
        self.steps
    }

    pub fn final_time(&self) -> &DateTime<Utc> {
        &self.final_time
    }

    pub fn into_event_log(self) -> EventLog {
        self.event_log
    }
}

#[cfg(test)]
mod tests {
    use rstest::*;

    use super::*;
    use crate::test_utils::runner_fixed;
    use crate::{SimulationRunner, agents::functions::OrderSpec};

    #[tokio::test]
    async fn test_new_is_empty() {
        let log = EventLog::new();
        assert!(log.is_empty());
        assert_eq!(log.num_rows(), 0);
    }

    #[tokio::test]
    async fn test_empty_to_dataframe() -> Result<()> {
        let ctx = SessionContext::new();
        let log = EventLog::new();
        let df = log.to_dataframe(&ctx)?;
        assert_eq!(df.count().await?, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_empty_count_by_type() -> Result<()> {
        let ctx = SessionContext::new();
        let log = EventLog::new();
        let counts = log.count_by_type(&ctx).await?;
        assert!(counts.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_from_batches_empty_batch() -> Result<()> {
        let empty = RecordBatch::new_empty(EVENT_SCHEMA.clone());
        let log = EventLog::from_batches(vec![empty]);
        assert!(log.is_empty());
        assert_eq!(log.num_rows(), 0);

        let ctx = SessionContext::new();
        assert_eq!(log.to_dataframe(&ctx)?.count().await?, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_append_batches() {
        let mut log = EventLog::new();
        let empty = RecordBatch::new_empty(EVENT_SCHEMA.clone());
        log.append_batches(vec![empty]);
        assert!(log.is_empty());
    }

    #[rstest]
    #[tokio::test]
    async fn test_event_log_from_run(
        #[future]
        #[with(OrderSpec::Once(vec![1, 2, 3]))]
        runner_fixed: Result<SimulationRunner>,
    ) -> Result<()> {
        let mut runner = runner_fixed.await?;
        let result = runner.run(50).await?;

        assert_eq!(result.steps(), 50);
        assert!(!result.event_log().is_empty());

        let ctx = runner.ctx.ctx();
        let counts = result.event_log().count_by_type(ctx).await?;
        assert!(
            !counts.is_empty(),
            "should have at least one event type after 50 steps"
        );

        let order_created_count = counts
            .get("caspers.universe.order_created")
            .copied()
            .unwrap_or(0);
        assert!(
            order_created_count > 0,
            "expected order_created events, got {counts:?}"
        );

        let created_df = result
            .event_log()
            .by_type(ctx, "caspers.universe.order_created")?;
        let created_rows = created_df.count().await?;
        assert_eq!(created_rows as u64, order_created_count);

        let all_rows = result.event_log().to_dataframe(ctx)?.count().await?;
        let total_from_counts: u64 = counts.values().sum();
        assert_eq!(all_rows as u64, total_from_counts);

        Ok(())
    }
}
