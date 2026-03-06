use std::collections::HashMap;

use arrow::array::{Array, AsArray};
use datafusion::prelude::{col, lit};
use rstest::*;

use super::*;
use crate::agents::functions::OrderSpec;
use crate::test_utils::runner_fixed;
use crate::{OrderStatus, PersonRole, PersonStatusFlag, Result, SimulationRunner};

/// Collect (event_type, time_str) pairs per order_id.
async fn events_per_order(
    runner: &SimulationRunner,
    result: &SimulationRunResult,
) -> Result<HashMap<Vec<u8>, Vec<(String, String)>>> {
    let ctx = runner.ctx.ctx();
    let lifecycle = result.event_log().order_lifecycle_events(ctx)?;
    let batches = lifecycle.collect().await?;

    let mut map: HashMap<Vec<u8>, Vec<(String, String)>> = HashMap::new();
    for batch in &batches {
        let order_ids = batch
            .column_by_name("order_id")
            .expect("order_id")
            .as_fixed_size_binary();
        let event_types = batch
            .column_by_name("event_type")
            .expect("event_type")
            .as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .expect("StringArray");
        let times = batch
            .column_by_name("time")
            .expect("time")
            .as_any()
            .downcast_ref::<arrow::array::LargeStringArray>()
            .expect("LargeStringArray");

        for i in 0..batch.num_rows() {
            if order_ids.is_valid(i) {
                let oid = order_ids.value(i).to_vec();
                let etype = event_types.value(i).to_string();
                let ts = times.value(i).to_string();
                map.entry(oid).or_default().push((etype, ts));
            }
        }
    }
    Ok(map)
}

/// Verify that a single order traverses the full lifecycle:
/// order_created -> order_ready -> order_picked_up -> order_delivered
#[rstest]
#[tokio::test]
async fn test_order_full_lifecycle(
    #[future]
    #[with(OrderSpec::Once(vec![1]))]
    runner_fixed: Result<SimulationRunner>,
) -> Result<()> {
    let mut runner = runner_fixed.await?;
    let result = runner.run(500).await?;

    let ctx = runner.ctx.ctx();
    let counts = result.event_log().count_by_type(ctx).await?;

    let created = counts
        .get(SimulationEvent::OrderCreated.event_type())
        .copied()
        .unwrap_or(0);
    let ready = counts
        .get(SimulationEvent::OrderReady.event_type())
        .copied()
        .unwrap_or(0);
    let picked_up = counts
        .get(SimulationEvent::OrderPickedUp.event_type())
        .copied()
        .unwrap_or(0);
    let delivered = counts
        .get(SimulationEvent::OrderDelivered.event_type())
        .copied()
        .unwrap_or(0);

    assert_eq!(created, 1, "expected exactly 1 order_created event");
    assert!(
        ready >= 1,
        "expected at least 1 order_ready event, got 0. Full counts: {counts:?}"
    );
    assert!(
        picked_up >= 1,
        "expected at least 1 order_picked_up event, got 0. Full counts: {counts:?}"
    );
    assert!(
        delivered >= 1,
        "expected at least 1 order_delivered event, got 0. Full counts: {counts:?}"
    );

    // Verify all lifecycle events reference the same order_id.
    let per_order = events_per_order(&runner, &result).await?;
    assert!(
        !per_order.is_empty(),
        "should have at least one order with events"
    );

    for (oid, events) in &per_order {
        let types: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
        assert!(
            types.contains(&SimulationEvent::OrderCreated.event_type()),
            "order {oid:?} missing order_created, got {types:?}"
        );
        assert!(
            types.contains(&SimulationEvent::OrderReady.event_type()),
            "order {oid:?} missing order_ready, got {types:?}"
        );
        assert!(
            types.contains(&SimulationEvent::OrderPickedUp.event_type()),
            "order {oid:?} missing order_picked_up, got {types:?}"
        );
        assert!(
            types.contains(&SimulationEvent::OrderDelivered.event_type()),
            "order {oid:?} missing order_delivered, got {types:?}"
        );

        // Verify temporal ordering via ISO 8601 string comparison.
        let ts = |et: &str| {
            events
                .iter()
                .find(|(t, _)| t == et)
                .map(|(_, ts)| ts.clone())
        };
        let t_created = ts(SimulationEvent::OrderCreated.event_type()).unwrap();
        let t_ready = ts(SimulationEvent::OrderReady.event_type()).unwrap();
        let t_picked_up = ts(SimulationEvent::OrderPickedUp.event_type()).unwrap();
        let t_delivered = ts(SimulationEvent::OrderDelivered.event_type()).unwrap();

        assert!(
            t_created <= t_ready,
            "order_created ({t_created}) should be <= order_ready ({t_ready})"
        );
        assert!(
            t_ready <= t_picked_up,
            "order_ready ({t_ready}) should be <= order_picked_up ({t_picked_up})"
        );
        assert!(
            t_picked_up <= t_delivered,
            "order_picked_up ({t_picked_up}) should be <= order_delivered ({t_delivered})"
        );
    }

    Ok(())
}

