# Tributary Documentation

**Last Updated:** March 16, 2026

Welcome to the Tributary documentation. Tributary is a Rust-based simulation and geospatial processing framework designed for building high-performance, agent-based simulations with first-class support for Apache Arrow, Delta Lake, and Python integration.

## Overview

Tributary provides a complete toolkit for building domain-agnostic simulations with:

- **Agent-Based Modeling** - Flexible agent behaviors with tick-based updates
- **Resource Allocation** - Capacity-constrained processing with pluggable strategies
- **Workflow Orchestration** - Multi-stage business processes with state persistence
- **Geospatial Operations** - Location tracking, H3 indexing, and road network routing
- **Event Infrastructure** - CloudEvents 1.0 compliant event emission and storage
- **Observability** - OTLP-compliant telemetry generation (logs, traces, metrics)
- **State Persistence** - Arrow RecordBatch storage with snapshot/restore
- **Demand Generation** - Probabilistic demand patterns with Poisson, TimeOfDay, and Spatial generators

## Key Features

### Performance

- **10-100x** faster bulk operations with Arrow columnar storage
- **<1ms** average tick time for 1,000+ agents
- **7µs** routing queries with road network graphs
- **<1%** telemetry overhead on simulation runtime
- **9x** compression with Parquet snapshots

### Extensibility

- Domain-agnostic core framework
- Trait-based plugin architecture
- Two validated domains: kitchen operations and logistics
- Zero framework modifications needed for new domains

### Production Ready

- **649+ tests** passing across all crates
- Zero clippy warnings on library code
- Comprehensive error handling
- Type-safe identifiers (UUID v7)
- Full API documentation

## Quick Navigation

### Getting Started

- **[Getting Started Guide](GETTING_STARTED.md)** - Step-by-step tutorial for new users
- **[Architecture](ARCHITECTURE.md)** - Core design principles and patterns
- **[Examples](../crates/sim-kitchen/examples/)** - Working code examples

### Core Concepts

Learn about the framework's key abstractions:

- **[Agents](concepts/agents.md)** - Agent trait, lifecycle, and state management
- **[Resources](concepts/resources.md)** - Resource allocation and task processing
- **[Workflows](concepts/workflows.md)** - Multi-step process orchestration
- **[Demand Generation](concepts/demand-generation.md)** - Probabilistic demand patterns
- **[Events](concepts/events.md)** - CloudEvents infrastructure and emission
- **[Telemetry](concepts/telemetry.md)** - Observability and OTLP export
- **[Snapshots](concepts/snapshots.md)** - State persistence and restore
- **[Spatial Operations](concepts/spatial.md)** - Geospatial indexing and routing

### Crate Documentation

- **[sim-core](../crates/sim-core/README.md)** - Core simulation engine
- **[sim-geo](../crates/sim-geo/README.md)** - Geospatial operations
- **[sim-events](../crates/sim-events/README.md)** - Event handling
- **[sim-telemetry](../crates/sim-telemetry/README.md)** - Observability
- **[sim-kitchen](../crates/sim-kitchen/README.md)** - Ghost kitchen domain (reference)
- **[sim-logistics](../crates/sim-logistics/README.md)** - Package delivery domain

### Migration & Status

- **[Migration Status](../migration/README.md)** - Project progress and roadmap
- **[Handover](../migration/HANDOVER.md)** - Latest completion status
- **[Completed Phases](../migration/completed/)** - Historical implementation details

## Crate Overview

### Core Framework

**sim-core** - The simulation engine providing:
- Population management with agent lifecycle
- Resource allocation with capacity constraints
- Workflow engine for multi-step processes
- Simulation runner with lifecycle hooks
- Time management and configuration
- Type-safe identifiers (AgentId, ResourceId, etc.)

**sim-geo** - Geospatial capabilities including:
- Location management with H3 spatial indexing
- Journey tracking with velocity models
- Road network routing with fast_paths
- Movement handlers for agent mobility
- Nearest-neighbor and radius queries

**sim-events** - Event infrastructure with:
- CloudEvents 1.0 specification compliance
- Event dispatcher with buffering
- Parquet and JSON Lines export
- Arrow RecordBatch storage
- Type-safe event payloads

**sim-telemetry** - Observability tooling for:
- Log generation with severity levels
- Trace generation with span relationships
- Metric generation (counters, gauges, histograms)
- OTLP export for OpenTelemetry
- JSON and Prometheus formats

### Domain Implementations

**sim-kitchen** - Ghost kitchen simulation demonstrating:
- Customer agents with order behavior
- Kitchen resources with capacity
- Multi-workflow coordination (food prep, delivery, supply chain)
- Inventory management with auto-replenishment
- 122 tests validating all patterns

**sim-logistics** - Package delivery simulation showing:
- Package and Driver agents
- SortingStation and LoadingDock resources
- PackageShipmentWorkflow with 5 steps
- H3-based warehouse clustering
- 77 tests with ZERO framework changes (proves generality)

