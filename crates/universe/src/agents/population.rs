use std::sync::{Arc, LazyLock};

use arrow::{
    array::{AsArray as _, RecordBatch},
    compute::concat_batches,
};
use arrow_schema::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::{
    functions::core::expr_ext::FieldAccessor as _,
    logical_expr::ScalarUDF,
    prelude::{DataFrame, Expr, array_element, case, cast, col, lit, random, round, when},
    scalar::ScalarValue,
};
use geo_traits::to_geo::ToGeoPoint;
use geoarrow::array::PointArray;
use geoarrow_array::GeoArrowArrayAccessor;
use geoarrow_schema::{CoordType, Dimension, LineStringType, PointType};
use h3o::{LatLng, Resolution};
use tracing::{Level, instrument};
use uuid::Uuid;

use crate::{
    BrandId, EntityView as _, EventPayload, MenuItemId, ObjectLabel, OrderCreatedPayload,
    PersonRole, PersonStatusFlag, Result, SimulationContext, SiteId, State,
    agents::functions::create_order, state::Transport,
};

pub struct PopulationRunner {
    create_orders: Arc<ScalarUDF>,
}

impl PopulationRunner {
    pub async fn try_new(ctx: &SimulationContext) -> Result<Self> {
        let batches = ctx
            .snapshots()
            .objects()
            .await?
            .filter(col("label").eq(lit(ObjectLabel::MenuItem.as_ref())))?
            .select([
                col("parent_id").alias("brand_id"),
                col("id").alias("menu_item_id"),
            ])?
            .collect()
            .await?;
        let order_choices = concat_batches(batches[0].schema_ref(), &batches)?;
        let create_orders = create_order(order_choices);
        Ok(PopulationRunner { create_orders })
    }

    #[instrument(
        name = "step_population",
        level = Level::TRACE,
        skip(self, ctx, state),
        fields(
            caspers.site_id = site_id.to_string()
        )
    )]
    pub(crate) async fn step(
        &self,
        ctx: &SimulationContext,
        site_id: &SiteId,
        state: &State,
    ) -> Result<impl Iterator<Item = EventPayload>> {
        let site = state.objects().site(site_id)?;
        let props = site.properties()?;
        let lat_lng = LatLng::new(props.latitude, props.longitude)?;
        let ts = state.current_time().timestamp_millis();

        let idle_people = state
            .population()
            .idle_people_in_cell(ctx, lat_lng.to_cell(Resolution::Six), &PersonRole::Customer)
            .await?
            .collect()
            .await?;

        let idle_people = ctx.ctx().read_batches(idle_people)?;

        let orders = idle_people
            .select(vec![
                col("id"),
                self.create_orders
                    .call(vec![
                        lit(ScalarValue::TimestampMillisecond(
                            Some(ts),
                            Some("UTC".into()),
                        )),
                        col("state"),
                    ])
                    .alias("order"),
                col("position"),
            ])?
            .filter(col("order").is_not_null())?
            .select_columns(&["id", "order", "position"])?
            .collect()
            .await?;

        let orders = orders.into_iter().flat_map(|o| {
            let positions: PointArray = (
                o.column(2).as_struct(),
                PointType::new(Dimension::XY, Default::default()),
            )
                .try_into()
                .unwrap();
            let orders_iter = o
                .column(0)
                .as_fixed_size_binary()
                .iter()
                .zip(o.column(1).as_list::<i32>().iter())
                .zip(positions.iter());
            let mut orders = Vec::new();

            for ((person_id, order), pos) in orders_iter {
                if let (Some(person_id), Some(order), Some(Ok(pos))) = (person_id, order, pos) {
                    let items = order
                        .as_fixed_size_list()
                        .iter()
                        .flat_map(|it| {
                            it.map(|it2| {
                                let arr = it2.as_fixed_size_binary();
                                (
                                    BrandId::from(Uuid::from_slice(arr.value(0)).unwrap()),
                                    MenuItemId::from(Uuid::from_slice(arr.value(1)).unwrap()),
                                )
                            })
                        })
                        .collect();
                    orders.push(EventPayload::OrderCreated(OrderCreatedPayload {
                        site_id: *site_id,
                        person_id: Uuid::from_slice(person_id).unwrap().into(),
                        items,
                        destination: pos.to_point(),
                    }));
                }
            }

            orders
        });

        Ok(orders)
    }
}

// ============================================================================
// PopulationHandler - Journey tracking using DataFusion
// ============================================================================

static JOURNEY_STATE: LazyLock<SchemaRef> = LazyLock::new(|| {
    SchemaRef::new(Schema::new(vec![
        Field::new("person_id", DataType::FixedSizeBinary(16), false),
        LineStringType::new(Dimension::XY, Default::default())
            .with_coord_type(CoordType::Separated)
            .to_field("journey", true),
        Field::new("distance_m", DataType::Float64, true),
        Field::new("progress", DataType::Float64, false),
    ]))
});

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PopulationStats {
    pub active_journeys: usize,
    pub completed_journeys: usize,
}

