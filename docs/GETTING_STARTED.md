# Getting Started with Tributary

**Last Updated:** March 16, 2026

This guide will walk you through building your first simulation with Tributary. By the end, you'll have a working agent-based simulation with resources, workflows, and event emission.

## Prerequisites

### Required

- **Rust 1.95 or later** - [Install Rust](https://rustup.rs/)
- **Basic Rust knowledge** - Familiarity with structs, traits, and ownership
- **Git** - For cloning the repository

### Optional

- **Python 3.x** - For Python bindings (Phase 12+)
- **VS Code** with rust-analyzer - Recommended IDE setup

## Installation

### Clone the Repository

```bash
git clone https://github.com/open-lakehouse/tributary.git
cd tributary
```

### Verify Installation

```bash
# Run all tests
cargo test --workspace

# Check code quality
cargo clippy --workspace

# Build the project
cargo build --release
```

Expected output:
- 920 tests passing
- Zero clippy warnings
- Successful build

## Your First Simulation

Let's build a simple simulation step-by-step. We'll create a coffee shop simulation with customers who order drinks and baristas who prepare them.

### Step 1: Create a New Crate

```bash
cargo new --lib sim-coffee
cd sim-coffee
```

### Step 2: Add Dependencies

Edit `Cargo.toml`:

```toml
[package]
name = "sim-coffee"
version = "0.1.0"
edition = "2021"

[dependencies]
sim-core = { path = "../crates/sim-core" }
sim-events = { path = "../crates/sim-events" }
chrono = { version = "0.4", features = ["serde"] }
geo = "0.28"
serde_json = "1.0"
rand = "0.8"
```

### Step 3: Define Your Agent

Create `src/customer.rs`:

```rust
use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};
use geo::Point;
use rand::Rng;

#[derive(Debug)]
pub struct Customer {
    id: AgentId,
    status: CustomerStatus,
    location: Point<f64>,
    order_probability: f64,
}

#[derive(Debug, Clone)]
pub enum CustomerStatus {
    Idle,
    Ordering,
    Waiting,
    Done,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Ordering => "ordering",
            Self::Waiting => "waiting",
            Self::Done => "done",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

impl Customer {
    pub fn new(location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(),
            status: CustomerStatus::Idle,
            location,
            order_probability: 0.1, // 10% chance per tick
        }
    }
}

impl Agent for Customer {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "customer"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.location)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match self.status {
            CustomerStatus::Idle => {
                // Probabilistically decide to order
                let mut rng = rand::thread_rng();
                if rng.gen::<f64>() < self.order_probability {
                    self.status = CustomerStatus::Ordering;

                    // Create a demand (coffee order)
                    result.demands.push(sim_core::Demand {
                        demand_type: "coffee_order".to_string(),
                        location: Some(self.location),
                        agent_id: Some(self.id),
                        created_at: ctx.current_time(),
                        metadata: std::collections::HashMap::new(),
                    });

                    println!("[{}] Customer {} is ordering coffee",
                             ctx.current_time().format("%H:%M:%S"),
                             self.id);
                }
            }
            CustomerStatus::Ordering => {
                // Transition to waiting after order is placed
                self.status = CustomerStatus::Waiting;
            }
            CustomerStatus::Waiting => {
                // Will transition to Done when order completes
                // (handled by workflow completion)
            }
            CustomerStatus::Done => {
                // Customer is done
            }
        }

        Ok(result)
    }
}
```

### Step 4: Define Your Resource

Create `src/barista.rs`:

```rust
use sim_core::{Resource, ResourceId, Task, TaskId};
use chrono::Duration;

#[derive(Debug)]
pub struct Barista {
    id: ResourceId,
    capacity: usize,
    current_load: usize,
}

impl Barista {
    pub fn new(capacity: usize) -> Self {
        Self {
            id: ResourceId::new(),
            capacity,
            current_load: 0,
        }
    }
}

impl Resource for Barista {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "barista"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.current_load
    }

    fn available_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.current_load)
    }
}

#[derive(Debug)]
pub struct CoffeeOrder {
    id: TaskId,
    customer_id: AgentId,
    drink_type: String,
}

impl CoffeeOrder {
    pub fn new(customer_id: AgentId, drink_type: String) -> Self {
        Self {
            id: TaskId::new(),
            customer_id,
            drink_type,
        }
    }
}

impl Task for CoffeeOrder {
    fn id(&self) -> TaskId {
        self.id
    }

    fn task_type(&self) -> &str {
        "coffee_order"
    }

    fn required_resource_type(&self) -> &str {
        "barista"
    }

    fn estimated_duration(&self) -> Duration {
        // Coffee takes 2 minutes to make
        Duration::minutes(2)
    }

    fn priority(&self) -> i32 {
        0 // FIFO ordering
    }
}
```

### Step 5: Define Events

Create `src/events.rs`:

```rust
use sim_core::{AgentId, TaskId};
use sim_events::EventPayload;
use serde_json::json;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct OrderCreatedEvent {
    pub order_id: TaskId,
    pub customer_id: AgentId,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.coffee.order.created"
    }

    fn source(&self) -> String {
        format!("/customers/{}", self.customer_id)
    }

    fn subject(&self) -> Option<String> {
        Some(self.order_id.to_string())
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "order_id": self.order_id.to_string(),
            "customer_id": self.customer_id.to_string(),
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}

#[derive(Debug)]
pub struct OrderCompletedEvent {
    pub order_id: TaskId,
    pub customer_id: AgentId,
    pub timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCompletedEvent {
    fn event_type(&self) -> &str {
        "io.coffee.order.completed"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "order_id": self.order_id.to_string(),
            "customer_id": self.customer_id.to_string(),
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

### Step 6: Create Simulation Hooks

Create `src/hooks.rs`:

```rust
use sim_core::{SimulationContext, SimulationHooks, AgentTickResult};
use sim_events::{Event, EventDispatcher};
use crate::events::{OrderCreatedEvent, OrderCompletedEvent};

pub struct CoffeeShopHooks {
    pub dispatcher: EventDispatcher,
}

impl CoffeeShopHooks {
    pub fn new() -> Self {
        Self {
            dispatcher: EventDispatcher::new(),
        }
    }
}

impl SimulationHooks for CoffeeShopHooks {
    fn on_start(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!("☕ Coffee Shop Simulation Starting");
        println!("   Time: {}", ctx.current_time());
        println!("   Time step: {} seconds", ctx.time_step().num_seconds());
        println!();
        Ok(())
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        // Emit events for new orders
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "coffee_order" {
                    if let Some(agent_id) = demand.agent_id {
                        let event = Event::new(Box::new(OrderCreatedEvent {
                            order_id: TaskId::new(), // Would be actual task ID in real impl
                            customer_id: agent_id,
                            timestamp: ctx.current_time(),
                        }), ctx.current_time());
                        self.dispatcher.emit(event);
                    }
                }
            }
        }
        Ok(())
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        if ctx.step() % 10 == 0 {
            println!("[Step {}] Elapsed: {:?}", ctx.step(), ctx.elapsed());
            println!("  Events emitted: {}", self.dispatcher.buffered_count());
        }
        Ok(())
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        println!();
        println!("☕ Coffee Shop Simulation Complete");
        println!("   Final time: {}", ctx.current_time());
        println!("   Total steps: {}", ctx.step());
        println!("   Total events: {}", self.dispatcher.buffered_count());

        // Write events to file
        let count = self.dispatcher.flush_to_parquet("coffee_events.parquet")?;
        println!("   Wrote {} events to coffee_events.parquet", count);

        Ok(())
    }
}
```

### Step 7: Build the Simulation

Create `src/lib.rs`:

```rust
mod customer;
mod barista;
mod events;
mod hooks;

