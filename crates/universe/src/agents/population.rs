use std::sync::{Arc, LazyLock};

use arrow::array::RecordBatch;
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::{
    common::JoinType,
    logical_expr::{ExprSchemable as _, ScalarUDF},
    prelude::{
        DataFrame, Expr, array_element, array_length, case, cast, col, lit, random, round, when,
    },
    scalar::ScalarValue,
};
use geoarrow_schema::{CoordType, Dimension, LineStringType};
use itertools::Itertools as _;

use crate::{
    POPULATION_SCHEMA, PersonRole, PersonStatusFlag, Result, SimulationContext,
    functions::to_point, state::Transport,
};

static JOURNEY_STATE: LazyLock<SchemaRef> = LazyLock::new(|| {
    SchemaRef::new(Schema::new(vec![
        Field::new("person_id", DataType::FixedSizeBinary(16), false),
        LineStringType::new(Dimension::XY, Default::default())
            .with_coord_type(CoordType::Separated)
            .to_field("journey", true),
        Field::new("distance_m", DataType::Float64, true),
        Field::new("order_id", DataType::FixedSizeBinary(16), true),
        Field::new("progress", DataType::Float64, false),
    ]))
});

#[derive(Clone)]
pub(crate) struct PopulationHandler {
    create_orders: Arc<ScalarUDF>,

    pub(crate) population: Vec<RecordBatch>,
    journeys: Vec<RecordBatch>,
}

impl PopulationHandler {
    pub(crate) async fn try_new(
        ctx: &SimulationContext,
        create_orders: Arc<ScalarUDF>,
    ) -> Result<Self> {
        let population = ctx.snapshots().population().await?.collect().await?;
        Ok(PopulationHandler {
            create_orders,
            population,
            journeys: vec![RecordBatch::new_empty(JOURNEY_STATE.clone())],
        })
    }

    pub(crate) fn population(&self, ctx: &SimulationContext) -> Result<DataFrame> {
        if self.population.is_empty() {
            return Ok(ctx
                .ctx()
                .read_batch(RecordBatch::new_empty(POPULATION_SCHEMA.clone()))?);
        }
        Ok(ctx.ctx().read_batches(self.population.iter().cloned())?)
    }

    pub(crate) fn journeys(&self, ctx: &SimulationContext) -> Result<DataFrame> {
        if self.journeys.is_empty() {
            return Ok(ctx
                .ctx()
                .read_batch(RecordBatch::new_empty(JOURNEY_STATE.clone()))?);
        }
        Ok(ctx.ctx().read_batches(self.journeys.iter().cloned())?)
    }

    pub(crate) async fn create_orders(&self, ctx: &SimulationContext) -> Result<DataFrame> {
        // TODO: adjust timestamp passed to create order function to local time. Internally
        // we always process against UTC so we just need adjust the current time to local time
        // and "pretend" its this time in UTC. The the timestamp passed to the uuidv7 function
        // should NOT be adjusted however, as we just need it to be globally ordered...
        let current_time = lit(ScalarValue::TimestampMillisecond(
            Some(ctx.current_time().timestamp_millis()),
            Some("UTC".into()),
        ));

        let step_duration = lit(ScalarValue::Float64(Some(
            ctx.time_step().as_millis() as f64
        )));

        let submitted_at_expression = current_time.clone()
            + cast(
                round(vec![step_duration * random()]),
                DataType::Duration(TimeUnit::Millisecond),
            );

        let new_orders = self
            .population(ctx)?
            .filter(
                col("status")
                    .eq(lit(PersonStatusFlag::Idle.as_ref()))
                    .and(col("role").eq(lit(PersonRole::Customer.as_ref()))),
            )?
            .select(vec![
                col("id").alias("person_id"),
                submitted_at_expression.alias("submitted_at"),
                col("position").alias("destination"),
                self.create_orders
                    .call(vec![current_time.clone(), col("state")])
                    .alias("items"),
            ])?
            .filter(col("items").is_not_null())?
            .select([
                col("person_id"),
                col("submitted_at"),
                col("destination"),
                col("items"),
            ])?;

        Ok(new_orders)
    }

