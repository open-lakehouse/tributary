//! Ghost Kitchen Simulation - Complete Reference Implementation
//!
//! This crate demonstrates how to build a complete domain-specific simulation using the
//! sim-core, sim-geo, and sim-events framework. It serves as the reference implementation
//! showing all framework capabilities and best practices.
//!
//! # Overview
//!
//! A ghost kitchen simulation models customers ordering food, kitchens preparing orders,
//! and the complete lifecycle from order placement to completion. The simulation emits
//! CloudEvents for all state transitions, enabling observability and analytics.
//!
//! # Domain Model
//!
//! ## Agents: Customers
//!
//! Customers are autonomous agents that progress through a state machine:
//!
//! - **Idle**: Initial state, may decide to place an order (probabilistic)
//! - **Ordering**: Creating a food order (emits demand)
//! - **WaitingForFood**: Passive state while order is being prepared
//! - **Eating**: Consuming the food (timed duration)
//! - **Done**: Finished, ready for removal from simulation
//!
//! ## Resources: Kitchens
//!
//! Kitchens are capacity-constrained resources that process orders:
//!
//! - Each kitchen has a maximum capacity (concurrent orders)
//! - Load is automatically tracked by the framework's ResourceAllocator
//! - Orders are assigned using allocation strategies (e.g., LeastLoaded)
//!
//! ## Tasks: Orders
//!
//! Orders are tasks that flow through the system:
//!
//! - Created by customers when they transition to Ordering state
//! - Assigned to kitchens by the ResourceAllocator
//! - Have estimated preparation time based on items
//! - Completed after prep time elapses
//!
//! ## Events: CloudEvents
//!
//! All state transitions emit CloudEvents:
//!
//! - `io.kitchen.order.created` - Customer places order
//! - `io.kitchen.order.assigned` - Order assigned to kitchen
//! - `io.kitchen.order.completed` - Order finished cooking
//! - `io.kitchen.customer.eating` - Customer receives food
//! - `io.kitchen.customer.done` - Customer finished
//!
//! # Complete Lifecycle Example
//!
//! ```rust,no_run
//! use sim_kitchen::KitchenSimulation;
//!
//! // Create simulation with 3 kitchens, 5 capacity each
//! let mut sim = KitchenSimulation::new(3, 5, true);
//!
//! // Add 20 customers
//! sim.add_customers(20);
//!
//! // Run for 100 steps (50 minutes simulated time at 30s/step)
//! sim.run_for_steps(100).unwrap();
//!
//! // Simulation handles:
//! // - Agent ticking (customer state machine)
//! // - Demand processing (order creation)
//! // - Resource allocation (kitchen assignment)
//! // - Task completion (order finishing)
//! // - Event emission (CloudEvents for all transitions)
//! ```
//!
//! # Architecture Patterns
//!
//! This implementation demonstrates key framework patterns:
//!
//! ## 1. Agent State Machine
//!
//! ```rust,ignore
//! impl Agent for Customer {
//!     fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
//!         match &self.status {
//!             CustomerStatus::Idle => {
//!                 // Probabilistic transition to Ordering
//!             }
//!             CustomerStatus::Ordering => {
//!                 // Emit demand for food
//!             }
//!             CustomerStatus::Eating { finish_time } => {
//!                 // Check if eating complete
//!             }
//!             // ...
//!         }
//!     }
//! }
//! ```
//!
//! ## 2. Resource Allocation
//!
//! The framework's ResourceAllocator automatically:
//! - Tracks resource capacity and load
//! - Assigns tasks using pluggable strategies
//! - Updates capacity when tasks start/complete
//!
//! ```rust,ignore
//! let mut allocator = ResourceAllocator::new();
//! allocator.add_resource(kitchen)?;
//! allocator.add_task(order)?;
//! let assignments = allocator.assign_tasks(&LeastLoaded, current_time)?;
//! ```
//!
//! ## 3. Simulation Hooks
//!
//! Hooks provide lifecycle callbacks for custom logic:
//!
//! ```rust,ignore
//! impl SimulationHooks for KitchenSimulationHooks {
//!     fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) {
//!         // Process demands (orders) from agent ticks
//!     }
//!
//!     fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) {
//!         // Track order assignments, emit events
//!     }
//! }
//! ```
//!
//! ## 4. Event Emission
//!
//! CloudEvents provide observability:
//!
//! ```rust,ignore
//! let event = Event::new(Box::new(OrderCreatedEvent {
//!     order_id: order_id.clone(),
//!     customer_id: customer_id.to_string(),
//!     items: vec!["burger".to_string()],
//!     timestamp: ctx.current_time(),
//! }), ctx.current_time());
//! dispatcher.emit(event);
//! ```
//!
//! # Module Organization
//!
//! - [`agents`] - Customer agent implementation and state machine
//! - [`resources`] - Kitchen and Order resource/task types
//! - [`events`] - CloudEvent payloads for all transitions
//! - [`simulation`] - Orchestration, hooks, and helper functions
//!
//! # Extending the Pattern
//!
//! To create a similar simulation for a different domain:
//!
//! 1. **Define your domain types**: Replace Customer, Kitchen, Order with your entities
//! 2. **Implement trait**: Make your types implement Agent, Resource, Task
//! 3. **Create events**: Define EventPayload types for state transitions
//! 4. **Add hooks**: Implement SimulationHooks for domain-specific logic
//! 5. **Build runner**: Use SimulationRunner to orchestrate everything
//!
//! See the `examples/` directory for complete working examples.

pub mod agents;
pub mod artifact_generators;
pub mod events;
pub mod locations; // Agent E: Distribution sites for supply chain
pub mod profiles;
pub mod recipes;
pub mod resources;
pub mod simulation;

pub mod workflows; // Agent G: Supply chain workflow

// Re-exports for clean public API
pub use agents::{Courier, CourierState, Customer, CustomerStatus, DriverState, SupplyDriver};
pub use events::*;
pub use locations::{DistributionSite, OrderFulfillment};
pub use recipes::{Recipe, RecipeRegistry, RecipeStep, StationKind};
pub use resources::{Inventory, InventoryError, InventoryManager, Kitchen, KitchenStation, Order};
pub use simulation::{KitchenSimulation, KitchenSimulationHooks, add_customers, build_simulation};
pub use workflows::{
    CourierDeliveryTask, CourierDeliveryTaskFactory, CourierDeliveryWorkflow,
    SupplyChainTaskFactory, SupplyChainWorkflow, SupplyDeliveryTask,
};
