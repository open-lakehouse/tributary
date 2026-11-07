# Basic Ghost Kitchen Example

A minimal working example showing a ghost kitchen with customers ordering food.

## Overview

This example demonstrates:
- Customer agents with simple state machine
- Kitchen resources with capacity constraints
- Order tasks
- Basic event emission

## Code

See `crates/sim-kitchen/examples/basic.rs` for the complete runnable example.

## Domain Model

```
Customer (Agent)          Kitchen (Resource)          Order (Task)
┌────────────┐           ┌──────────────┐           ┌──────────┐
│ Idle       │           │ Capacity: 5  │           │ items    │
│ Ordering   │──creates──│ Load: 0-5    │──processes│ prep_time│
│ Waiting    │           │              │           │          │
│ Eating     │◄──────────│              │           └──────────┘
│ Done       │           └──────────────┘
└────────────┘
```

## Step-by-Step

### 1. Define Agent

```rust
use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};
use geo_types::Point;

#[derive(Clone, Debug)]
enum CustomerStatus {
    Idle,
    Ordering,
    WaitingForFood { order_id: String },
    Eating { finish_time: DateTime<Utc> },
    Done,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            CustomerStatus::Idle => "idle",
            CustomerStatus::Ordering => "ordering",
            CustomerStatus::WaitingForFood { .. } => "waiting",
            CustomerStatus::Eating { .. } => "eating",
            CustomerStatus::Done => "done",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, CustomerStatus::Idle)
    }
}

#[derive(Clone)]
struct Customer {
    id: AgentId,
    status: CustomerStatus,
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
        None  // Non-spatial
    }

    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match &self.status {
            CustomerStatus::Idle => {
                // 10% chance to order each tick
                if rand::random::<f64>() < 0.1 {
                    self.status = CustomerStatus::Ordering;
                }
            }
            CustomerStatus::Ordering => {
                // Emit demand for food
                result.demands.push(Demand {
                    demand_type: "food_order".to_string(),
                    location: None,
                    metadata: HashMap::from([
                        ("customer_id".to_string(), json!(self.id.to_string())),
                        ("items".to_string(), json!(["burger", "fries"])),
                    ]),
                });

                let order_id = format!("order-{}", Uuid::new_v4());
                self.status = CustomerStatus::WaitingForFood { order_id };
            }
            CustomerStatus::Eating { finish_time } => {
                if ctx.current_time() >= *finish_time {
                    self.status = CustomerStatus::Done;
                }
            }
            _ => {}
        }

        Ok(result)
    }

    fn from_arrow_row(row: &ArrowAgentRow) -> Result<Self> {
        let id = AgentId::from_uuid(uuid::Uuid::from_slice(&row.id)?);
        Ok(Customer {
            id,
            status: CustomerStatus::Idle,
        })
    }
}
```

### 2. Define Resource

```rust
use sim_core::{Resource, ResourceId};

#[derive(Debug, Clone)]
struct Kitchen {
    id: ResourceId,
    name: String,
    capacity: usize,
    current_load: usize,
}

impl Resource for Kitchen {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "kitchen"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.current_load
    }
}
```

### 3. Define Task

```rust
use sim_core::{Task, TaskId};
use chrono::Duration;

#[derive(Debug, Clone)]
struct FoodOrder {
    id: TaskId,
    customer_id: String,
    items: Vec<String>,
    prep_time: Duration,
}

impl Task for FoodOrder {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "kitchen"
    }

    fn estimated_duration(&self) -> Duration {
        self.prep_time
    }
}
```

### 4. Define Events

```rust
use sim_events::EventPayload;

#[derive(Debug, Clone, Serialize)]
struct OrderCreatedEvent {
    order_id: String,
    customer_id: String,
    items: Vec<String>,
    timestamp: DateTime<Utc>,
}

impl EventPayload for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "io.kitchen.order.created"
    }

    fn source(&self) -> String {
        format!("/orders/{}", self.order_id)
    }

    fn to_json(&self) -> serde_json::Value {
        json!({
            "order_id": self.order_id,
            "customer_id": self.customer_id,
            "items": self.items,
            "timestamp": self.timestamp.to_rfc3339(),
        })
    }
}
```

### 5. Create Simulation Hooks

