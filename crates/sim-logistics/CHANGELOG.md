# Changelog

All notable changes to `sim-logistics` will be documented in this file.

## [0.0.1] - 2026-03-16

### Added

#### Framework Validation Implementation
Complete package delivery simulation built with **ZERO** framework modifications.

#### Domain Model
- **Package agent**: Passive tracking of package lifecycle
- **Driver agent**: Active pickup and delivery
- **SortingStation resource**: Package sorting with capacity
- **LoadingDock resource**: Vehicle loading operations
- **PackageShipmentWorkflow**: 5-step delivery pipeline

#### Agents
- Package with priority levels (Standard, Express, Overnight)
- Driver with vehicle capacity and route optimization

#### Resources
- SortingStation with throughput limits
- LoadingDock with vehicle queuing
- VehicleLoadingState tracking

#### Workflows
- PackageShipmentWorkflow: Received -> Sorted -> Loaded -> InTransit -> Delivered
- ShipmentTask and ShipmentTaskFactory
- Multi-stage progression with resource dependencies

#### Locations
- Warehouse with package processing
- DeliveryAddress with H3 spatial indexing
- Route optimization via sim-geo integration

#### Events (CloudEvents 1.0)
- `io.logistics.package.received`
- `io.logistics.package.sorted`
- `io.logistics.package.loaded`
- `io.logistics.package.in_transit`
- `io.logistics.package.delivered`

#### Framework Generality Validation
Demonstrates that sim-core, sim-geo, and sim-events are truly domain-agnostic:
- No special-casing for logistics
- No framework modifications needed
- All abstractions work out-of-box
- Reusable allocation strategies
- Standard event patterns

### Dependencies
- `sim-core 0.0.1` - Unchanged simulation framework
- `sim-geo 0.0.1` - Unchanged geospatial components
- `sim-events 0.0.1` - Unchanged event infrastructure
- `chrono 0.4` - Time handling
- `geo 0.31` - Geospatial types
- `rand 0.9` - Random generation

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
