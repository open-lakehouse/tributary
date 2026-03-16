# Agents

**Last Updated:** March 16, 2026

## Overview

Agents are the fundamental entities in Tributary simulations. They represent autonomous actors that exist in the simulation environment, exhibit behaviors, and respond to stimuli. Agents implement the `Agent` trait and participate in the tick-based simulation loop.

## Objective

This document explains:
- The Agent trait and its requirements
- Agent lifecycle and state management
- How to implement custom agents
- Common patterns and best practices
- Integration with the simulation framework

## Prerequisites

- Basic understanding of Rust traits
- Familiarity with [Getting Started](../GETTING_STARTED.md)
- Knowledge of simulation concepts

## The Agent Trait

### Core Definition

```rust
pub trait Agent: Send + Sync {
    /// Unique identifier for this agent
    fn id(&self) -> AgentId;

    /// Type name for this agent (e.g., "customer", "driver")
    fn agent_type(&self) -> &str;

    /// Current status of this agent
    fn status(&self) -> &dyn AgentStatus;

    /// Current position in 2D space (optional)
    fn position(&self) -> Option<Point<f64>>;

    /// Execute one simulation tick
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult>;
}
```

### AgentStatus Trait

All agent status types must implement `AgentStatus`:

```rust
pub trait AgentStatus: Debug {
    /// String representation of status
    fn as_str(&self) -> &str;

    /// Check if agent is idle (available for work)
    fn is_idle(&self) -> bool;
}
```

### AgentTickResult

The `tick()` method returns an `AgentTickResult` containing:

```rust
pub struct AgentTickResult {
    /// Journey requests (movement)
    pub journeys: Vec<JourneyState>,

    /// Demand requests (work to be done)
    pub demands: Vec<Demand>,

    /// State change metadata
    pub state_changes: Vec<StateChange>,
}
```

## Agent Lifecycle

### 1. Creation

Agents are created with unique IDs and initial state:

```rust
#[derive(Debug)]
pub struct Customer {
    id: AgentId,
    status: CustomerStatus,
    location: Point<f64>,
}

impl Customer {
    pub fn new(location: Point<f64>) -> Self {
        Self {
            id: AgentId::new(), // UUID v7 (time-ordered)
            status: CustomerStatus::Idle,
            location,
        }
    }
}
```

### 2. Registration

Agents are added to the PopulationHandler:

```rust
let mut population = PopulationHandler::new();
population.add_agent(Box::new(customer));
```

### 3. Tick Execution

Each simulation step calls `tick()` on all agents:

```rust
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let mut result = AgentTickResult::default();

    // Agent logic here
    match self.status {
        CustomerStatus::Idle => {
            // Check conditions, potentially create demands
        }
        CustomerStatus::Working => {
            // Update progress, potentially complete work
        }
    }

    Ok(result)
}
```

### 4. State Updates

Agents update their state during ticks and in response to external events:

```rust
// Internal state transition
self.status = CustomerStatus::WaitingForFood;

// External state update (via workflow completion)
// Handled by simulation hooks
```

### 5. Completion

Agents reach terminal states or are removed from the simulation:

```rust
match self.status {
    CustomerStatus::Done => {
        // Agent lifecycle complete
        // Can be removed or recycled
    }
    _ => { /* Continue processing */ }
}
```

## Implementation Examples

### Example 1: Simple Customer Agent

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
    hunger_level: f64,
}

#[derive(Debug, Clone)]
pub enum CustomerStatus {
    Idle,
    Hungry,
    Ordering,
    WaitingForFood,
    Eating,
    Done,
}

impl AgentStatus for CustomerStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Hungry => "hungry",
            Self::Ordering => "ordering",
            Self::WaitingForFood => "waiting_for_food",
            Self::Eating => "eating",
            Self::Done => "done",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle | Self::Hungry)
    }
}

