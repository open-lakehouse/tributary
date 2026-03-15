# sim-core

Generic agent-based simulation framework.

## Overview

`sim-core` provides the core abstractions for building agent-based simulations. It is designed to be completely domain-agnostic, allowing you to model various scenarios including logistics, retail, manufacturing, services, and more.

## Features

- **Generic Agent System** - Define any type of agent with custom behavior
- **Resource Allocation** - Capacity-constrained processors with pluggable strategies
- **Multi-Stage Workflows** - Sequential processing pipelines
- **Type-Safe Identifiers** - UUID-based IDs for all entity types
- **Extensible Design** - Trait-based architecture for maximum flexibility

## Core Concepts

### Agents

Agents are entities that exist in the simulation and have behavior. Implement the `Agent` trait for your domain:

```rust
use sim_core::{Agent, AgentId, AgentStatus, AgentTickResult, SimulationContext};

struct Customer {
    id: AgentId,
    // ... your fields
}

impl Agent for Customer {
    fn id(&self) -> AgentId { self.id }
    fn agent_type(&self) -> &str { "customer" }
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        // Update agent state
        Ok(AgentTickResult::default())
    }
    // ... other trait methods
}
```

### Resources

Resources are capacity-constrained processors (machines, stations, workers, etc.):

```rust
use sim_core::{Resource, ResourceId};

struct Kitchen {
    id: ResourceId,
    capacity: usize,
    current_load: usize,
}

impl Resource for Kitchen {
    fn id(&self) -> ResourceId { self.id }
    fn resource_type(&self) -> &str { "kitchen" }
    fn capacity(&self) -> usize { self.capacity }
    fn current_load(&self) -> usize { self.current_load }
}
```

### Workflows

Workflows represent multi-stage processing:

```rust
use sim_core::{Workflow, WorkflowId, WorkflowStep};

struct CookingWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl Workflow for CookingWorkflow {
    fn id(&self) -> WorkflowId { self.id }
    fn workflow_type(&self) -> &str { "cooking" }
    fn steps(&self) -> &[WorkflowStep] { &self.steps }
}
```

## Population Management

Manage your agents with the generic `PopulationHandler`:

```rust
use sim_core::{PopulationHandler, SimulationContext, SimulationConfig};

let config = SimulationConfig::default();
let ctx = SimulationContext::from_now(config);
let mut population = PopulationHandler::<Customer>::new();

population.add_agent(customer);

// Tick all agents
let results = population.tick_agents(&ctx)?;
```

## Resource Allocation

Allocate tasks to resources using built-in strategies:

```rust
use sim_core::{ResourceAllocator, LeastLoaded};

let mut allocator = ResourceAllocator::<Kitchen, Order>::new();

allocator.add_resource(kitchen);
allocator.add_task(order);

// Assign using least-loaded strategy
let assignments = allocator.assign_tasks(&LeastLoaded)?;
```

Available strategies:
- `RoundRobin` - Rotate through resources
- `LeastLoaded` - Assign to least busy resource
- `Random` - Random assignment

You can also implement custom strategies via the `AllocationStrategy` trait.

## Workflow Execution

Manage multi-stage workflows:

```rust
use sim_core::WorkflowEngine;

let mut engine = WorkflowEngine::<CookingWorkflow>::new();

engine.register_workflow(workflow);
let instance = engine.start_instance(workflow_id)?;

// Advance based on time
let completed = engine.advance_instances(&ctx)?;
```

## Testing

The crate includes comprehensive tests:

```bash
cargo test -p sim-core
```

All tests pass with full coverage of core functionality.

## Documentation

View the full API documentation:

```bash
cargo doc -p sim-core --open
```

## License

See the workspace LICENSE file.
