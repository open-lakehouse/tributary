use std::sync::Arc;

use arrow::{
    array::{AsArray, RecordBatch},
    compute::concat_batches,
    datatypes::Int64Type,
    util::pretty::print_batches,
};
use arrow_schema::DataType;
use datafusion::{
    common::{HashSet, JoinType},
    functions::core::expr_ext::FieldAccessor,
    functions_aggregate::{count::count_all, expr_fn::array_agg},
    functions_window::expr_fn::row_number,
    logical_expr::{ExprSchemable, ScalarUDF},
    prelude::{DataFrame, array_element, array_length, cast, col, lit},
    scalar::ScalarValue,
};
use itertools::Itertools;

use crate::{
    MovementHandler, ObjectLabel, OrderStatus, PersonRole, PersonStatusFlag, Result,
    SimulationContext,
    agents::{KitchenHandler, PopulationHandler, functions::create_order},
    functions::h3_longlatash3,
};

use self::event_log::{EventLog, SimulationRunResult};
pub use self::events::*;
pub use self::population_event_schemas::*;
use crate::context::schemas::snapshots::create_snapshot_from_batches;

mod event_log;
mod events;
mod population_event_schemas;

pub struct SimulationRunnerBuilder {
    ctx: SimulationContext,

    create_orders: Option<Box<dyn Fn(RecordBatch) -> Arc<ScalarUDF>>>,
    dry_run: bool,
}

impl From<SimulationContext> for SimulationRunnerBuilder {
    fn from(ctx: SimulationContext) -> Self {
        Self {
            ctx,
            create_orders: None,
            dry_run: false,
        }
    }
}

impl SimulationRunnerBuilder {
    pub fn new(ctx: SimulationContext) -> Self {
        ctx.into()
    }

    pub fn with_create_orders(
        mut self,
        create_orders: Box<dyn Fn(RecordBatch) -> Arc<ScalarUDF>>,
    ) -> Self {
        self.create_orders = Some(create_orders);
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub async fn build(self) -> Result<SimulationRunner> {
        let batches = self
            .ctx
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

        let create_orders = if let Some(create) = self.create_orders {
            create(order_choices)
        } else {
            create_order(order_choices)
        };

        let population = PopulationHandler::try_new(&self.ctx, create_orders).await?;
        let kitchens = KitchenHandler::try_new(&self.ctx).await?;
        let movement = MovementHandler::try_new(&self.ctx).await?;

        Ok(SimulationRunner {
            ctx: self.ctx,
            population,
            kitchens,
            movement,
            dry_run: self.dry_run,
        })
    }
}

pub struct SimulationRunner {
    pub(crate) ctx: SimulationContext,

    pub(crate) population: PopulationHandler,
    pub(crate) kitchens: KitchenHandler,
    pub(crate) movement: MovementHandler,

    dry_run: bool,
}

impl SimulationRunner {
    pub async fn run(&mut self, steps: usize) -> Result<SimulationRunResult> {
        tracing::info!(
            target: "caspers::simulation",
            "starting simulation run for {} steps ({} / {})",
            steps,
            self.ctx.simulation_id(),
            self.ctx.snapshot_id()
        );

        let mut event_log = EventLog::new();
        for _ in 0..steps {
            let events = self.step().await?;
            let batches = events.collect().await?;
            event_log.append_batches(batches);
        }

        if !self.dry_run {
            self.create_snapshot().await?;
        }

        Ok(SimulationRunResult::new(
            event_log,
            steps,
            *self.ctx.current_time(),
        ))
    }

    pub async fn step(&mut self) -> Result<DataFrame> {
        let mut events = EventsHelper::empty(&self.ctx)?;

        if let Some(journey_events) = self.handle_journeys().await? {
            events = events.union(journey_events)?;
        }

        if let Some(delivery_events) = self.start_deliveries().await? {
            events = events.union(delivery_events)?;
        }

        let orders = self
            .population
            .create_orders(&self.ctx)
            .await?
            .cache()
            .await?;

        let orders_count = orders.clone().count().await?;
        let kitchen_events = if orders_count > 0 {
            self.kitchens.step(&self.ctx, Some(orders)).await?
        } else {
            self.kitchens.step(&self.ctx, None).await?
        };
        events = events.union(kitchen_events)?;

        self.send_events(events.clone()).await?;

        self.ctx.step_time();

        Ok(events)
    }