pub use customer::{Customer, CustomerStatus};
pub use barista::{Barista, CoffeeOrder};
pub use hooks::CoffeeShopHooks;

use sim_core::{
    SimulationRunner, SimulationConfig, PopulationHandler, ResourceAllocator,
};
use chrono::Duration;
use geo::Point;

pub fn build_coffee_shop(
    num_baristas: usize,
    barista_capacity: usize,
    num_customers: usize,
) -> SimulationRunner {
    // Configure simulation
    let config = SimulationConfig {
        name: "coffee-shop".to_string(),
        time_step: Duration::seconds(30), // 30-second steps
        verbose: false,
        custom: Default::default(),
    };

    let mut runner = SimulationRunner::new(config);

    // Add baristas (resources)
    let mut allocator = ResourceAllocator::new();
    for _ in 0..num_baristas {
        allocator.add_resource(Box::new(Barista::new(barista_capacity)));
    }
    runner.set_resource_allocator(allocator);

    // Add customers (agents)
    let mut population = PopulationHandler::new();
    for i in 0..num_customers {
        // Place customers in a grid around the coffee shop
        let x = (i as f64 % 10.0) * 0.001;
        let y = (i as f64 / 10.0) * 0.001;
        let location = Point::new(-122.4 + x, 37.8 + y);

        population.add_agent(Box::new(Customer::new(location)));
    }
    runner.set_population_handler(population);

    // Add hooks for event emission
    runner.set_hooks(Box::new(CoffeeShopHooks::new()));

    runner
}
```

### Step 8: Create an Example

Create `examples/coffee_shop.rs`:

```rust
use sim_coffee::build_coffee_shop;
use chrono::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("☕ Building Coffee Shop Simulation...");
    println!();

    // Create simulation
    // 3 baristas, capacity 2 each (can make 2 coffees simultaneously)
    // 20 customers
    let mut runner = build_coffee_shop(3, 2, 20);

    // Run for 100 steps (50 minutes with 30s steps)
    println!("Running simulation...");
    runner.run_for_steps(100)?;

    // Stats
    let stats = runner.stats();
    println!();
    println!("📊 Statistics:");
    println!("   Total steps: {}", stats.total_steps);
    println!("   Agent ticks: {}", stats.total_agent_ticks);
    println!("   Assignments: {}", stats.total_assignments);

    Ok(())
}
```

### Step 9: Run Your Simulation

```bash
cargo run --example coffee_shop
```

Expected output:
```
☕ Building Coffee Shop Simulation...

