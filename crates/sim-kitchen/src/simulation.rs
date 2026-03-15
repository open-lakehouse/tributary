//! Simulation orchestration for ghost kitchen

use crate::agents::Customer;
use crate::events::*;
use crate::resources::{Kitchen, Order};
use chrono::{DateTime, Utc};
use sim_core::{
    AgentId, PopulationHandler, ResourceAllocator, SimulationContext, SimulationHooks,
    SimulationRunner,
    population::AgentTickResult,
    resources::{Assignment, LeastLoaded},
};
use sim_events::{Event, EventDispatcher};
use std::collections::HashMap;

/// Workflow type placeholder (not used in this example)
#[derive(Debug, Clone)]
pub struct NoWorkflow;

impl sim_core::Workflow for NoWorkflow {
    fn id(&self) -> sim_core::WorkflowId {
        sim_core::WorkflowId::new()
    }

    fn workflow_type(&self) -> &str {
        "none"
    }

    fn steps(&self) -> &[sim_core::workflows::WorkflowStep] {
        &[]
    }
}

/// Order tracking information
#[derive(Debug, Clone)]
#[allow(dead_code)] // Future use for tracking order lifecycle
struct OrderInfo {
    order_id: String,
    customer_id: AgentId,
    created_at: DateTime<Utc>,
    assigned_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
}

/// Simulation hooks that emit events and manage order lifecycle
pub struct KitchenSimulationHooks {
    /// Event dispatcher for CloudEvents
    pub dispatcher: EventDispatcher,
    /// Track active orders
    orders: HashMap<String, OrderInfo>,
    /// Customer spawn time tracking
    #[allow(dead_code)] // Future use for tracking customer lifecycle
    customer_spawn_times: HashMap<AgentId, DateTime<Utc>>,
}

impl KitchenSimulationHooks {
    pub fn new() -> Self {
        Self {
            dispatcher: EventDispatcher::new(),
            orders: HashMap::new(),
            customer_spawn_times: HashMap::new(),
        }
    }

    /// Get the event dispatcher
    pub fn dispatcher(&self) -> &EventDispatcher {
        &self.dispatcher
    }

    /// Take ownership of the dispatcher (for flushing at end)
    pub fn take_dispatcher(self) -> EventDispatcher {
        self.dispatcher
    }
}

impl Default for KitchenSimulationHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationHooks for KitchenSimulationHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        if ctx.config().verbose {
            println!("🏁 Ghost Kitchen Simulation Starting");
            println!("   Time: {}", ctx.current_time());
            println!("   Time step: {:?}", ctx.time_step());
        }
        Ok(())
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        // Process demands (orders) from agent ticks
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "food_order" {
                    let order_id = demand
                        .metadata
                        .get("order_id")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("unknown");

                    let customer_id = demand
                        .metadata
                        .get("customer_id")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("unknown");

                    let items: Vec<String> = demand
                        .metadata
                        .get("items")
                        .and_then(|v: &serde_json::Value| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    // Track order
                    self.orders.insert(
                        order_id.to_string(),
                        OrderInfo {
                            order_id: order_id.to_string(),
                            customer_id: sim_core::AgentId::new(), // Simplified
                            created_at: ctx.current_time(),
                            assigned_at: None,
                            completed_at: None,
                        },
                    );

                    // Emit order created event
                    let event = Event::new(Box::new(OrderCreatedEvent {
                        order_id: order_id.to_string(),
                        customer_id: customer_id.to_string(),
                        items,
                        timestamp: ctx.current_time(),
                    }));

                    self.dispatcher.emit(event);

                    if ctx.config().verbose {
                        println!(
                            "📝 [Step {}] Order created: {}",
                            ctx.step(),
                            order_id
                        );
                    }
                }
            }

            // Track customer state changes
            for (key, value) in &result.state_changes {
                if key == "finished_eating" && value.as_bool() == Some(true) {
                    // Customer finished eating
                    if ctx.config().verbose {
                        println!("✅ [Step {}] Customer finished eating", ctx.step());
                    }
                }
            }
        }

        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment],
    ) -> sim_core::Result<()> {
        for _assignment in assignments {
            // Find order info (simplified - would need actual lookup)
            if ctx.config().verbose {
                println!(
                    "👨‍🍳 [Step {}] Order assigned to kitchen",
                    ctx.step()
                );
            }
        }

        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        if ctx.config().verbose {
            println!("\n🏁 Simulation Complete");
            println!("   Total steps: {}", ctx.step());
            println!("   Elapsed time: {:?}", ctx.elapsed());
            println!("   Total events: {}", self.dispatcher.buffered_count());
            println!("\n📊 Event Summary:");
            for (event_type, count) in self.dispatcher.event_counts() {
                println!("   {}: {}", event_type, count);
            }
        }
        Ok(())
    }
}

