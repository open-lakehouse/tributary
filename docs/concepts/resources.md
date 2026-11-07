# Resources

**Last Updated:** March 16, 2026

## Overview

Resources are capacity-constrained processors that handle tasks in Tributary simulations. They represent physical or logical entities that perform work (kitchens, baristas, sorting stations, loading docks) and manage workload through the resource allocation system.

## Objective

This document explains:
- The Resource and Task traits
- Resource allocation strategies
- Task processing patterns
- Capacity management
- Integration with workflows

## Prerequisites

- Understanding of [Agents](agents.md)
- Basic Rust trait knowledge
- [Getting Started](../GETTING_STARTED.md) tutorial completed

## The Resource Trait

### Core Definition

```rust
pub trait Resource: Send + Sync + Debug {
    fn id(&self) -> ResourceId;
    fn resource_type(&self) -> &str;
    fn capacity(&self) -> usize;
    fn current_load(&self) -> usize;
    fn available_capacity(&self) -> usize {
        self.capacity().saturating_sub(self.current_load())
    }
}
```

### The Task Trait

Tasks represent work to be processed by resources:

```rust
pub trait Task: Send + Sync + Debug {
    fn id(&self) -> TaskId;
    fn task_type(&self) -> &str;
    fn required_resource_type(&self) -> &str;
    fn estimated_duration(&self) -> Duration;
    fn priority(&self) -> i32 {
        0  // Default FIFO ordering
    }
}
```

## Resource Allocation System

### ResourceAllocator

The ResourceAllocator matches tasks to resources:

```rust
pub struct ResourceAllocator {
    resources: RecordBatch,              // Arrow storage
    tasks: RecordBatch,                  // Arrow storage
    resource_id_to_row: HashMap<ResourceId, usize>,
    task_id_to_row: HashMap<TaskId, usize>,
}

impl ResourceAllocator {
    pub fn add_resource(&mut self, resource: Box<dyn Resource>);
    pub fn add_task(&mut self, task: Box<dyn Task>);
    pub fn assign_tasks<S>(&mut self, strategy: &S) -> Vec<TaskAssignment>
        where S: AllocationStrategy;
}
```

### Allocation Strategies

#### RoundRobin

Distributes tasks evenly across all resources:

```rust
pub struct RoundRobin {
    next_index: usize,
}

impl AllocationStrategy for RoundRobin {
    fn assign(&self, task: &dyn Task, resources: &[&dyn Resource]) -> Option<ResourceId> {
        let available: Vec<_> = resources
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .collect();

        if available.is_empty() {
            return None;
        }

        let index = self.next_index % available.len();
        Some(available[index].id())
    }
}
```

#### LeastLoaded

Assigns to the resource with the lowest current load:

```rust
pub struct LeastLoaded;

impl AllocationStrategy for LeastLoaded {
    fn assign(&self, task: &dyn Task, resources: &[&dyn Resource]) -> Option<ResourceId> {
        resources
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .min_by_key(|r| r.current_load())
            .map(|r| r.id())
    }
}
```

#### Random

Random assignment for load distribution:

```rust
pub struct Random;

impl AllocationStrategy for Random {
    fn assign(&self, task: &dyn Task, resources: &[&dyn Resource]) -> Option<ResourceId> {
        use rand::seq::SliceRandom;

        let available: Vec<_> = resources
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .collect();

        available
            .choose(&mut rand::thread_rng())
            .map(|r| r.id())
    }
}
```

## Implementation Examples

### Example 1: Kitchen Resource

```rust
use sim_core::{Resource, ResourceId};

#[derive(Debug)]
pub struct Kitchen {
    id: ResourceId,
    capacity: usize,          // Number of concurrent orders
    current_load: usize,      // Active orders
    location: Point<f64>,
}

impl Kitchen {
    pub fn new(capacity: usize, location: Point<f64>) -> Self {
        Self {
            id: ResourceId::new(),
            capacity,
            current_load: 0,
            location,
        }
    }

    pub fn start_order(&mut self) -> bool {
        if self.current_load < self.capacity {
            self.current_load += 1;
            true
        } else {
            false
        }
    }

    pub fn complete_order(&mut self) {
        self.current_load = self.current_load.saturating_sub(1);
    }
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

### Example 2: Sorting Station (Multi-Lane)

```rust
#[derive(Debug)]
pub struct SortingStation {
    id: ResourceId,
    lanes: Vec<SortingLane>,
}

