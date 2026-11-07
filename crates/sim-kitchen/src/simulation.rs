//! Ghost kitchen simulation
//!
//! [`KitchenSimulation`] wires the domain onto the sim-core framework:
//!
//! - **Customers** ([`Customer`]) are agents. An idle customer may decide to
//!   order; it then waits, eats, finishes and eventually orders again.
//! - **Kitchens** ([`Kitchen`]) are resources. A kitchen's capacity is the
//!   number of orders it prepares at once.
//! - **Orders** are [`FoodPrepWorkflow`] instances (prep → cook → package)
//!   whose steps are [`PrepTask`]s allocated to the order's kitchen.
//! - **Hooks** ([`KitchenSimulationHooks`]) turn framework callbacks into
//!   CloudEvents and feed them to the artifact generators.
//!
//! # Order lifecycle and events
//!
//! ```text
//! customer orders        → io.kitchen.order.created   (routed to the least busy kitchen)
//! first step allocated   → io.kitchen.order.assigned
//! last step completed    → io.kitchen.order.completed
//! food delivered         → io.kitchen.customer.eating
//! customer finishes      → io.kitchen.customer.done
//! ```
//!
//! Later preparation steps outrank new orders when the allocator hands out
//! capacity, so a kitchen never has more orders in progress than its capacity.
//!
//! # Example
//!
//! ```rust,no_run
//! use sim_kitchen::KitchenSimulation;
//!
//! let mut sim = KitchenSimulation::new_seeded(3, 5, false, Some(42));
//! sim.add_customers(20);
//! sim.run_for_steps(200)?;
//! sim.save_events("output/events.parquet")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::agents::{Customer, CustomerStatus};
use crate::artifact_generators::{
    CustomerDoneArtifactGenerator, OrderCompletedArtifactGenerator, OrderCreatedArtifactGenerator,
};
use crate::events::*;
pub use crate::pricing::menu_price;
use crate::profiles::ProfileRegistry;
use crate::resources::{Kitchen, PrepTask, PrepTaskFactory};
use crate::workflows::FoodPrepWorkflow;
use chrono::{DateTime, Utc};
use sim_artifacts::{
    ArtifactCollector, ArtifactCollectorProcessor, ArtifactStorage, JsonlArtifactStorage,
    NoOpStorage,
};
use sim_core::utils::idents::WorkflowInstanceId;
use sim_core::workflows::{WorkflowEngine, WorkflowRegistry, WorkflowStepEvent};
use sim_core::{
    AgentId, PopulationHandler, ResourceAllocator, ResourceId, SimEvent, SimulationContext,
    SimulationHooks, SimulationRunner, Workflow, WorkflowId, population::AgentTickResult,
    resources::Assignment,
};
use sim_events::{Event, EventDispatcher};
use std::collections::HashMap;

/// The runner type behind [`KitchenSimulation`]
pub type KitchenRunner = SimulationRunner<Customer, Kitchen, PrepTask, FoodPrepWorkflow>;

/// Name of the workflow engine that holds every kitchen's [`FoodPrepWorkflow`]
pub const FOOD_PREP_ENGINE: &str = "food_prep";

/// Simulation step length
const TIME_STEP_SECS: i64 = 30;

/// Chance per step that an idle customer orders
const ORDER_PROBABILITY: f64 = 0.2;

/// Time a customer spends eating
const EATING_MINUTES: i64 = 15;

#[derive(Debug)]
struct KitchenInfo {
    resource_id: ResourceId,
    name: String,
    workflow_id: WorkflowId,
    /// Orders routed here and not yet completed (queued or in progress)
    open_orders: usize,
}

#[derive(Debug)]
struct OrderInfo {
    customer_id: AgentId,
    kitchen: usize,
    created_at: DateTime<Utc>,
    assigned: bool,
}

/// Customer order awaiting the end of the meal
#[derive(Debug)]
struct CustomerOrder {
    order_id: String,
    total_price: f64,
    created_at: DateTime<Utc>,
}

/// Hooks that emit kitchen events and track orders
///
/// Hook state (open orders, routing) is not part of runner snapshots. After a
/// snapshot restore, workflow instances the hooks don't know about complete
/// without emitting order events.
pub struct KitchenSimulationHooks {
    dispatcher: EventDispatcher,
    kitchens: Vec<KitchenInfo>,
    /// Orders from creation until their workflow completes
    orders: HashMap<String, OrderInfo>,
    instance_orders: HashMap<WorkflowInstanceId, String>,
    /// Orders created this tick, waiting for their workflow instance
    new_orders: Vec<String>,
    /// Orders completed this tick, waiting to be delivered: (order_id, customer)
    ready_orders: Vec<(String, AgentId)>,
    customer_orders: HashMap<AgentId, CustomerOrder>,
    artifacts: ArtifactCollectorProcessor<ProfileRegistry>,
    /// Number of dispatcher events already fed to the artifact generators
    artifacts_seen: usize,
}