    async fn handle_journeys(&mut self) -> Result<Option<DataFrame>> {
        self.population.advance_journeys(&self.ctx).await?;

        let Some(completed) = self.population.get_completed_journeys(&self.ctx).await? else {
            return Ok(None);
        };

        let completed = completed
            .clone()
            .join_on(
                self.population
                    .population(&self.ctx)?
                    .select_columns(&["id", "status"])?,
                JoinType::Left,
                vec![col("person_id").eq(col("id"))],
            )?
            .cache()
            .await?;

        let needs_return = completed
            .clone()
            .filter(col("status").eq(lit(PersonStatusFlag::Delivering.as_ref())))?
            .cache()
            .await?;

        let finished_journeys = completed
            .clone()
            .filter(col("status").not_eq(lit(PersonStatusFlag::Delivering.as_ref())))?;

        // Mark orders as Delivered for completed delivery journeys.
        let delivered_order_ids = needs_return
            .clone()
            .select([col("order_id")])?
            .collect()
            .await?
            .iter()
            .flat_map(|b| b.column(0).as_fixed_size_binary().iter().flatten())
            .map(|val| lit(ScalarValue::FixedSizeBinary(16, Some(val.to_vec()))))
            .collect_vec();
        if !delivered_order_ids.is_empty() {
            self.kitchens
                .set_order_status(&self.ctx, delivered_order_ids, OrderStatus::Delivered)
                .await?;
        }

        // Start return journeys for couriers that just completed deliveries.
        let return_trips = needs_return.clone().select([
            col("person_id"),
            array_element(
                col("journey"),
                array_length(col("journey")).cast_to(&DataType::Int64, completed.schema())?,
            )
            .alias("origin"),
            array_element(col("journey"), lit(1_i64)).alias("destination"),
        ])?;

        let delivering_person_ids = needs_return
            .clone()
            .select([col("person_id")])?
            .collect()
            .await?
            .iter()
            .flat_map(|b| b.column(0).as_fixed_size_binary().iter().flatten())
            .map(|val| lit(ScalarValue::FixedSizeBinary(16, Some(val.to_vec()))))
            .collect_vec();

        if let Some(journeys) = self.movement.plan_journeys(&self.ctx, return_trips).await? {
            let journeys =
                journeys.with_column("order_id", lit(ScalarValue::FixedSizeBinary(16, None)))?;
            self.population.start_journeys(&self.ctx, journeys).await?;
        }

        // Transition couriers away from Delivering regardless of whether a
        // return journey was planned (routing can fail for edge locations).
        if !delivering_person_ids.is_empty() {
            self.population
                .set_person_status(&self.ctx, delivering_person_ids, PersonStatusFlag::Moving)
                .await?;
        }

        // Set couriers back to Idle when their return trips complete.
        let returned_person_ids = finished_journeys
            .select([col("person_id")])?
            .collect()
            .await?
            .iter()
            .flat_map(|b| b.column(0).as_fixed_size_binary().iter().flatten())
            .map(|val| lit(ScalarValue::FixedSizeBinary(16, Some(val.to_vec()))))
            .collect_vec();
        if !returned_person_ids.is_empty() {
            self.population
                .set_person_status(&self.ctx, returned_person_ids, PersonStatusFlag::Idle)
                .await?;
        }

        // Only emit order_delivered for actual deliveries, not return trips.
        let needs_return_count = needs_return.clone().count().await?;
        if needs_return_count > 0 {
            let events = EventsHelper::order_delivered(needs_return.clone().select([
                col("person_id"),
                col("order_id"),
                self.ctx.current_time_expr().alias("timestamp"),
            ])?)?;
            Ok(Some(events))
        } else {
            Ok(None)
        }
    }