    pub(crate) async fn set_person_status(
        &mut self,
        ctx: &SimulationContext,
        person_ids: Vec<Expr>,
        status: PersonStatusFlag,
    ) -> Result<()> {
        self.population = self
            .population(ctx)?
            .select(vec![
                col("id"),
                col("role"),
                case(col("id").in_list(person_ids.clone(), false))
                    .when(lit(true), lit(status.as_ref()))
                    .otherwise(col("status"))?
                    .alias("status"),
                col("properties"),
                col("position"),
                col("state"),
            ])?
            .collect()
            .await?;
        Ok(())
    }

    pub(crate) async fn start_journeys(
        &mut self,
        ctx: &SimulationContext,
        new_journeys: DataFrame,
    ) -> Result<()> {
        let new_journeys = new_journeys.with_column("progress", lit(0.0_f64))?;
        self.journeys = self.journeys(ctx)?.union(new_journeys)?.collect().await?;
        Ok(())
    }

    pub(crate) async fn advance_journeys(&mut self, ctx: &SimulationContext) -> Result<()> {
        // Early return if no journeys to process
        if self.journeys.is_empty() || self.journeys.iter().all(|b| b.num_rows() == 0) {
            return Ok(());
        }

        let time_step_seconds = lit(ctx.time_step().as_secs_f64());
        let velocity_m_s = lit(Transport::default().default_velocity_m_s());
        let progress_update = velocity_m_s * time_step_seconds / col("distance_m");

        let next_progress = when(
            (col("progress") + progress_update.clone()).lt(lit(1.0_f64)),
            col("progress") + progress_update,
        )
        .otherwise(lit(1.0_f64))?;

        self.journeys = self
            .journeys(ctx)?
            .with_column("progress", next_progress)?
            .collect()
            .await?;

        Ok(())
    }

    pub(crate) async fn get_completed_journeys(
        &mut self,
        ctx: &SimulationContext,
    ) -> Result<Option<DataFrame>> {
        if self.journeys.is_empty() || self.journeys.iter().all(|b| b.num_rows() == 0) {
            return Ok(None);
        }

        // Filter for completed journeys
        let completed = self
            .journeys(ctx)?
            .filter(col("progress").gt_eq(lit(1.0_f64)))?
            .collect()
            .await?;

        let completed_count = completed.iter().map(|b| b.num_rows()).sum::<usize>();
        if completed_count == 0 {
            return Ok(None);
        }

        let completed = ctx.ctx().read_batches(completed)?;

        let new_positions = completed.clone().select(vec![
            col("person_id"),
            array_element(
                col("journey"),
                array_length(col("journey")).cast_to(&DataType::Int64, completed.schema())?,
            )
            .alias("new_position"),
        ])?;

        // update population with new positions
        let updated_population = self
            .population(ctx)?
            .join_on(
                new_positions,
                JoinType::Left,
                vec![col("id").eq(col("person_id"))],
            )?
            .select(vec![
                col("id"),
                col("role"),
                col("status"),
                col("properties"),
                to_point()
                    .call(vec![
                        when(col("new_position").is_not_null(), col("new_position"))
                            .otherwise(col("position"))?,
                    ])
                    .alias("position"),
                col("state"),
            ])?
            .collect()
            .await?;

        let updated_population: Vec<_> = updated_population
            .into_iter()
            .map(|b| b.with_schema(POPULATION_SCHEMA.clone()))
            .try_collect()?;

        self.population = updated_population;

        // Remove completed journeys from active journeys
        self.journeys = self
            .journeys(ctx)?
            .filter(col("progress").lt(lit(1.0_f64)))?
            .collect()
            .await?;

        Ok(Some(completed))
    }
}