impl std::ops::Add for PopulationStats {
    type Output = PopulationStats;

    fn add(self, other: PopulationStats) -> PopulationStats {
        PopulationStats {
            active_journeys: self.active_journeys + other.active_journeys,
            completed_journeys: self.completed_journeys + other.completed_journeys,
        }
    }
}

#[derive(Clone)]
pub(crate) struct PopulationHandler {
    create_orders: Arc<ScalarUDF>,

    population: Vec<RecordBatch>,
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
        Ok(ctx.ctx().read_batches(self.population.iter().cloned())?)
    }

    pub(crate) fn journeys(&self, ctx: &SimulationContext) -> Result<DataFrame> {
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

    pub(crate) async fn get_stats(&self, ctx: &SimulationContext) -> Result<PopulationStats> {
        // Handle empty journeys
        if self.journeys.is_empty() || self.journeys.iter().all(|b| b.num_rows() == 0) {
            return Ok(PopulationStats {
                active_journeys: 0,
                completed_journeys: 0,
            });
        }

        let journeys = self.journeys(ctx)?;

        // Count active and completed journeys
        let stats_df = journeys
            .aggregate(
                vec![],
                vec![
                    datafusion::functions_aggregate::expr_fn::count(
                        datafusion::prelude::Expr::Case(datafusion::logical_expr::Case::new(
                            None,
                            vec![(
                                Box::new(col("is_complete").eq(lit(false))),
                                Box::new(lit(1_i64)),
                            )],
                            None,
                        )),
                    )
                    .alias("active"),
                    datafusion::functions_aggregate::expr_fn::count(
                        datafusion::prelude::Expr::Case(datafusion::logical_expr::Case::new(
                            None,
                            vec![(
                                Box::new(col("is_complete").eq(lit(true))),
                                Box::new(lit(1_i64)),
                            )],
                            None,
                        )),
                    )
                    .alias("completed"),
                ],
            )?
            .collect()
            .await?;

        let batch = &stats_df[0];
        let active = batch
            .column(0)
            .as_primitive::<arrow::datatypes::Int64Type>()
            .value(0) as usize;
        let completed = batch
            .column(1)
            .as_primitive::<arrow::datatypes::Int64Type>()
            .value(0) as usize;

        Ok(PopulationStats {
            active_journeys: active,
            completed_journeys: completed,
        })
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
        // Filter for completed journeys
        let completed = self
            .journeys(ctx)?
            .filter(col("is_complete").eq(lit(true)))?
            .collect()
            .await?;

        let completed_count = completed.iter().map(|b| b.num_rows()).sum::<usize>();
        if completed_count == 0 {
            return Ok(None);
        }

        // Remove completed journeys from active journeys
        self.journeys = self
            .journeys(ctx)?
            .filter(col("is_complete").eq(lit(false)))?
            .collect()
            .await?;

        Ok(Some(ctx.ctx().read_batches(completed)?))
    }

    pub(crate) async fn get_current_positions(&self, ctx: &SimulationContext) -> Result<DataFrame> {
        let journeys = self.journeys(ctx)?;

        // Calculate current position based on current_leg_index and current_leg_progress
        let current_leg_idx_expr = cast(col("current_leg_index") + lit(1_u64), DataType::Int64);
        let current_leg = array_element(col("journey_legs"), current_leg_idx_expr.clone());

        let current_dest_x = current_leg.clone().field("destination").field("x");
        let current_dest_y = current_leg.clone().field("destination").field("y");

        // Get previous position
        let prev_leg_idx_expr = cast(col("current_leg_index"), DataType::Int64);
        let prev_leg = array_element(col("journey_legs"), prev_leg_idx_expr.clone());
        let prev_dest_x = prev_leg.clone().field("destination").field("x");
        let prev_dest_y = prev_leg.clone().field("destination").field("y");

        let start_x = datafusion::prelude::Expr::Case(datafusion::logical_expr::Case::new(
            None,
            vec![(
                Box::new(col("current_leg_index").eq(lit(0_u64))),
                Box::new(col("origin").field("x")),
            )],
            Some(Box::new(prev_dest_x)),
        ));

        let start_y = datafusion::prelude::Expr::Case(datafusion::logical_expr::Case::new(
            None,
            vec![(
                Box::new(col("current_leg_index").eq(lit(0_u64))),
                Box::new(col("origin").field("y")),
            )],
            Some(Box::new(prev_dest_y)),
        ));

        let current_pos_x =
            start_x.clone() + (current_dest_x - start_x.clone()) * col("current_leg_progress");
        let current_pos_y =
            start_y.clone() + (current_dest_y - start_y.clone()) * col("current_leg_progress");

        // Create position struct
        Ok(journeys.select(vec![
            col("person_id"),
            datafusion::functions::core::expr_fn::named_struct(vec![
                lit("x"),
                current_pos_x,
                lit("y"),
                current_pos_y,
            ])
            .alias("position"),
        ])?)
    }
}
