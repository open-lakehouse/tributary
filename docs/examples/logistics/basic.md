# Basic Logistics Example

Basic package shipment simulation with sorting and loading.

## Overview

- Package agents moving through system
- Sorting station and loading dock resources
- Simple workflow: Receive → Sort → Load → Transit → Deliver

## Code Location

`/Users/robert.pack/code/management/crates/sim-logistics/tests/integration_test.rs`

## Domain Model

```rust
// Package agent
struct Package {
    id: AgentId,
    state: PackageState,
    origin: Point<f64>,
    destination: Point<f64>,
}

enum PackageState {
    Received,
    InSorting,
    ReadyForLoading,
    InTransit,
    Delivered,
}

// Sorting station resource
struct SortingStation {
    id: ResourceId,
    capacity: usize,
}

// Loading dock resource
struct LoadingDock {
    id: ResourceId,
    capacity: usize,
}
```

## Example

```rust
fn main() -> Result<()> {
    let mut sim = LogisticsSimulation::new();

    // Add package
    sim.add_package(Package {
        id: AgentId::new(),
        state: PackageState::Received,
        origin: Point::new(-122.4194, 37.7749),
        destination: Point::new(-122.3959, 37.7937),
    });

    // Run (24 hours at 30 sec/step)
    sim.run_for_steps(2880)?;

    let metrics = sim.get_metrics();
    println!("Packages delivered: {}", metrics.packages_delivered);

    Ok(())
}
```

## See Also

- [package-routing.md](./package-routing.md) - Road network routing
- [warehouse-clustering.md](./warehouse-clustering.md) - H3 spatial clustering