#[derive(Debug)]
struct SortingLane {
    lane_id: usize,
    capacity: usize,
    current_packages: usize,
}

impl SortingStation {
    pub fn new(num_lanes: usize, lane_capacity: usize) -> Self {
        let lanes = (0..num_lanes)
            .map(|i| SortingLane {
                lane_id: i,
                capacity: lane_capacity,
                current_packages: 0,
            })
            .collect();

        Self {
            id: ResourceId::new(),
            lanes,
        }
    }

    pub fn total_capacity(&self) -> usize {
        self.lanes.iter().map(|l| l.capacity).sum()
    }

    pub fn total_load(&self) -> usize {
        self.lanes.iter().map(|l| l.current_packages).sum()
    }

    pub fn assign_to_lane(&mut self, package_id: &str) -> Option<usize> {
        self.lanes
            .iter_mut()
            .find(|lane| lane.current_packages < lane.capacity)
            .map(|lane| {
                lane.current_packages += 1;
                lane.lane_id
            })
    }
}

impl Resource for SortingStation {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "sorting_station"
    }

    fn capacity(&self) -> usize {
        self.total_capacity()
    }

    fn current_load(&self) -> usize {
        self.total_load()
    }
}
```

## Task Processing Patterns

### Pattern 1: Demand to Task Conversion

Convert agent demands into tasks:

```rust
impl SimulationHooks for MyHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "food_order" {
                    let task = Box::new(FoodOrder {
                        id: TaskId::new(),
                        customer_id: demand.agent_id.unwrap(),
                        items: vec!["burger".to_string()],
                        created_at: ctx.current_time(),
                    });

                    self.allocator.add_task(task);
                }
            }
        }

        // Allocate tasks to resources
        let assignments = self.allocator.assign_tasks(&LeastLoaded)?;
        println!("Assigned {} tasks", assignments.len());

        Ok(())
    }
}
```

### Pattern 2: Task Completion

Handle task completion and update resource load:

```rust
pub fn complete_task(&mut self, task_id: TaskId, resource_id: ResourceId) -> Result<()> {
    // Remove task
    self.allocator.remove_task(task_id)?;

    // Update resource load
    if let Some(resource) = self.resources.get_mut(&resource_id) {
        resource.current_load -= 1;
    }

    // Emit completion event
    self.dispatcher.emit(Event::new(Box::new(TaskCompletedEvent {
        task_id,
        resource_id,
        timestamp: Utc::now(),
    }), ctx.current_time()));

    Ok(())
}
```

### Pattern 3: Priority Queue

Use task priority for ordering:

```rust
pub struct PriorityStrategy;

impl AllocationStrategy for PriorityStrategy {
    fn assign(&self, task: &dyn Task, resources: &[&dyn Resource]) -> Option<ResourceId> {
        // Sort resources by (available capacity, priority handling capability)
        resources
            .iter()
            .filter(|r| r.current_load() < r.capacity())
            .max_by_key(|r| r.available_capacity())
            .map(|r| r.id())
    }
}

// Tasks sorted by priority before allocation
let mut tasks = vec![task1, task2, task3];
tasks.sort_by_key(|t| -t.priority());  // Descending priority

for task in tasks {
    allocator.add_task(task);
}
```

## Examples from Reference Implementations

### sim-kitchen: Stove Resource

Multi-burner cooking station with capacity management:

```rust
pub struct Stove {
    id: ResourceId,
    burners: usize,
    active_orders: HashSet<TaskId>,
}

impl Resource for Stove {
    fn capacity(&self) -> usize {
        self.burners
    }

    fn current_load(&self) -> usize {
        self.active_orders.len()
    }
}
```

**File:** `crates/sim-kitchen/src/resources/stove.rs` (conceptual)

### sim-logistics: LoadingDock Resource

Bay-based loading station:

```rust
pub struct LoadingDock {
    id: ResourceId,
    bays: Vec<LoadingBay>,
}

struct LoadingBay {
    bay_number: usize,
    occupied: bool,
    current_truck: Option<AgentId>,
}

impl Resource for LoadingDock {
    fn capacity(&self) -> usize {
        self.bays.len()
    }

