# sim-logistics

Package delivery simulation demonstrating framework generality.

## Overview

`sim-logistics` validates that the simulation framework (sim-core, sim-geo, sim-events) is truly domain-agnostic by implementing a complete logistics domain with **ZERO** framework modifications.

This crate proves that:
- Generic abstractions work across domains
- Allocation strategies are reusable
- Event patterns transfer seamlessly
- Geospatial components are domain-agnostic

## Domain Model

### Agents
- **Package**: Passive agent tracking package lifecycle
- **Driver**: Active agent handling pickup and delivery

### Resources
- **SortingStation**: Package sorting with capacity constraints
- **LoadingDock**: Vehicle loading operations

### Workflows
- **PackageShipmentWorkflow**: 5-step delivery pipeline
  1. Received (at warehouse)
  2. Sorted (by sorting station)
  3. Loaded (onto vehicle)
  4. InTransit (with driver)
  5. Delivered (at destination)

### Locations
- **Warehouse**: Package processing centers with H3 indexing
- **DeliveryAddress**: Final destinations with spatial optimization

## Quick Start

### Basic Example

```rust
use sim_logistics::LogisticsSimulation;

let mut sim = LogisticsSimulation::new();
sim.run_for_hours(24)?;

let metrics = sim.get_metrics();
println!("Packages delivered: {}", metrics.packages_delivered);
println!("Driver utilization: {:.1}%", metrics.driver_utilization);
```

### Priority Packages

```rust
use sim_logistics::{Package, PackagePriority};

let express = Package::new(
    origin,
    destination,
    PackagePriority::Express,
);

let overnight = Package::new(
    origin,
    destination,
    PackagePriority::Overnight,
);
```

### Multi-Warehouse Network

```rust
use sim_logistics::Warehouse;

let mut sim = LogisticsSimulation::new();

for location in warehouse_locations {
    sim.add_warehouse(Warehouse {
        id: LocationId::new(),
        position: location,
        capacity: 1000,
    })?;
}

sim.run_for_steps(1000)?;
```

## Events (CloudEvents 1.0)

All state transitions emit CloudEvents:

- `io.logistics.package.received` - Package arrives at warehouse
- `io.logistics.package.sorted` - Package sorted for loading
- `io.logistics.package.loaded` - Package loaded onto vehicle
- `io.logistics.package.in_transit` - Package en route to destination
- `io.logistics.package.delivered` - Package delivered successfully

## Framework Validation

This implementation demonstrates:

### No Framework Modifications
- sim-core: **Unchanged**
- sim-geo: **Unchanged**
- sim-events: **Unchanged**

### All Abstractions Work
- `Agent` trait: Package and Driver
- `Resource` trait: SortingStation and LoadingDock
- `Task` trait: ShipmentTask
- `Workflow` trait: PackageShipmentWorkflow

### Reusable Strategies
- LeastLoaded allocation strategy
- RoundRobin distribution
- Standard event patterns

### Geospatial Integration
- LocationHandler for warehouses
- MovementHandler for driver routes
- H3 indexing for delivery zones

## Comparison with sim-kitchen

| Aspect | sim-kitchen | sim-logistics | Framework Changes |
|--------|-------------|---------------|-------------------|
| Agents | Customer, Courier | Package, Driver | **None** |
| Resources | Kitchen | SortingStation, LoadingDock | **None** |
| Tasks | Order | ShipmentTask | **None** |
| Workflows | Supply chain | Package shipment | **None** |
| Events | 5 types | 5 types | **None** |
| Locations | Distribution sites | Warehouses, Addresses | **None** |

**Result**: Same framework, different domain, no modifications needed.

## Testing

```bash
cargo test -p sim-logistics
```

All tests validate:
- Package state transitions
- Resource capacity management
- Workflow progression
- Event emission
- Geospatial operations

## Dependencies

- `sim-core 0.0.1` - Unchanged simulation framework
- `sim-geo 0.0.1` - Unchanged geospatial components
- `sim-events 0.0.1` - Unchanged event infrastructure
- `chrono 0.4` - Time handling
- `geo 0.31` - Geospatial types
- `h3o 0.9` - H3 indexing
- `rand 0.9` - Random generation
- `serde_json 1.0` - JSON serialization

## Version Information

Current version: `0.0.1`

See [CHANGELOG.md](CHANGELOG.md) for detailed version history.

## Documentation

- [Architecture](docs/architecture.md) - Framework generality validation, design principles
- [Patterns](docs/patterns.md) - Common usage patterns and best practices
- [Troubleshooting](docs/troubleshooting.md) - Common issues, debugging tips, and FAQ

## Related Crates

- `sim-core` - Core simulation framework (unchanged)
- `sim-geo` - Geospatial components (unchanged)
- `sim-events` - CloudEvents infrastructure (unchanged)
- `sim-kitchen` - Reference implementation (ghost kitchen)

## License

MIT OR Apache-2.0