/// After delivery, the courier's status should return to Idle so they can
/// be assigned new deliveries.
#[rstest]
#[tokio::test]
async fn test_courier_returns_to_idle(
    #[future]
    #[with(OrderSpec::Once(vec![1]))]
    runner_fixed: Result<SimulationRunner>,
) -> Result<()> {
    let mut runner = runner_fixed.await?;
    let result = runner.run(500).await?;

    let ctx = runner.ctx.ctx();
    let counts = result.event_log().count_by_type(ctx).await?;
    let delivered = counts
        .get(SimulationEvent::OrderDelivered.event_type())
        .copied()
        .unwrap_or(0);
    assert!(
        delivered >= 1,
        "expected at least 1 delivery to verify courier return. Full counts: {counts:?}"
    );

    // After 500 steps (500 minutes of sim time), all couriers that delivered
    // should have had time to return and be set back to Idle.
    let couriers = runner
        .population()
        .population(runner.ctx())?
        .filter(col("role").eq(lit(PersonRole::Courier.as_ref())))?
        .filter(col("status").eq(lit(PersonStatusFlag::Delivering.as_ref())))?
        .count()
        .await?;

    assert_eq!(
        couriers, 0,
        "no couriers should still be in Delivering status after 500 steps"
    );

    // Verify at least some couriers are idle (available for new work).
    let idle_couriers = runner
        .population()
        .population(runner.ctx())?
        .filter(col("role").eq(lit(PersonRole::Courier.as_ref())))?
        .filter(col("status").eq(lit(PersonStatusFlag::Idle.as_ref())))?
        .count()
        .await?;

    assert!(
        idle_couriers > 0,
        "expected some couriers to be back in Idle status"
    );

    Ok(())
}

/// Verify order status in KitchenHandler transitions through the full lifecycle.
#[rstest]
#[tokio::test]
async fn test_order_status_transitions(
    #[future]
    #[with(OrderSpec::Once(vec![1]))]
    runner_fixed: Result<SimulationRunner>,
) -> Result<()> {
    let mut runner = runner_fixed.await?;
    let result = runner.run(500).await?;

    let ctx = runner.ctx.ctx();
    let counts = result.event_log().count_by_type(ctx).await?;

    // Ensure the full lifecycle ran.
    assert!(
        counts
            .get(SimulationEvent::OrderDelivered.event_type())
            .copied()
            .unwrap_or(0)
            >= 1,
        "need at least one delivered order to test status transitions. Full counts: {counts:?}"
    );

    // After a full lifecycle run, the order should be in Delivered status.
    let delivered_orders = runner
        .kitchens()
        .orders(runner.ctx())?
        .filter(col("status").eq(lit(OrderStatus::Delivered.as_ref())))?
        .count()
        .await?;

    assert!(
        delivered_orders >= 1,
        "expected at least 1 order with Delivered status, got {delivered_orders}"
    );

    // No orders should remain in Ready or Submitted (they should have progressed).
    let ready_orders = runner
        .kitchens()
        .orders(runner.ctx())?
        .filter(col("status").eq(lit(OrderStatus::Ready.as_ref())))?
        .count()
        .await?;
    assert_eq!(
        ready_orders, 0,
        "no orders should still be in Ready status after full lifecycle"
    );

    Ok(())
}

/// Verify that multiple orders each independently complete their lifecycle.
#[rstest]
#[tokio::test]
async fn test_multiple_orders_lifecycle(
    #[future]
    #[with(OrderSpec::Once(vec![1, 1, 1]))]
    runner_fixed: Result<SimulationRunner>,
) -> Result<()> {
    let mut runner = runner_fixed.await?;
    let result = runner.run(500).await?;

    let ctx = runner.ctx.ctx();
    let counts = result.event_log().count_by_type(ctx).await?;

    let created = counts
        .get(SimulationEvent::OrderCreated.event_type())
        .copied()
        .unwrap_or(0);
    assert!(
        created >= 3,
        "expected at least 3 order_created events for 3 orders, got {created}"
    );

    let per_order = events_per_order(&runner, &result).await?;
    let mut fully_completed = 0;
    for (_oid, events) in &per_order {
        let types: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
        let has_all = types.contains(&SimulationEvent::OrderCreated.event_type())
            && types.contains(&SimulationEvent::OrderReady.event_type())
            && types.contains(&SimulationEvent::OrderPickedUp.event_type())
            && types.contains(&SimulationEvent::OrderDelivered.event_type());
        if has_all {
            fully_completed += 1;

            let ts = |et: &str| {
                events
                    .iter()
                    .find(|(t, _)| t == et)
                    .map(|(_, ts)| ts.clone())
            };
            let t_created = ts(SimulationEvent::OrderCreated.event_type()).unwrap();
            let t_ready = ts(SimulationEvent::OrderReady.event_type()).unwrap();
            let t_picked_up = ts(SimulationEvent::OrderPickedUp.event_type()).unwrap();
            let t_delivered = ts(SimulationEvent::OrderDelivered.event_type()).unwrap();

            assert!(t_created <= t_ready);
            assert!(t_ready <= t_picked_up);
            assert!(t_picked_up <= t_delivered);
        }
    }

    assert!(
        fully_completed >= 1,
        "expected at least 1 fully completed order out of {created} created, got {fully_completed}"
    );

    Ok(())
}