    fn current_load(&self) -> usize {
        self.bays.iter().filter(|b| b.occupied).count()
    }
}
```

**File:** `crates/sim-logistics/src/resources/loading_dock.rs`

## Performance Considerations

### Arrow Storage

Resources and tasks use Arrow RecordBatch:

- **Bulk operations**: Faster than HashMap iteration
- **SQL queries**: Cross-entity analytics with DataFusion
- **Memory efficiency**: Columnar storage reduces overhead

### Allocation Efficiency

Optimize allocation strategies:

```rust
// ✅ Good - filter once, iterate efficiently
let available: Vec<_> = resources
    .iter()
    .filter(|r| r.current_load() < r.capacity())
    .collect();

let best = available
    .iter()
    .min_by_key(|r| r.current_load())
    .map(|r| r.id());

// ❌ Bad - multiple passes, repeated filtering
for resource in resources {
    if resource.current_load() < resource.capacity() {
        if is_best(resource) {
            return Some(resource.id());
        }
    }
}
```

## Common Pitfalls

### Pitfall 1: Not Updating Load

```rust
// ❌ Wrong - load never decreases
impl Resource for Kitchen {
    fn current_load(&self) -> usize {
        self.current_orders.len()  // Never cleared!
    }
}

// ✅ Correct - explicitly manage load
pub fn complete_order(&mut self, order_id: TaskId) {
    self.current_orders.remove(&order_id);
}
```

### Pitfall 2: Capacity Overflow

```rust
// ❌ Wrong - can exceed capacity
pub fn start_order(&mut self) {
    self.current_load += 1;  // Unchecked!
}

// ✅ Correct - check capacity first
pub fn start_order(&mut self) -> bool {
    if self.current_load < self.capacity {
        self.current_load += 1;
        true
    } else {
        false
    }
}
```

### Pitfall 3: Mismatched Resource Types

```rust
// ❌ Wrong - type mismatch
let task = FoodOrder { /* ... */ };
task.required_resource_type();  // Returns "kitchen"

let barista = Barista { /* ... */ };
barista.resource_type();  // Returns "barista"

// Won't match! Task will never be assigned

// ✅ Correct - matching types
let task = FoodOrder { /* ... */ };
assert_eq!(task.required_resource_type(), "kitchen");

let kitchen = Kitchen { /* ... */ };
assert_eq!(kitchen.resource_type(), "kitchen");
```

## Testing Resources

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_capacity() {
        let mut kitchen = Kitchen::new(3, Point::new(0.0, 0.0));

        assert_eq!(kitchen.capacity(), 3);
        assert_eq!(kitchen.current_load(), 0);
        assert_eq!(kitchen.available_capacity(), 3);

        // Start orders
        assert!(kitchen.start_order());
        assert_eq!(kitchen.current_load(), 1);

        assert!(kitchen.start_order());
        assert_eq!(kitchen.current_load(), 2);

        assert!(kitchen.start_order());
        assert_eq!(kitchen.current_load(), 3);

        // Capacity reached
        assert!(!kitchen.start_order());
        assert_eq!(kitchen.current_load(), 3);

        // Complete order
        kitchen.complete_order();
        assert_eq!(kitchen.current_load(), 2);

        // Can start again
        assert!(kitchen.start_order());
        assert_eq!(kitchen.current_load(), 3);
    }

    #[test]
    fn test_allocation_strategy() {
        let resources = vec![
            Box::new(Kitchen::new(3, Point::new(0.0, 0.0))) as Box<dyn Resource>,
            Box::new(Kitchen::new(3, Point::new(1.0, 1.0))),
        ];

        let task = Box::new(FoodOrder::new(AgentId::new()));

        let strategy = LeastLoaded;
        let assigned = strategy.assign(task.as_ref(), &resources);

        assert!(assigned.is_some());
    }
}
```

## Related Documentation

- [Agents](agents.md) - Agent behavior and demands
- [Workflows](workflows.md) - Multi-step task processing
- [Snapshots](snapshots.md) - Resource state persistence

## Summary

Resources and tasks form the work processing backbone:

- Resources have capacity and manage load
- Tasks specify required resource type and duration
- ResourceAllocator matches tasks to resources with strategies
- Use Arrow storage for performance
- Follow examples from sim-kitchen and sim-logistics
- Test capacity management and allocation logic

---

**Last Updated:** March 16, 2026