impl Customer {
    pub fn new(location: Point<f64>, order_probability: f64) -> Self {
        Self {
            id: AgentId::new(),
            status: CustomerStatus::Idle,
            location,
            order_probability,
            hunger_level: 0.0,
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
                // Hunger increases over time
                self.hunger_level += 0.01;

                if self.hunger_level > 0.5 {
                    self.status = CustomerStatus::Hungry;
                }
            }

            CustomerStatus::Hungry => {
                // Probabilistically decide to order
                let mut rng = rand::thread_rng();
                if rng.gen::<f64>() < self.order_probability {
                    self.status = CustomerStatus::Ordering;

                    // Create demand
                    result.demands.push(sim_core::Demand {
                        demand_type: "food_order".to_string(),
                        location: Some(self.location),
                        agent_id: Some(self.id),
                        created_at: ctx.current_time(),
                        metadata: std::collections::HashMap::new(),
                    });
                }
            }

            CustomerStatus::Ordering => {
                // Transition to waiting (order placed)
                self.status = CustomerStatus::WaitingForFood;
            }

            CustomerStatus::WaitingForFood => {
                // Wait for external event (workflow completion)
            }

            CustomerStatus::Eating => {
                // Decrease hunger while eating
                self.hunger_level -= 0.1;

                if self.hunger_level <= 0.0 {
                    self.hunger_level = 0.0;
                    self.status = CustomerStatus::Done;
                }
            }

            CustomerStatus::Done => {
                // Terminal state
            }
        }

        Ok(result)
    }
}
```

### Example 2: Driver Agent with Movement

```rust
use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};
use sim_geo::JourneyState;
use geo::Point;

#[derive(Debug)]
pub struct Driver {
    id: AgentId,
    status: DriverStatus,
    current_location: Point<f64>,
    home_location: Point<f64>,
    capacity: usize,
    current_load: usize,
}

#[derive(Debug, Clone)]
pub enum DriverStatus {
    Idle,
    EnRouteToPickup,
    Loading,
    EnRouteToDelivery,
    Delivering,
    ReturningHome,
}

impl AgentStatus for DriverStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::EnRouteToPickup => "en_route_to_pickup",
            Self::Loading => "loading",
            Self::EnRouteToDelivery => "en_route_to_delivery",
            Self::Delivering => "delivering",
            Self::ReturningHome => "returning_home",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}

impl Driver {
    pub fn new(home_location: Point<f64>, capacity: usize) -> Self {
        Self {
            id: AgentId::new(),
            status: DriverStatus::Idle,
            current_location: home_location,
            home_location,
            capacity,
            current_load: 0,
        }
    }

    pub fn assign_pickup(&mut self, pickup_location: Point<f64>) -> JourneyState {
        self.status = DriverStatus::EnRouteToPickup;

        JourneyState {
            agent_id: self.id,
            origin: self.current_location,
            destination: pickup_location,
            started_at: chrono::Utc::now(),
            progress: 0.0,
        }
    }
}

impl Agent for Driver {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "driver"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(self.current_location)
    }

    fn tick(&mut self, ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        match self.status {
            DriverStatus::Idle => {
                // Wait for assignment
            }

            DriverStatus::EnRouteToPickup => {
                // Movement handled by MovementHandler
                // Transition occurs when journey completes
            }

            DriverStatus::Loading => {
                // Loading takes time
                // Transition to EnRouteToDelivery when complete
            }

            DriverStatus::EnRouteToDelivery => {
                // Movement handled by MovementHandler
            }

            DriverStatus::Delivering => {
                // Unload packages
                self.current_load = 0;

                // Return home
                self.status = DriverStatus::ReturningHome;

                result.journeys.push(JourneyState {
                    agent_id: self.id,
                    origin: self.current_location,
                    destination: self.home_location,
                    started_at: ctx.current_time(),
                    progress: 0.0,
                });
            }

            DriverStatus::ReturningHome => {
                // Movement handled by MovementHandler
                // Become idle when home
            }
        }

        Ok(result)
    }
}
```

## State Management Patterns

### Pattern 1: Probabilistic State Transitions

Use random number generation for stochastic behavior:

```rust
use rand::Rng;