    async fn start_deliveries(&mut self) -> Result<Option<DataFrame>> {
        let Some(picked_up_orders) = self.assign_couriers().await? else {
            return Ok(None);
        };

        let picked_up_orders = picked_up_orders.cache().await?;
        let person_ids = picked_up_orders
            .clone()
            .select([col("assigned_courier")])?
            .collect()
            .await?
            .iter()
            .flat_map(|b| b.column(0).as_fixed_size_binary().iter().flatten())
            .map(|val| lit(ScalarValue::FixedSizeBinary(16, Some(val.to_vec()))))
            .collect_vec();
        self.population
            .set_person_status(&self.ctx, person_ids, PersonStatusFlag::Delivering)
            .await?;

        // Transition order status from Ready to PickedUp.
        let order_ids = picked_up_orders
            .clone()
            .select([col("order_id")])?
            .collect()
            .await?
            .iter()
            .flat_map(|b| b.column(0).as_fixed_size_binary().iter().flatten())
            .map(|val| lit(ScalarValue::FixedSizeBinary(16, Some(val.to_vec()))))
            .collect_vec();
        if !order_ids.is_empty() {
            self.kitchens
                .set_order_status(&self.ctx, order_ids, OrderStatus::PickedUp)
                .await?;
        }

        let journey_data = picked_up_orders.clone().select([
            col("assigned_courier").alias("person_id"),
            col("origin"),
            col("destination"),
        ])?;
        let maybe_journeys = self.movement.plan_journeys(&self.ctx, journey_data).await?;

        if let Some(journeys) = maybe_journeys {
            let journeys = journeys
                .join_on(
                    picked_up_orders
                        .clone()
                        .select([col("assigned_courier"), col("order_id")])?,
                    JoinType::Left,
                    vec![col("person_id").eq(col("assigned_courier"))],
                )?
                .drop_columns(&["assigned_courier"])?;
            self.population.start_journeys(&self.ctx, journeys).await?;
        }

        Some(EventsHelper::order_picked_up(
            picked_up_orders.clone().select([
                col("site_id"),
                col("assigned_courier").alias("courier_id"),
                col("order_id"),
                self.ctx.current_time_expr().alias("timestamp"),
            ])?,
        ))
        .transpose()
    }

    /// Assign courires to ready order for pickup.
    ///
    /// The returned DataFrame has the following schema:
    ///
    /// ```ignore
    /// {
    ///   site_id: bytes(16)
    ///   order_id: bytes(16)
    ///   submitted_at: timestamp[ms]
    ///   start_position: {
    ///     x: float64
    ///     y: float64
    ///   }
    ///   destination: {
    ///     x: float64
    ///     y: float64
    ///   }
    ///   assigned_courier: bytes(16)
    /// }
    /// ```
    async fn assign_couriers(&mut self) -> Result<Option<DataFrame>> {
        // TODO: more explicitly handle this when we consolidate location handling.
        let resolution = lit(9_i8);
        let Some(order_for_pickup) = self.kitchens.ready_orders(&self.ctx)? else {
            return Ok(None);
        };

        let order_for_pickup = order_for_pickup
            .select([
                col("site_id"),
                col("order_id"),
                col("submitted_at"),
                col("origin"),
                col("destination"),
                h3_longlatash3()
                    .call(vec![
                        col("origin").field("x"),
                        col("origin").field("y"),
                        resolution.clone(),
                    ])
                    .alias("location"),
            ])?
            .sort(vec![
                col("location").sort(true, false),
                col("submitted_at").sort(true, false),
            ])?
            .select([
                col("site_id"),
                col("order_id"),
                col("submitted_at"),
                col("origin"),
                col("destination"),
                col("location"),
                row_number().alias("driver_queue_pos"),
            ])?
            .cache()
            .await?;

        let batches = order_for_pickup.clone().collect().await?;
        let mut locations = HashSet::new();
        for batch in batches.iter() {
            for loc in batch
                .column_by_name("location")
                .expect("location")
                .as_primitive::<Int64Type>()
                .iter()
                .flatten()
            {
                locations.insert(loc);
            }
        }
        let locations = locations.into_iter().map(lit).collect_vec();

        if locations.is_empty() {
            return Ok(None);
        }

        let available_couriers = self
            .population
            .population(&self.ctx)?
            // TODO: This should filter on check-in once we have a check-in status implemented.
            .filter(
                col("role")
                    .eq(lit(PersonRole::Courier.as_ref()))
                    .and(col("status").eq(lit(PersonStatusFlag::Idle.as_ref()))),
            )?
            .select([
                col("id").alias("person_id"),
                h3_longlatash3()
                    .call(vec![
                        col("position").field("x"),
                        col("position").field("y"),
                        resolution,
                    ])
                    .alias("location"),
            ])?
            .filter(col("location").in_list(locations, false))?
            .aggregate(
                vec![col("location")],
                vec![array_agg(col("person_id")).alias("couriers")],
            )?
            .select([col("location").alias("location_queue"), col("couriers")])?;

        let assigned_orders = order_for_pickup
            .join_on(
                available_couriers,
                JoinType::Left,
                [col("location").eq(col("location_queue"))],
            )?
            .select([
                col("site_id"),
                col("order_id"),
                col("submitted_at"),
                col("origin"),
                col("destination"),
                col("driver_queue_pos"),
                col("couriers"),
            ])?
            .filter(col("driver_queue_pos").lt_eq(array_length(col("couriers"))))?
            .select([
                col("site_id"),
                col("order_id"),
                col("submitted_at"),
                col("origin"),
                col("destination"),
                array_element(
                    col("couriers"),
                    cast(col("driver_queue_pos"), DataType::Int32),
                )
                .alias("assigned_courier"),
            ])?;

        Ok(Some(assigned_orders))
    }

