# sim-kitchen

Ghost kitchen simulation: the reference domain for the `sim-*` framework.
Customers order food, kitchens prepare it, customers eat and order again.
Every transition is a CloudEvent, and the events drive synthetic artifacts
(web events, emails, receipts, payment records).

## Quick start

```bash
just sim-kitchen 240          # 240 steps (2 h); events + artifacts under .caspers/sim-kitchen
cargo run --bin caspers -- run --duration 240 --seed 42   # same domain in the TUI
```

```rust
use sim_kitchen::KitchenSimulation;

// 3 kitchens × 5 concurrent orders, seeded for reproducibility
let mut sim = KitchenSimulation::new_seeded(3, 5, false, Some(42))
    .with_artifact_dir("output/artifacts");
sim.add_customers(20);
sim.run_for_steps(240)?;
sim.save_events("output/events.parquet")?;
```

`KitchenSimulation` is the entry point. Drive it with `step`, `run_for_steps`
or `run_controlled_for` (control channel, used by the CLI), not by ticking the
runner directly: new orders start, food is delivered and customers finish in
`step`, around the framework tick.

## Domain model

| Framework concept | Kitchen type |
|---|---|
| Agent | `Customer`: `Idle → Ordering → WaitingForFood → Eating → Done → Idle` |
| Resource | `Kitchen`: capacity = orders prepared at once; resource type `kitchen:<name>` |
| Workflow | `FoodPrepWorkflow`, one per kitchen: prep (2 min) → cook (5 min) → package (1 min) |
| Task | `PrepTask`: one workflow step, created by `PrepTaskFactory` |
| Hooks | `KitchenSimulationHooks`: routing, events, artifacts |

A new order goes to the kitchen with the fewest open orders, and all its
steps run there. Later steps have higher task priority than new orders, so a
kitchen never has more orders in progress than its capacity; extra orders
queue.

Time advances 30 s per step. With a seed, customer decisions, customer IDs,
order IDs and the order of events are reproducible (only CloudEvents `id`s
differ).

## Events

| Type | When | Data |
|---|---|---|
| `io.kitchen.order.created` | customer orders | `order_id`, `customer_id`, `items`, `total_price`, `timestamp` |
| `io.kitchen.order.assigned` | kitchen starts the order | `order_id`, `kitchen_id`, `kitchen_name`, `estimated_completion` |
| `io.kitchen.order.completed` | last step done | `order_id`, `customer_id`, `kitchen_id`, `prep_time_seconds` |
| `io.kitchen.customer.eating` | food delivered | `customer_id`, `order_id`, `finish_time` |
| `io.kitchen.customer.done` | meal finished | `customer_id`, `order_id`, `total_price`, `total_time_seconds` |

Order events use the order ID as CloudEvents subject, customer events the
customer ID. `caspers validate --domain kitchen` checks the lifecycle
invariants (pass `--kitchen-capacity <kitchens × capacity>` for the capacity
bound).

## Artifacts

| Event | Artifacts |
|---|---|
| `order.created` | web session (page view → order placed), confirmation email and SMS, payment API call and authorization |
| `order.completed` | kitchen ticket, ingredient usage, quality-control record |
| `customer.done` | HTML receipt, loyalty transaction, review request |

Artifacts are only written when you call `with_artifact_dir`.

Amounts come from `OrderCharges::for_subtotal(total_price)`: the event's menu
subtotal plus 8.5% tax and an 18% tip, rounded to cents. The payment
authorization, confirmation email, receipt and loyalty record all show the
same `total`. Session, message and document IDs are UUID v5s derived from the
order ID, so a seeded run reproduces every artifact field except
`source_event_id`, which points at the (random) CloudEvents `id`. A generator
fails on an event that lacks a field it needs instead of filling in a
placeholder.

## Tests

```bash
cargo test -p sim-kitchen
```

`tests/lifecycle_invariant_tests.rs` checks event-stream invariants
(complete lifecycles, causal order, no duplicates, capacity), and
`tests/determinism_tests.rs` checks that a seed reproduces the event stream
and the artifacts.