## Framework Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      SimulationRunner                       │
│  (Orchestrates tick loop, lifecycle hooks, time management) │
└──────┬────────────────────────────────────────────┬─────────┘
       │                                            │
       ├─── PopulationHandler ──────────────────────┤
       │    (Manages agents, tick execution)        │
       │                                            │
       ├─── ResourceAllocator ──────────────────────┤
       │    (Matches tasks to resources)            │
       │                                            │
       ├─── WorkflowEngine ─────────────────────────┤
       │    (Multi-step process orchestration)      │
       │                                            │
       ├─── LocationHandler (sim-geo) ─────────────┤
       │    (H3 spatial indexing, queries)          │
       │                                            │
       ├─── MovementHandler (sim-geo) ─────────────┤
       │    (Journey tracking, routing)             │
       │                                            │
       ├─── EventDispatcher (sim-events) ──────────┤
       │    (CloudEvents emission, buffering)       │
       │                                            │
       └─── TelemetryCollector (sim-telemetry) ────┘
            (Log/trace/metric generation)
```

## Data Flow

### Simulation Tick

```
1. Runner.tick()
   ├─> Generate demands (DemandGenerator)
   ├─> Tick all agents (PopulationHandler)
   │   └─> Collect AgentTickResult (demands, journeys, state changes)
   ├─> Allocate resources (ResourceAllocator)
   ├─> Advance workflows (WorkflowEngine)
   ├─> Process journeys (MovementHandler)
   ├─> Emit events (EventDispatcher)
   └─> Generate telemetry (TelemetryCollector)
```

### State Storage

All state is stored in Apache Arrow RecordBatch format:
- Agents: id, agent_type, status, position_x, position_y, metadata
- Resources: id, resource_type, capacity, current_load, metadata
- Tasks: id, required_resource_type, estimated_duration, metadata
- Workflows: instance_id, workflow_type, current_step, status
- Locations: id, location_type, position_x, position_y, h3_cell

Benefits:
- 10-100x faster bulk operations
- SQL queries with DataFusion
- Efficient Parquet snapshots (9x compression)
- Arrow-compatible analytics

## Success Stories

### Kitchen Operations (sim-kitchen)

- **24-hour simulation** with 2,000+ orders
- **3 concurrent workflows** (food prep, delivery, supply chain)
- **Inventory management** with auto-replenishment
- **0.06ms average tick** (1,667x better than 100ms target)
- **122 tests** validating all patterns

### Logistics (sim-logistics)

- **24-hour simulation** with 2,000+ packages
- **99.4% delivery success** rate
- **ZERO framework modifications** needed
- **0.31ms average tick** time
- **77 tests** all passing

## Common Use Cases

### 1. Restaurant/Kitchen Operations
- Order management and fulfillment
- Kitchen capacity planning
- Delivery route optimization
- Inventory replenishment

### 2. Logistics & Warehousing
- Package routing and delivery
- Warehouse operations
- Driver scheduling
- Multi-leg shipments

### 3. Transportation Networks
- Route planning with road networks
- Vehicle movement simulation
- Demand forecasting
- Traffic modeling

### 4. Healthcare Systems
- Patient flow through facilities
- Resource allocation (beds, equipment)
- Staff scheduling
- Emergency response

### 5. Manufacturing
- Assembly line simulation
- Quality control processes
- Inventory management
- Production scheduling

## Performance Targets

| Component | Target | Actual | Status |
|-----------|--------|--------|--------|
| Tick time (1k agents) | <100ms | <1ms | ✅ 100x better |
| Routing query | <1ms | 7µs | ✅ 143x better |
| Telemetry overhead | <5% | <1% | ✅ 5x better |
| Snapshot save (10k agents) | <100ms | 15ms | ✅ 6.7x better |
| Snapshot restore (10k agents) | <100ms | 7ms | ✅ 14x better |

## Development Status

**Overall Progress:** 78% complete (12/14 phases)

**Completed Phases:**
- Phase 1: Critical bug fixes
- Phase 3: Code simplification
- Phase 5: Workflow integration
- Phase 6: Arrow migration
- Phase 7: Road network routing
- Phase 8: Telemetry generation
- Phase 9: Multi-workflow coordination
- Phase 10: Demand generation
- Phase 11: Logistics domain validation

**Remaining Work:**
- Phase 12-14: Python bindings, CLI, benchmarks (3 weeks)

See [Migration Status](../migration/README.md) for detailed progress.

## Getting Help

### Documentation
- **Concepts** - Read the concept guides in `docs/concepts/`
- **Examples** - Check working examples in each crate
- **API Docs** - Run `cargo doc --open -p sim-core`

### Testing
- **Unit Tests** - `cargo test --lib -p sim-core`
- **Integration Tests** - `cargo test -p sim-kitchen`
- **All Tests** - `cargo test --workspace`

### Code Quality
- **Linting** - `cargo clippy --lib --workspace`
- **Formatting** - `cargo fmt --check`
- **Coverage** - 649+ tests across all crates

## Contributing

This is a research/demonstration project. For production use:

1. Review the [Architecture](ARCHITECTURE.md) document
2. Study the domain examples (sim-kitchen, sim-logistics)
3. Use the trait-based APIs to build your domain
4. Follow the patterns in existing implementations

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Next Steps

- **New Users**: Start with [Getting Started](GETTING_STARTED.md)
- **Developers**: Read [Architecture](ARCHITECTURE.md) and explore [sim-kitchen](../crates/sim-kitchen/)
- **Researchers**: Check [completed phases](../migration/completed/) for implementation details
