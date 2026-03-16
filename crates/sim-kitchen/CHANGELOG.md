# Changelog

All notable changes to `sim-kitchen` will be documented in this file.

## [0.0.1] - 2026-03-16

### Added

#### Reference Implementation
Complete ghost kitchen simulation demonstrating framework usage.

#### Domain Model
- **Customer agent**: State machine (Idle -> Ordering -> WaitingForFood -> Eating -> Done)
- **Kitchen resource**: Capacity-constrained food preparation
- **Order task**: Food orders with item lists and prep time
- **CloudEvents**: 5 event types for all state transitions

#### Agents
- Customer with probabilistic ordering behavior
- Courier for delivery workflows
- SupplyDriver for supply chain logistics

#### Resources
- Kitchen with capacity tracking
- Inventory system with stock management
- InventoryManager for multi-inventory coordination

#### Workflows
- Supply chain workflow (supplier -> distribution -> kitchen)
- Courier delivery workflow (pickup -> transport -> delivery)
- Multi-stage food preparation

#### Locations
- Distribution sites with H3 indexing
- Order fulfillment tracking
- Spatial demand patterns

#### Events (CloudEvents 1.0)
- `io.kitchen.order.created`
- `io.kitchen.order.assigned`
- `io.kitchen.order.completed`
- `io.kitchen.customer.eating`
- `io.kitchen.customer.done`

#### Multi-Workflow Patterns
Demonstrates advanced framework capabilities:
- Concurrent workflow execution
- Inter-workflow dependencies
- Resource sharing across workflows
- Complex state coordination

### Dependencies
- `sim-core 0.0.1` - Simulation framework
- `sim-geo 0.0.1` - Geospatial components
- `sim-events 0.0.1` - Event infrastructure
- `chrono 0.4` - Time handling
- `geo 0.31` - Geospatial types
- `rand 0.9` - Random generation

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