Running simulation...
☕ Coffee Shop Simulation Starting
   Time: 2026-03-16 10:00:00 UTC
   Time step: 30 seconds

[10:00:30] Customer 019cf220-... is ordering coffee
[10:01:00] Customer 019cf220-... is ordering coffee
[Step 10] Elapsed: 5 minutes
  Events emitted: 2
[Step 20] Elapsed: 10 minutes
  Events emitted: 5
...

☕ Coffee Shop Simulation Complete
   Final time: 2026-03-16 10:50:00 UTC
   Total steps: 100
   Total events: 15
   Wrote 15 events to coffee_events.parquet

📊 Statistics:
   Total steps: 100
   Agent ticks: 2000
   Assignments: 15
```

## Next Steps

### Add Workflows

Create multi-step processes for coffee preparation:

```rust
use sim_core::{Workflow, WorkflowStep, WorkflowId};
use std::collections::HashMap;

#[derive(Debug)]
struct CoffeeWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl CoffeeWorkflow {
    fn new() -> Self {
        Self {
            id: WorkflowId::new(),
            steps: vec![
                WorkflowStep {
                    step_index: 0,
                    required_resource_type: "barista".to_string(),
                    expected_duration: Duration::minutes(1),
                    metadata: HashMap::from([
                        ("step_name".to_string(), "grind_beans".to_string()),
                    ]),
                },
                WorkflowStep {
                    step_index: 1,
                    required_resource_type: "barista".to_string(),
                    expected_duration: Duration::minutes(1),
                    metadata: HashMap::from([
                        ("step_name".to_string(), "brew".to_string()),
                    ]),
                },
            ],
        }
    }
}

impl Workflow for CoffeeWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "coffee_preparation"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}
```

### Add Geospatial Features

Track customer locations and movement:

```rust
use sim_geo::{LocationHandler, MovementHandler, JourneyState};
use h3o::Resolution;

// In your simulation setup
let mut locations = LocationHandler::new(Resolution::Nine);
let mut movement = MovementHandler::new();

// In your hooks
impl SimulationHooks for CoffeeShopHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        // Process journey requests
        for result in results {
            for journey in &result.journeys {
                self.movement.add_journey(journey.clone());
            }
        }

        // Advance all journeys
        self.movement.advance_journeys(ctx.current_time(), ctx.time_step())?;

        Ok(())
    }
}
```

### Add Demand Generation

Use probabilistic demand patterns:

```rust
use sim_core::demand::{PoissonDemandGenerator, TimeOfDayDemandGenerator};

// In your simulation setup
let mut runner = build_coffee_shop(3, 2, 20);

// Add Poisson demand (10 orders per hour average)
let poisson_gen = PoissonDemandGenerator::new(10.0);
runner.add_demand_generator(Box::new(poisson_gen));

// Or use time-based demand with rush hours
let tod_gen = TimeOfDayDemandGenerator::with_peaks(vec![
    PeakPeriod { start_hour: 7, end_hour: 9, rate_multiplier: 3.0 },   // Morning
    PeakPeriod { start_hour: 12, end_hour: 14, rate_multiplier: 2.0 }, // Lunch
]);
runner.add_demand_generator(Box::new(tod_gen));
```

### Add Telemetry

Generate observability data:

```rust
use sim_telemetry::{TelemetryCollector, EventToLogGenerator};

// In your hooks
pub struct CoffeeShopHooks {
    pub dispatcher: EventDispatcher,
    pub telemetry: TelemetryCollector,
}

impl CoffeeShopHooks {
    pub fn new() -> Self {
        let mut telemetry = TelemetryCollector::new();
        telemetry.add_generator(Box::new(EventToLogGenerator::new()));

        Self {
            dispatcher: EventDispatcher::new(),
            telemetry,
        }
    }
}

impl SimulationHooks for CoffeeShopHooks {
    fn on_end(&mut self, ctx: &SimulationContext) -> sim_core::Result<()> {
        // Export telemetry
        let records = self.telemetry.flush();
        println!("Generated {} telemetry records", records.len());

        // Write to JSON
        let json = serde_json::to_string_pretty(&records)?;
        std::fs::write("telemetry.json", json)?;

        Ok(())
    }
}
```

## Common Patterns

### Pattern 1: Probabilistic Behavior

```rust
use rand::Rng;

fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let mut rng = rand::thread_rng();

    if rng.gen::<f64>() < 0.1 {  // 10% probability
        // Take action
    }

    Ok(AgentTickResult::default())
}
```

### Pattern 2: Time-Based Behavior

```rust
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let hour = ctx.current_time().hour();

    let probability = match hour {
        7..=9 => 0.3,    // Morning rush
        12..=14 => 0.25, // Lunch rush
        _ => 0.05,       // Off-peak
    };

    // Use probability...
    Ok(AgentTickResult::default())
}
```

### Pattern 3: State Transitions

```rust
match self.status {
    Status::Idle => {
        if should_transition() {
            self.status = Status::Active;
            // Emit state change
        }
    }
    Status::Active => {
        if is_complete() {
            self.status = Status::Done;
        }
    }
    Status::Done => {
        // Terminal state
    }
}
```

## Common Pitfalls

### Pitfall 1: Forgetting to Return AgentTickResult

```rust
// ❌ Wrong
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    self.do_something();
    // Missing: Ok(AgentTickResult::default())
}

// ✅ Correct
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let mut result = AgentTickResult::default();
    self.do_something();
    Ok(result)
}
```

### Pitfall 2: Not Cloning Shared State

```rust
// ❌ Wrong - moves the location
result.journeys.push(JourneyState {
    origin: self.location,  // Error: move
    destination: target,
});

// ✅ Correct - clones the point
result.journeys.push(JourneyState {
    origin: self.location.clone(),
    destination: target,
});
```

### Pitfall 3: Infinite Loops in State Machines

```rust
// ❌ Wrong - can loop forever
match self.status {
    Status::A => {
        self.status = Status::B;
        // Process B in same tick - can cause infinite recursion
    }
    Status::B => {
        self.status = Status::A;
    }
}

// ✅ Correct - transition happens next tick
match self.status {
    Status::A => {
        if condition_met {
            self.status = Status::B;
        }
    }
    Status::B => {
        // Process in next tick
    }
}
```

## Learning Resources

### Documentation

- [Architecture Guide](ARCHITECTURE.md) - Design principles
- [Agents Concept](concepts/agents.md) - Agent patterns
- [Resources Concept](concepts/resources.md) - Resource allocation
- [Workflows Concept](concepts/workflows.md) - Multi-step processes

### Examples

- [sim-kitchen](../crates/sim-kitchen/) - Ghost kitchen reference implementation
- [sim-logistics](../crates/sim-logistics/) - Package delivery implementation

## Testing Your Simulation

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_ordering() {
        let mut customer = Customer::new(Point::new(0.0, 0.0));
        let ctx = SimulationContext::default();

        // Force ordering state
        customer.status = CustomerStatus::Ordering;

        let result = customer.tick(&ctx).unwrap();
        assert_eq!(customer.status.as_str(), "waiting");
    }

    #[test]
    fn test_barista_capacity() {
        let barista = Barista::new(2);
        assert_eq!(barista.capacity(), 2);
        assert_eq!(barista.current_load(), 0);
        assert_eq!(barista.available_capacity(), 2);
    }
}
```

### Integration Tests

```rust
#[test]
fn test_coffee_shop_simulation() {
    let mut runner = build_coffee_shop(3, 2, 20);

    runner.run_for_steps(100).unwrap();

    let stats = runner.stats();
    assert_eq!(stats.total_steps, 100);
    assert_eq!(stats.total_agent_ticks, 2000); // 20 agents * 100 steps
}
```

## Troubleshooting

### Issue: Agents Not Ticking

**Symptom:** Agent count is 0 or agents don't execute
**Solution:** Ensure you call `runner.set_population_handler(population)`

### Issue: Resources Not Allocating

**Symptom:** Tasks created but never assigned
**Solution:** Ensure you call `runner.set_resource_allocator(allocator)`

### Issue: Events Not Written

**Symptom:** Parquet file is empty or missing
**Solution:** Ensure you call `dispatcher.flush_to_parquet()` in `on_end` hook

### Issue: Compilation Errors with Traits

**Symptom:** "trait bounds not satisfied" errors
**Solution:** Ensure your types implement required traits (Agent, Resource, Task, etc.)

## What's Next?

You've built your first simulation! Here are some next steps:

1. **Explore Concepts** - Read the concept guides to understand patterns
2. **Study Examples** - Review sim-kitchen and sim-logistics for real-world patterns
3. **Add Complexity** - Implement workflows, demand generation, telemetry
4. **Optimize** - Use Arrow storage and batching for performance
5. **Visualize** - Export events and analyze with your favorite tools

Happy simulating! 🚀