    async fn send_events(&self, events: DataFrame) -> Result<()> {
        let aggreagte = events
            .clone()
            .aggregate(vec![col("type")], vec![count_all()])?;
        let type_count = aggreagte.clone().count().await?;
        if type_count > 0 {
            let agg_batches = aggreagte.collect().await?;
            print_batches(&agg_batches)?;
        }
        Ok(())
    }

    /// Persist the current handler state as a new snapshot.
    ///
    /// Returns the new snapshot ID. The caller decides when to call this
    /// (e.g. after `run`, periodically via `step`, or on a schedule).
    pub async fn create_snapshot(&mut self) -> Result<uuid::Uuid> {
        let snapshot_id = create_snapshot_from_batches(
            &self.ctx,
            None,
            &self.population.population,
            &self.kitchens.orders,
            &self.kitchens.order_lines,
        )
        .await?;
        self.ctx.set_snapshot_id(snapshot_id);
        Ok(snapshot_id)
    }

    #[cfg(test)]
    pub(crate) fn ctx(&self) -> &SimulationContext {
        &self.ctx
    }

    #[cfg(test)]
    pub(crate) fn population(&self) -> &PopulationHandler {
        &self.population
    }

    #[cfg(test)]
    pub(crate) fn kitchens(&self) -> &KitchenHandler {
        &self.kitchens
    }

    #[cfg(test)]
    pub(crate) fn advance_time(&mut self) {
        self.ctx.step_time();
    }
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod lifecycle_tests;

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use rstest::*;

    use super::*;
    use crate::agents::functions::OrderSpec;
    use crate::test_utils::{runner, runner_fixed};

    #[rstest]
    #[tokio::test]
    async fn test_simulation_step(#[future] runner: Result<SimulationRunner>) -> Result<()> {
        let mut runner = runner.await?;

        let _result = runner.run(300).await?;

        Ok(())
    }

    #[rstest]
    #[tokio::test]
    async fn test_snapshot_roundtrip(
        #[future]
        #[with(OrderSpec::Once(vec![1, 2]))]
        runner_fixed: Result<SimulationRunner>,
    ) -> Result<()> {
        let mut runner = runner_fixed.await?;

        let result = runner.run(50).await?;

        let orders_before: usize = runner.kitchens().orders.iter().map(|b| b.num_rows()).sum();
        let lines_before: usize = runner
            .kitchens()
            .order_lines
            .iter()
            .map(|b| b.num_rows())
            .sum();
        let time_after_run = *runner.ctx().current_time();

        assert!(
            orders_before > 0,
            "should have created orders during the run"
        );
        assert!(
            lines_before > 0,
            "should have created order lines during the run"
        );

        // Build a new KitchenHandler from the same context (which now points to the
        // snapshot written at the end of run). The handler should load the persisted
        // orders and order lines.
        let reloaded = KitchenHandler::try_new(runner.ctx()).await?;
        let orders_after: usize = reloaded.orders.iter().map(|b| b.num_rows()).sum();
        let lines_after: usize = reloaded.order_lines.iter().map(|b| b.num_rows()).sum();

        assert_eq!(
            orders_after, orders_before,
            "orders should survive the snapshot round-trip"
        );
        assert_eq!(
            lines_after, lines_before,
            "order lines should survive the snapshot round-trip"
        );

        // Verify the snapshot metadata has the correct simulation_time. Reading it
        // back through the system schema confirms the auto-restore path would work.
        let system = runner.ctx().system();
        let sn_meta = system
            .snapshots()
            .await?
            .filter(col("id").eq(lit(runner.ctx().snapshot_id().to_string())))?
            .select_columns(&["simulation_time"])?
            .collect()
            .await?;

        assert!(!sn_meta.is_empty(), "snapshot metadata should exist");
        let stored_ts = sn_meta[0]
            .column(0)
            .as_primitive::<arrow::datatypes::TimestampMillisecondType>()
            .value(0);
        let stored_time =
            DateTime::<Utc>::from_timestamp_millis(stored_ts).expect("valid timestamp");
        assert_eq!(
            stored_time.timestamp_millis(),
            time_after_run.timestamp_millis(),
            "snapshot simulation_time should match the time after the run (ms precision)"
        );

        Ok(())
    }
}