```rust
use sim_core::SimulationHooks;
use sim_events::EventDispatcher;

struct KitchenHooks {
    event_dispatcher: EventDispatcher,
    resources: ResourceAllocator<Kitchen, FoodOrder>,
}

impl SimulationHooks for KitchenHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult]
    ) -> Result<()> {
        // Process demands → create orders
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "food_order" {
                    let order_id = format!("order-{}", Uuid::new_v4());
                    let customer_id = demand.metadata.get("customer_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    // Create task
                    let order = FoodOrder {
                        id: TaskId::new(),
                        customer_id: customer_id.to_string(),
                        items: vec!["burger".to_string(), "fries".to_string()],
                        prep_time: Duration::minutes(15),
                    };

                    self.resources.add_task(order)?;

                    // Emit event
                    self.event_dispatcher.emit(Event::new(Box::new(OrderCreatedEvent {
                        order_id,
                        customer_id: customer_id.to_string(),
                        items: vec!["burger".to_string(), "fries".to_string()],
                        timestamp: ctx.current_time(),
                    })))?;
                }
            }
        }

        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment]
    ) -> Result<()> {
        // Emit assignment events
        for assignment in assignments {
            self.event_dispatcher.emit(Event::new(Box::new(OrderAssignedEvent {
                order_id: assignment.task_id.to_string(),
                kitchen_id: assignment.resource_id.to_string(),
                timestamp: ctx.current_time(),
            })))?;
        }

        Ok(())
    }
}
```

### 6. Main Simulation

```rust
use sim_core::{SimulationRunner, SimulationConfig, PopulationHandler, ResourceAllocator, LeastLoaded};

fn main() -> Result<()> {
    // Configuration
    let config = SimulationConfig {
        time_step: Duration::seconds(30),
        ..Default::default()
    };

    // Create runner
    let mut runner = SimulationRunner::new(config);

    // Setup population
    let mut population = PopulationHandler::new();
    for _ in 0..10 {
        population.add_agent(Customer {
            id: AgentId::new(),
            status: CustomerStatus::Idle,
        });
    }

    // Setup resources
    let mut resources = ResourceAllocator::new();
    for i in 0..3 {
        resources.add_resource(Kitchen {
            id: ResourceId::new(),
            name: format!("Kitchen {}", i + 1),
            capacity: 5,
            current_load: 0,
        })?;
    }

    // Setup hooks
    let hooks = KitchenHooks {
        event_dispatcher: EventDispatcher::new(),
        resources: resources.clone(),
    };

    // Register components
    runner.set_population_handler(population);
    runner.set_hooks(Box::new(hooks));

    // Run simulation
    println!("Starting basic kitchen simulation...");
    for step in 0..100 {
        runner.tick()?;

        // Allocate resources every step
        runner.allocate_resources(&LeastLoaded)?;

        if step % 10 == 0 {
            println!("Step {}: {} agents, {} pending orders",
                step,
                runner.population().unwrap().count(),
                runner.resources().unwrap().pending_task_count()
            );
        }
    }

    // Print statistics
    let stats = runner.stats();
    println!("\nSimulation complete:");
    println!("  Total steps: {}", stats.total_steps);
    println!("  Agent ticks: {}", stats.total_agent_ticks);
    println!("  Assignments: {}", stats.total_assignments);

    Ok(())
}
```

## Expected Output

```
Starting basic kitchen simulation...
Step 0: 10 agents, 0 pending orders
Step 10: 10 agents, 2 pending orders
Step 20: 10 agents, 3 pending orders
Step 30: 10 agents, 1 pending orders
...
Step 90: 10 agents, 4 pending orders

Simulation complete:
  Total steps: 100
  Agent ticks: 1000
  Assignments: 47
```

## Key Patterns

1. **Agent State Machine**: Customers transition through states
2. **Demand Generation**: Agents emit demands, hooks convert to tasks
3. **Resource Allocation**: Framework assigns tasks to kitchens
4. **Event Emission**: All state transitions emit CloudEvents

## Extensions

To extend this example:

1. **Add more agent states**: Delivery, payment, etc.
2. **Multi-step workflows**: Prep → Cook → Package
3. **Geospatial delivery**: Add courier agents with journeys
4. **Telemetry**: Generate logs and metrics from events

See [multi-workflow.md](./multi-workflow.md) for advanced patterns.
