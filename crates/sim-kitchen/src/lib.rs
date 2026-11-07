//! Ghost kitchen simulation: the reference domain for the sim-* framework
//!
//! Customers order food, kitchens prepare it, customers eat and order again.
//! Every state transition is a CloudEvent, and the events drive synthetic
//! artifacts (web events, emails, receipts, payment records).
//!
//! # Domain model
//!
//! | Framework concept | Kitchen type |
//! |---|---|
//! | Agent | [`Customer`]: `Idle → Ordering → WaitingForFood → Eating → Done → Idle` |
//! | Resource | [`Kitchen`]: capacity = orders prepared at once |
//! | Workflow | [`FoodPrepWorkflow`]: prep (2 min) → cook (5 min) → package (1 min) |
//! | Task | [`PrepTask`]: one workflow step, created by [`PrepTaskFactory`] |
//! | Hooks | [`KitchenSimulationHooks`]: routing, events, artifacts |
//!
//! New orders go to the kitchen with the fewest open orders. An order's steps
//! all run in that kitchen, and later steps take priority over new orders, so
//! a kitchen never has more orders in progress than its capacity.
//!
//! # Events
//!
//! | Type | When | Key fields |
//! |---|---|---|
//! | `io.kitchen.order.created` | customer orders | `order_id`, `customer_id`, `items`, `total_price` |
//! | `io.kitchen.order.assigned` | kitchen starts the order | `order_id`, `kitchen_id`, `kitchen_name` |
//! | `io.kitchen.order.completed` | last step done | `order_id`, `kitchen_id`, `prep_time_seconds` |
//! | `io.kitchen.customer.eating` | food delivered | `order_id`, `customer_id`, `finish_time` |
//! | `io.kitchen.customer.done` | meal finished | `order_id`, `customer_id`, `total_time_seconds` |
//!
//! # Example
//!
//! ```rust,no_run
//! use sim_kitchen::KitchenSimulation;
//!
//! // 3 kitchens × 5 concurrent orders, seeded for reproducibility
//! let mut sim = KitchenSimulation::new_seeded(3, 5, false, Some(42))
//!     .with_artifact_dir("output/artifacts");
//! sim.add_customers(20);
//! sim.run_for_steps(240)?; // 2 hours at 30 s per step
//! sim.save_events("output/events.parquet")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod agents;
pub mod artifact_generators;
pub mod events;
pub mod profiles;
pub mod resources;
pub mod simulation;
pub mod workflows;

pub use agents::{Customer, CustomerStatus};
pub use events::*;
pub use resources::{Kitchen, PrepTask, PrepTaskFactory};
pub use simulation::{
    FOOD_PREP_ENGINE, KitchenRunner, KitchenSimulation, KitchenSimulationHooks, menu_price,
};
pub use workflows::FoodPrepWorkflow;
