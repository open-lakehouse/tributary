# sim-kitchen Usage Patterns

## Pattern 1: Run Basic Simulation

```rust
use sim_kitchen::{build_simulation, add_customers};

let mut runner = build_simulation(3, 5, true);  // 3 kitchens, 5 capacity each
add_customers(&mut runner, 20);  // Add 20 customers
runner.run_for_steps(100)?;
```

## Pattern 2: Custom Customer Behavior

```rust
use sim_kitchen::Customer;

let customer = Customer::new_with_config(
    0.3,  // 30% order probability per tick
    Duration::minutes(15),  // Eating duration
);
```

## Pattern 3: Multi-Workflow Simulation

```rust
// Demonstrates concurrent supply chain + delivery workflows
let mut sim = KitchenSimulation::with_workflows(
    num_kitchens,
    kitchen_capacity,
    enable_supply_chain,
    enable_delivery,
);
```

## Pattern 4: Geospatial Demand

```rust
use sim_kitchen::locations::DistributionSite;
use sim_geo::LocationHandler;

let mut locations = LocationHandler::new(Resolution::Nine);
locations.add_location(DistributionSite {
    id: LocationId::new(),
    position: Point::new(-122.4194, 37.7749),
    inventory: 1000,
})?;
```

## Best Practices

1. **Start simple**: Begin with basic customer + kitchen
2. **Add workflows incrementally**: Supply chain, then delivery
3. **Use events for observability**: Track all state transitions
4. **Profile performance**: Ensure <100ms per 100 steps