impl KitchenSimulationHooks {
    fn new(kitchens: &[Kitchen], storage: Box<dyn ArtifactStorage>) -> Self {
        let mut collector: ArtifactCollector<ProfileRegistry> = ArtifactCollector::new();
        collector.add_generator(Box::new(OrderCreatedArtifactGenerator::new()));
        collector.add_generator(Box::new(OrderCompletedArtifactGenerator::new()));
        collector.add_generator(Box::new(CustomerDoneArtifactGenerator::new()));
        let artifacts =
            ArtifactCollectorProcessor::new(collector, ProfileRegistry::new(), "kitchen-sim")
                .with_storage(storage);

        Self {
            dispatcher: EventDispatcher::new(),
            kitchens: kitchens
                .iter()
                .map(|k| KitchenInfo {
                    resource_id: k.id,
                    name: k.name.clone(),
                    workflow_id: FoodPrepWorkflow::for_kitchen(k).id(),
                    open_orders: 0,
                })
                .collect(),
            orders: HashMap::new(),
            instance_orders: HashMap::new(),
            new_orders: Vec::new(),
            ready_orders: Vec::new(),
            customer_orders: HashMap::new(),
            artifacts,
            artifacts_seen: 0,
        }
    }

    /// All events emitted so far
    pub fn dispatcher(&self) -> &EventDispatcher {
        &self.dispatcher
    }

    /// Number of artifacts generated so far
    pub fn artifact_count(&self) -> usize {
        self.artifacts.artifact_count()
    }

    /// Feed events emitted since the last call to the artifact generators
    fn process_artifacts(&mut self) -> sim_core::Result<()> {
        for event in &self.dispatcher.buffered_events()[self.artifacts_seen..] {
            self.artifacts
                .process(&event.to_cloudevents_json())
                .map_err(|e| sim_core::Error::Custom(format!("Artifact generation failed: {e}")))?;
        }
        self.artifacts_seen = self.dispatcher.buffered_count();
        Ok(())
    }

    /// Least busy kitchen; ties go to the first one
    fn route(&self) -> Option<usize> {
        (0..self.kitchens.len()).min_by_key(|&i| self.kitchens[i].open_orders)
    }

    fn create_order(
        &mut self,
        ctx: &SimulationContext,
        demand: &sim_core::Demand,
    ) -> sim_core::Result<()> {
        let field = |key: &str| {
            demand
                .metadata
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| sim_core::Error::Agent(format!("food_order demand has no {key}")))
        };
        let order_id = field("order_id")?;
        let customer_id = AgentId::parse(&field("customer_id")?)
            .map_err(|e| sim_core::Error::Agent(format!("food_order customer_id: {e}")))?;
        let items: Vec<String> = demand
            .metadata
            .get("items")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let total_price = items.iter().map(|i| menu_price(i)).sum();
        let now = ctx.current_time();

        let kitchen = self.route().ok_or_else(|| {
            sim_core::Error::InvalidState("an order was placed but there are no kitchens".into())
        })?;
        self.kitchens[kitchen].open_orders += 1;
        self.orders.insert(
            order_id.clone(),
            OrderInfo {
                customer_id,
                kitchen,
                created_at: now,
                assigned: false,
            },
        );
        self.customer_orders.insert(
            customer_id,
            CustomerOrder {
                order_id: order_id.clone(),
                total_price,
                created_at: now,
            },
        );
        self.new_orders.push(order_id.clone());

        if ctx.is_verbose() {
            println!(
                "[step {}] order {order_id} → {}",
                ctx.step(),
                self.kitchens[kitchen].name
            );
        }
        self.dispatcher.emit(Event::new(
            Box::new(OrderCreatedEvent {
                order_id,
                customer_id: customer_id.to_string(),
                items,
                total_price,
                timestamp: now,
            }),
            now,
        ));
        Ok(())
    }
}