/// Build a ghost kitchen simulation runner
pub fn build_simulation(
    num_kitchens: usize,
    kitchen_capacity: usize,
    verbose: bool,
) -> SimulationRunner<Customer, Kitchen, Order, NoWorkflow> {
    let mut config = sim_core::SimulationConfig::default();
    config.name = "ghost-kitchen".to_string();
    config.time_step = chrono::Duration::seconds(30); // 30 second steps
    config.verbose = verbose;

    let mut runner = SimulationRunner::new(config);

    // Create kitchens
    let mut allocator = ResourceAllocator::new();
    for i in 0..num_kitchens {
        let kitchen = Kitchen::new(format!("Kitchen-{}", i + 1), kitchen_capacity);
        allocator.add_resource(kitchen);
    }
    runner.set_resource_allocator(allocator);

    // Set hooks
    runner.set_hooks(Box::new(KitchenSimulationHooks::new()));

    runner
}

/// Add customers to the simulation
pub fn add_customers(
    runner: &mut SimulationRunner<Customer, Kitchen, Order, NoWorkflow>,
    num_customers: usize,
) {
    let mut population = PopulationHandler::new();

    for i in 0..num_customers {
        // Distribute customers around a point
        let location = geo::Point::new(
            -122.4 + (i as f64 * 0.001),
            37.8 + (i as f64 * 0.001),
        );

        let customer = Customer::with_params(
            location,
            0.2, // 20% chance to order per tick
            chrono::Duration::minutes(15),
        );

        population.add_agent(customer);
    }

    runner.set_population_handler(population);
}

/// Process demands and create orders
pub fn process_demands(
    runner: &mut SimulationRunner<Customer, Kitchen, Order, NoWorkflow>,
    results: &[AgentTickResult],
) -> sim_core::Result<()> {
    if let Some(allocator) = runner.resources_mut() {
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "food_order" {
                    let order_id = demand
                        .metadata
                        .get("order_id")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let customer_id = demand
                        .metadata
                        .get("customer_id")
                        .and_then(|v: &serde_json::Value| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let items: Vec<String> = demand
                        .metadata
                        .get("items")
                        .and_then(|v: &serde_json::Value| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();

                    let order = Order::new(order_id, customer_id, items);
                    allocator.add_task(order);
                }
            }
        }

        // Allocate orders to kitchens
        let current_time = chrono::Utc::now();
        let _assignments = allocator.assign_tasks(&LeastLoaded, current_time)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_simulation() {
        let runner = build_simulation(2, 5, false);

        assert_eq!(runner.context().config().name, "ghost-kitchen");
        assert!(runner.resources().is_some());
    }

    #[test]
    fn test_add_customers() {
        let mut runner = build_simulation(2, 5, false);
        add_customers(&mut runner, 10);

        assert!(runner.population().is_some());
        assert_eq!(runner.population().unwrap().count(), 10);
    }

    #[test]
    fn test_hooks_creation() {
        let hooks = KitchenSimulationHooks::new();
        assert_eq!(hooks.dispatcher.buffered_count(), 0);
    }
}