match self.status {
    Status::Idle => {
        let mut rng = rand::thread_rng();
        if rng.gen::<f64>() < 0.1 {  // 10% chance per tick
            self.status = Status::Active;
        }
    }
    _ => {}
}
```

### Pattern 2: Time-Based State Transitions

Use simulation context for time-dependent behavior:

```rust
match self.status {
    Status::Waiting => {
        if ctx.current_time() >= self.wait_until {
            self.status = Status::Active;
        }
    }
    _ => {}
}
```

### Pattern 3: Counter-Based State Transitions

Track ticks or events for state changes:

```rust
match self.status {
    Status::Processing => {
        self.ticks_in_state += 1;

        if self.ticks_in_state >= self.required_ticks {
            self.status = Status::Complete;
            self.ticks_in_state = 0;
        }
    }
    _ => {}
}
```

### Pattern 4: External Event Handling

Respond to workflow completions or other events:

```rust
// In simulation hooks
impl SimulationHooks for MyHooks {
    fn on_workflow_completed(
        &mut self,
        workflow_instance_id: WorkflowInstanceId,
        agent_id: AgentId,
    ) -> Result<()> {
        // Update agent state
        if let Some(agent) = self.population.get_agent_mut(agent_id) {
            agent.status = CustomerStatus::Eating;
        }
        Ok(())
    }
}
```

## Integration with Framework

### PopulationHandler

The PopulationHandler manages all agents:

```rust
pub struct PopulationHandler {
    agents: RecordBatch,           // Arrow storage
    id_to_row: HashMap<AgentId, usize>, // Fast lookup
}

impl PopulationHandler {
    pub fn add_agent(&mut self, agent: Box<dyn Agent>) { /* ... */ }
    pub fn get_agent(&self, id: AgentId) -> Option<&dyn Agent> { /* ... */ }
    pub fn tick_all(&mut self, ctx: &SimulationContext) -> Vec<AgentTickResult> { /* ... */ }
}
```

### SimulationRunner Integration

The runner coordinates agent execution:

```rust
// In simulation tick loop
let results = population.tick_all(&ctx);

// Process results
for result in results {
    // Handle demands
    for demand in result.demands {
        // Create tasks, start workflows
    }

    // Handle journeys
    for journey in result.journeys {
        movement_handler.add_journey(journey);
    }

    // Handle state changes
    for change in result.state_changes {
        event_dispatcher.emit_state_change(change);
    }
}
```

## Examples from Reference Implementations

### sim-kitchen: Customer Agent

The Customer agent demonstrates:
- Probabilistic ordering behavior
- State machine with 4 states (Idle → Ordering → Waiting → Eating)
- Demand generation for food orders
- Integration with workflow completion

**File:** `crates/sim-kitchen/src/agents/customer.rs`

### sim-kitchen: Courier Agent

The Courier agent demonstrates:
- Movement with journey requests
- Pickup and delivery workflow
- Location-based state transitions
- Integration with MovementHandler

**File:** `crates/sim-kitchen/src/agents/courier.rs`

### sim-logistics: Driver Agent

The Driver agent demonstrates:
- Complex state machine with 6 states
- Capacity management (truck load)
- Multi-leg journeys (pickup → delivery → return)
- H3-based warehouse routing

**File:** `crates/sim-logistics/src/agents/driver.rs`

### sim-logistics: Package Agent

The Package agent demonstrates:
- Passive agent (minimal behavior)
- Status tracking through workflow
- Integration with sorting and loading resources
- Delivery destination management

**File:** `crates/sim-logistics/src/agents/package.rs`

## Performance Considerations

### Arrow Storage

All agents are stored in Arrow RecordBatch for efficiency:

- **Bulk updates**: 7-785x faster with batch operations
- **Memory efficiency**: Columnar storage reduces overhead
- **Query support**: SQL queries across agent population

### Tick Optimization

Minimize work in `tick()` method:

```rust
// ✅ Good - early return
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    if self.status == Status::Done {
        return Ok(AgentTickResult::default());
    }

    // Process active agents
}

