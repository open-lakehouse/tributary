# sim-logistics Usage Patterns

## Pattern 1: Basic Package Simulation

```rust
use sim_logistics::LogisticsSimulation;

let mut sim = LogisticsSimulation::new();
sim.run_for_hours(24)?;

let metrics = sim.get_metrics();
println!("Packages delivered: {}", metrics.packages_delivered);
println!("Driver utilization: {:.1}%", metrics.driver_utilization);
```

## Pattern 2: Priority Package Handling

```rust
use sim_logistics::{Package, PackagePriority};

let express_package = Package::new(
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

## Pattern 3: Multi-Warehouse Network

```rust
use sim_logistics::{Warehouse, LogisticsSimulation};

let mut sim = LogisticsSimulation::new();

// Add multiple warehouses
for location in warehouse_locations {
    sim.add_warehouse(Warehouse {
        id: LocationId::new(),
        position: location,
        capacity: 1000,
    })?;
}
```

## Pattern 4: Route Optimization with sim-geo

```rust
use sim_geo::RoadNetwork;

let network = RoadNetwork::from_osm_pbf("city.osm.pbf")?;
let prepared = network.prepare_routing()?;

// Use in driver routing
for package in packages {
    let route = prepared.route(warehouse_node, delivery_node)?;
    driver.assign_route(route);
}
```

## Best Practices

1. **Use priority levels** for realistic delivery constraints
2. **Model capacity limits** at sorting stations and docks
3. **Integrate real road networks** for accurate routing
4. **Track driver utilization** for resource optimization
