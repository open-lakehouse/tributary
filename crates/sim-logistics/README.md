# sim-logistics

Package delivery simulation: the second domain on the `sim-*` framework. It
exercises agents, resources, workflows and demand generation from a domain
that shares nothing with the kitchen.

## Quick start

```rust
use sim_logistics::{LogisticsSimulation, PackagePriority};

let mut sim = LogisticsSimulation::new_seeded(Some(42));
sim.create_package(0, 17, 2.5, PackagePriority::Express)?; // optional extra packages
sim.run_for_hours(24)?;

let m = sim.get_metrics();
println!("{} of {} packages delivered, drivers {:.0}% busy",
    m.packages_delivered, m.packages_created, m.driver_utilization_percent);
sim.save_events("output/logistics-events.parquet")?;
```

From the CLI: `caspers run --no-tui --domain logistics --duration 2880 --seed 42`
(then `caspers validate --domain logistics --events output/events.parquet`).
From Python: `caspers.run_logistics_simulation(steps=2880)`.

## Domain model

| Framework concept | Logistics type |
|---|---|
| Agent | `Package`, passive: `created → received → sorted → in_transit → delivered` |
| Resource | `Station`: receivers, sorting stations, loading docks, drivers (capacity 1), confirmers |
| Workflow | `PackageShipmentWorkflow`: receive → sort (5 min) → load (6 min) → transit (30 min) → confirm (1 min) |
| Task | `ShipmentTask` from `ShipmentTaskFactory` (carries the package priority) |
| Demand | Poisson, 10 packages per hour |

The default layout has two warehouses, 1000 delivery addresses in the Bay
Area, two sorting stations, two docks and 20 drivers. Time advances 30 s per
step. Overnight and express packages are allocated before standard ones at
every station. With a seed the address layout, demand and package routing are
reproducible, and so is the event stream.

## Events

| Type | When | Data |
|---|---|---|
| `io.logistics.package.received` | receive step done | `package_id`, `warehouse_id`, destination |
| `io.logistics.package.sorted` | sort step done | `package_id`, `sorting_station_id`, `warehouse_id` |
| `io.logistics.package.loaded` | driver allocated | `package_id`, `driver_id`, `loading_dock_id` |
| `io.logistics.package.in_transit` | driver allocated | `package_id`, `driver_id`, `estimated_arrival` |
| `io.logistics.package.delivered` | confirm step done | `package_id`, `driver_id`, `delivery_time`, destination |

All events use the tracking number (`PKG-000001`, …) as `package_id` and
CloudEvents subject.

## Metrics

`get_metrics()` reports packages created and delivered, distinct drivers
used, mean creation-to-delivery time, and mean driver utilization (share of
drivers carrying a package, averaged over steps).