// ❌ Bad - unnecessary work
fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
    let mut result = AgentTickResult::default();

    match self.status {
        Status::Done => {
            // Still processing...
        }
        _ => { /* ... */ }
    }

    Ok(result)
}
```

### State Machine Complexity

Keep state machines simple:

- **Good**: 3-6 states with clear transitions
- **Warning**: 7-10 states may indicate complexity
- **Refactor**: 10+ states should be split into sub-agents or workflows

## Common Pitfalls

### Pitfall 1: Forgetting Send + Sync

```rust
// ❌ Wrong - not thread-safe
pub struct MyAgent {
    id: AgentId,
    data: Rc<RefCell<Data>>,  // Not Send + Sync
}

// ✅ Correct - thread-safe
pub struct MyAgent {
    id: AgentId,
    data: Arc<Mutex<Data>>,  // Send + Sync
}
```

### Pitfall 2: Infinite State Loops

```rust
// ❌ Wrong - can loop forever
match self.status {
    Status::A => {
        self.status = Status::B;
        // Process B immediately - recursion!
    }
    Status::B => {
        self.status = Status::A;
    }
}

// ✅ Correct - transition occurs, processing next tick
match self.status {
    Status::A => {
        if condition {
            self.status = Status::B;
        }
    }
    Status::B => {
        // Process in next tick
    }
}
```

### Pitfall 3: Mutable Borrow in Position

```rust
// ❌ Wrong - can't return mutable reference
fn position(&mut self) -> Option<Point<f64>> {
    Some(&mut self.location)  // Error
}

// ✅ Correct - return owned value or immutable reference
fn position(&self) -> Option<Point<f64>> {
    Some(self.location)  // Copy semantics for Point
}
```

### Pitfall 4: Not Implementing AgentStatus

```rust
// ❌ Wrong - using raw enum
pub enum MyStatus {
    Idle,
    Active,
}

// ✅ Correct - implementing AgentStatus
impl AgentStatus for MyStatus {
    fn as_str(&self) -> &str {
        match self {
            Self::Idle => "idle",
            Self::Active => "active",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }
}
```

## Testing Agents

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::SimulationContext;

    #[test]
    fn test_customer_state_transition() {
        let mut customer = Customer::new(Point::new(0.0, 0.0), 1.0);
        let ctx = SimulationContext::default();

        // Test Idle → Hungry
        customer.hunger_level = 0.6;
        customer.tick(&ctx).unwrap();
        assert_eq!(customer.status.as_str(), "hungry");

        // Test Hungry → Ordering (with high probability)
        customer.tick(&ctx).unwrap();
        assert_eq!(customer.status.as_str(), "ordering");
    }

    #[test]
    fn test_agent_demand_generation() {
        let mut customer = Customer::new(Point::new(0.0, 0.0), 1.0);
        customer.status = CustomerStatus::Hungry;

        let ctx = SimulationContext::default();
        let result = customer.tick(&ctx).unwrap();

        assert_eq!(result.demands.len(), 1);
        assert_eq!(result.demands[0].demand_type, "food_order");
    }
}
```

## Related Documentation

- [Resources](resources.md) - Resource allocation and task processing
- [Workflows](workflows.md) - Multi-step process orchestration
- [Spatial Operations](spatial.md) - Movement and geospatial features
- [Events](events.md) - CloudEvents emission from agents

## Summary

Agents are the core of Tributary simulations. Key takeaways:

- Implement the `Agent` trait with type-safe IDs and status
- Use state machines for behavior modeling
- Return demands, journeys, and state changes from `tick()`
- Leverage Arrow storage for performance
- Follow examples from sim-kitchen and sim-logistics
- Test agent behavior in isolation and integration

---

**Last Updated:** March 16, 2026