impl SimulationHooks for KitchenSimulationHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        for demand in results.iter().flat_map(|r| &r.demands) {
            if demand.demand_type == "food_order" {
                self.create_order(ctx, demand)?;
            }
        }
        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment],
    ) -> sim_core::Result<()> {
        let now = ctx.current_time();
        for assignment in assignments {
            let Some(order_id) = assignment
                .workflow_instance_id
                .and_then(|i| self.instance_orders.get(&i))
            else {
                continue;
            };
            let Some(order) = self.orders.get_mut(order_id) else {
                continue;
            };
            if order.assigned {
                continue; // a later step of an order already in progress
            }
            order.assigned = true;
            let kitchen = &self.kitchens[order.kitchen];
            self.dispatcher.emit(Event::new(
                Box::new(OrderAssignedEvent {
                    order_id: order_id.clone(),
                    kitchen_id: kitchen.resource_id.to_string(),
                    kitchen_name: kitchen.name.clone(),
                    estimated_completion: now + FoodPrepWorkflow::total_duration(),
                }),
                now,
            ));
        }
        Ok(())
    }

    fn on_workflow_step_completed(
        &mut self,
        ctx: &SimulationContext,
        event: &WorkflowStepEvent,
    ) -> sim_core::Result<()> {
        let WorkflowStepEvent::WorkflowCompleted {
            instance_id,
            completion_time,
            ..
        } = event
        else {
            return Ok(());
        };
        let Some(order_id) = self.instance_orders.remove(instance_id) else {
            return Ok(());
        };
        let Some(order) = self.orders.remove(&order_id) else {
            return Ok(());
        };
        let kitchen = &mut self.kitchens[order.kitchen];
        kitchen.open_orders -= 1;

        if ctx.is_verbose() {
            println!("[step {}] order {order_id} ready", ctx.step());
        }
        self.dispatcher.emit(Event::new(
            Box::new(OrderCompletedEvent {
                order_id: order_id.clone(),
                customer_id: order.customer_id.to_string(),
                kitchen_id: kitchen.resource_id.to_string(),
                prep_time_seconds: (*completion_time - order.created_at).num_seconds(),
            }),
            *completion_time,
        ));
        self.ready_orders.push((order_id, order.customer_id));
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Ghost kitchen simulation
///
/// Drive it with [`step`](Self::step), [`run_for_steps`](Self::run_for_steps)
/// or [`run_controlled_for`](Self::run_controlled_for) rather than ticking the
/// [`runner`](Self::runner) directly: orders start, food is delivered and
/// customers finish in `step`, around the framework tick.
pub struct KitchenSimulation {
    runner: KitchenRunner,
}

impl KitchenSimulation {
    /// Create a simulation with `num_kitchens` kitchens that each prepare up
    /// to `kitchen_capacity` orders at once (unseeded)
    pub fn new(num_kitchens: usize, kitchen_capacity: usize, verbose: bool) -> Self {
        Self::new_seeded(num_kitchens, kitchen_capacity, verbose, None)
    }

    /// Like [`new`](Self::new), with an optional global seed
    ///
    /// With a seed, customer decisions and customer/order IDs are
    /// reproducible run-to-run, and so is the order of emitted events.
    pub fn new_seeded(
        num_kitchens: usize,
        kitchen_capacity: usize,
        verbose: bool,
        seed: Option<u64>,
    ) -> Self {
        let config = sim_core::SimulationConfig {
            name: "ghost-kitchen".to_string(),
            time_step: chrono::Duration::seconds(TIME_STEP_SECS),
            verbose,
            seed,
            ..Default::default()
        };
        let mut runner = KitchenRunner::new(config);

        let kitchens: Vec<Kitchen> = (1..=num_kitchens)
            .map(|i| Kitchen::new(format!("Kitchen-{i}"), kitchen_capacity))
            .collect();

        let mut engine = WorkflowEngine::new();
        for kitchen in &kitchens {
            engine.register_workflow(FoodPrepWorkflow::for_kitchen(kitchen));
        }
        let mut registry = WorkflowRegistry::new();
        registry
            .register(FOOD_PREP_ENGINE.to_string(), engine)
            .expect("fresh registry");
        runner.set_workflow_registry(registry);
        runner.set_task_factory(Box::new(PrepTaskFactory));

        runner.set_hooks(Box::new(KitchenSimulationHooks::new(
            &kitchens,
            Box::new(NoOpStorage),
        )));

        let mut allocator = ResourceAllocator::new();
        allocator
            .add_resources(kitchens)
            .expect("kitchen names are unique");
        runner.set_resource_allocator(allocator);
        runner.set_population_handler(PopulationHandler::new());

        Self { runner }
    }

    /// Write generated artifacts as JSON Lines under `dir` at the end of each
    /// run (see [`JsonlArtifactStorage`] for the layout)
    ///
    /// Without this, artifacts are generated (see
    /// [`KitchenSimulationHooks::artifact_count`]) but not written.
    pub fn with_artifact_dir(mut self, dir: impl Into<std::path::PathBuf>) -> Self {
        self.hooks_mut()
            .artifacts
            .set_storage(Box::new(JsonlArtifactStorage::new(dir)));
        self
    }

    /// Add `num_customers` customers (replacing any existing population)
    ///
    /// When the simulation is seeded, each customer gets its own sub-seed
    /// derived from the global seed and its index.
    pub fn add_customers(&mut self, num_customers: usize) {
        let seed = self.runner.context().config().seed;
        let mut population = PopulationHandler::new();
        for i in 0..num_customers {
            let location = geo::Point::new(-122.4 + (i as f64 * 0.001), 37.8 + (i as f64 * 0.001));
            let eating = chrono::Duration::minutes(EATING_MINUTES);
            let customer = match seed {
                Some(global) => Customer::with_params_seeded(
                    location,
                    ORDER_PROBABILITY,
                    eating,
                    sim_core::derive_seed(global, "customer").wrapping_add(i as u64),
                ),
                None => Customer::with_params(location, ORDER_PROBABILITY, eating),
            };
            population.add_agent(customer);
        }
        self.runner.set_population_handler(population);
    }

    /// Execute one simulation step
    ///
    /// Ticks the runner, then starts workflows for new orders, delivers
    /// completed orders to their customers, reports customers who finished
    /// eating, and feeds the step's events to the artifact generators.
    pub fn step(&mut self) -> sim_core::Result<sim_core::TickResult> {
        let tick = self.runner.tick()?;
        self.start_new_orders()?;
        self.deliver_ready_orders()?;
        self.report_done_customers()?;
        self.hooks_mut().process_artifacts()?;
        Ok(tick)
    }

    /// Run `steps` steps, then persist artifacts
    pub fn run_for_steps(&mut self, steps: usize) -> sim_core::Result<()> {
        for _ in 0..steps {
            if self.step()? == sim_core::TickResult::ReachedEnd {
                break;
            }
        }
        self.flush_artifacts()
    }

    /// Run up to `max_steps` steps, driven by the control channel attached to
    /// the runner
    ///
    /// Honours Pause/Resume/StepN/Stop and emits `SimEvent::TickCompleted`
    /// with per-step event counts. At the step limit (or the end of a Range
    /// time mode) it pauses instead of returning, so a TUI can extend the
    /// run; it returns on Stop.
    ///
    /// Call `runner_mut().attach_control(receiver)` first.
    pub fn run_controlled_for(&mut self, max_steps: usize) -> sim_core::Result<()> {
        let mut steps_taken = 0;
        let mut step_limit_paused = false;

        loop {
            if steps_taken >= max_steps && !step_limit_paused {
                step_limit_paused = true;
                self.runner.set_paused(true);
                let step = self.runner.context().step();
                self.runner.emit_event(SimEvent::Paused { at_step: step });
            }

            let mut stopped = false;
            while self.runner.is_paused() {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if self.runner.process_control_commands()? {
                    stopped = true;
                    break;
                }
            }
            if stopped || self.runner.process_control_commands()? {
                break;
            }
            if self.runner.is_paused() {
                continue;
            }

            let before = self.hooks().dispatcher.event_counts().clone();
            let tick = self.step()?;
            steps_taken += 1;

            let event_counts = self
                .hooks()
                .dispatcher
                .event_counts()
                .iter()
                .filter_map(|(k, v)| {
                    let delta = v - before.get(k).copied().unwrap_or(0);
                    (delta > 0).then(|| (k.clone(), delta as u64))
                })
                .collect();
            let step = self.runner.context().step();
            self.runner.emit_event(SimEvent::TickCompleted {
                step,
                sim_time: self.runner.context().current_time(),
                event_counts,
            });

            if tick == sim_core::TickResult::ReachedEnd {
                self.runner.set_paused(true);
                self.runner.emit_event(SimEvent::Paused { at_step: step });
            }
        }

        self.flush_artifacts()?;
        self.runner.emit_event(SimEvent::SimulationEnded {
            total_steps: self.runner.context().step(),
            final_time: self.runner.context().current_time(),
        });
        Ok(())
    }

    /// The underlying runner
    pub fn runner(&self) -> &KitchenRunner {
        &self.runner
    }

    /// The underlying runner, mutably (time mode, control channel, clock)
    pub fn runner_mut(&mut self) -> &mut KitchenRunner {
        &mut self.runner
    }

    /// The simulation hooks (events, artifact count)
    pub fn hooks(&self) -> &KitchenSimulationHooks {
        self.runner
            .hooks_as::<KitchenSimulationHooks>()
            .expect("KitchenSimulation installs KitchenSimulationHooks")
    }

    fn hooks_mut(&mut self) -> &mut KitchenSimulationHooks {
        self.runner
            .hooks_as_mut::<KitchenSimulationHooks>()
            .expect("KitchenSimulation installs KitchenSimulationHooks")
    }

    /// All events emitted so far, in emission order
    pub fn event_records(&self) -> Vec<sim_events::EventRecord> {
        self.hooks().dispatcher.to_records()
    }

    /// Write all events emitted so far to a Parquet file and clear them
    ///
    /// Returns the number of events written. Creates parent directories.
    pub fn save_events(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> std::result::Result<usize, Box<dyn std::error::Error>> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let hooks = self.hooks_mut();
        hooks.process_artifacts()?;
        let records = hooks.dispatcher.to_records();
        hooks.dispatcher.clear();
        hooks.artifacts_seen = 0;

        let mut writer = sim_events::storage::ParquetEventWriter::new(path)?;
        writer.write_events(&records)?;
        writer.close()?;
        Ok(records.len())
    }

    fn flush_artifacts(&mut self) -> sim_core::Result<()> {
        let hooks = self.hooks_mut();
        hooks.process_artifacts()?;
        hooks
            .artifacts
            .flush_to_storage()
            .map_err(|e| sim_core::Error::Custom(format!("Failed to write artifacts: {e}")))
    }

    /// Start a food-preparation workflow for every order created this tick
    fn start_new_orders(&mut self) -> sim_core::Result<()> {
        let hooks = self.hooks_mut();
        let new_orders: Vec<(String, WorkflowId)> = std::mem::take(&mut hooks.new_orders)
            .into_iter()
            .map(|order_id| {
                let kitchen = hooks.orders[&order_id].kitchen;
                (order_id, hooks.kitchens[kitchen].workflow_id)
            })
            .collect();
        if new_orders.is_empty() {
            return Ok(());
        }

        let engine = self
            .runner
            .workflow_registry_mut()
            .and_then(|r| r.get_mut(FOOD_PREP_ENGINE))
            .ok_or_else(|| sim_core::Error::InvalidState("food_prep engine missing".into()))?;
        let started: Vec<(WorkflowInstanceId, String)> = new_orders
            .into_iter()
            .map(|(order_id, workflow_id)| {
                engine
                    .start_instance(workflow_id)
                    .map(|instance| (instance.id, order_id))
            })
            .collect::<sim_core::Result<_>>()?;
        self.hooks_mut().instance_orders.extend(started);
        Ok(())
    }

    /// Hand completed orders to their customers, who start eating
    fn deliver_ready_orders(&mut self) -> sim_core::Result<()> {
        let ready = std::mem::take(&mut self.hooks_mut().ready_orders);
        if ready.is_empty() {
            return Ok(());
        }
        let now = self.runner.context().current_time();
        let population = self
            .runner
            .population_mut()
            .ok_or_else(|| sim_core::Error::InvalidState("no customer population".into()))?;

        let mut fed = Vec::new();
        let mut events = Vec::new();
        for (order_id, customer_id) in ready {
            let Some(mut customer) = population.get_agent(customer_id) else {
                continue;
            };
            if !matches!(&customer.status, CustomerStatus::WaitingForFood { order_id: o } if *o == order_id)
            {
                continue;
            }
            customer.start_eating(now);
            events.push(Event::new(
                Box::new(CustomerEatingEvent {
                    customer_id: customer_id.to_string(),
                    order_id,
                    finish_time: now + customer.eating_duration,
                }),
                now,
            ));
            fed.push(customer);
        }
        population.update_agents(fed)?;
        self.hooks_mut().dispatcher.emit_batch(events);
        Ok(())
    }

    /// Emit `customer.done` for customers who finished eating this tick
    ///
    /// A customer is `Done` for exactly one tick before turning idle, so each
    /// meal is reported once.
    fn report_done_customers(&mut self) -> sim_core::Result<()> {
        let now = self.runner.context().current_time();
        let done: Vec<AgentId> = self
            .runner
            .population()
            .map(|p| p.iter().filter(|c| c.is_done()).map(|c| c.id).collect())
            .unwrap_or_default();
        let hooks = self.hooks_mut();
        for customer_id in done {
            let Some(order) = hooks.customer_orders.remove(&customer_id) else {
                continue;
            };
            hooks.dispatcher.emit(Event::new(
                Box::new(CustomerDoneEvent {
                    customer_id: customer_id.to_string(),
                    order_id: order.order_id,
                    total_price: order.total_price,
                    total_time_seconds: (now - order.created_at).num_seconds(),
                }),
                now,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
