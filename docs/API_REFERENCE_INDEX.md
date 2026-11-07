# API Reference and Examples Index

Complete API reference documentation and practical examples for all framework crates.

## API Reference

Comprehensive API documentation for each crate:

### Core Framework
- **[sim-core](./api-reference/sim-core.md)** - Core simulation abstractions
  - Agent, Resource, Task, Workflow traits
  - PopulationHandler, ResourceAllocator, WorkflowEngine
  - SimulationRunner and hooks
  - Demand generators
  - Snapshot management
  - SQL querying

- **[sim-geo](./api-reference/sim-geo.md)** - Geospatial capabilities
  - LocationHandler and Location types
  - MovementHandler and Journey tracking
  - RoadNetwork and routing
  - H3 spatial indexing
  - Velocity models

- **[sim-events](./api-reference/sim-events.md)** - CloudEvents infrastructure
  - Event and EventPayload types
  - EventDispatcher
  - Parquet storage
  - Common event patterns

- **[sim-telemetry](./api-reference/sim-telemetry.md)** - Observability
  - TelemetryGenerator trait
  - Log, trace, metric generators
  - TelemetryCollector
  - Format exporters (JSON, OTLP, Prometheus)

### Integration
- **[integration.md](./api-reference/integration.md)** - Cross-crate patterns
  - SimulationHooks pattern
  - TaskFactory pattern
  - Handler coordination
  - Complete simulation template

## Examples

### Kitchen Domain (Ghost Kitchen)

**Basic Examples:**
- **[basic.md](./examples/kitchen/basic.md)** - Minimal working example
  - Customer agents
  - Kitchen resources
  - Order tasks
  - Event emission
  - Expected output: ~47 orders processed in 100 steps

**Advanced Examples:**
- **[multi-workflow.md](./examples/kitchen/multi-workflow.md)** - Phase 9 multi-workflow orchestration
  - FoodOrderWorkflow
  - SupplyChainWorkflow
  - CourierDeliveryWorkflow
  - WorkflowRegistry usage
  - Workflow coordination

- **[inventory-management.md](./examples/kitchen/inventory-management.md)** - Auto-replenishment
  - InventoryManager API
  - Low-stock triggers
  - Supply chain integration

**Code Locations:**
- Full implementation: `crates/sim-kitchen/`
- Basic example: `crates/sim-kitchen/examples/basic.rs`
- Multi-workflow test: `crates/sim-kitchen/tests/multi_workflow_e2e_test.rs`

### Logistics Domain (Package Delivery)

**Basic Examples:**
- **[basic.md](./examples/logistics/basic.md)** - Simple package flow
  - Package agents
  - Sorting stations and loading docks
  - Basic workflow

**Advanced Examples:**
- **[package-routing.md](./examples/logistics/package-routing.md)** - Road network routing
  - RoadNetwork setup
  - Multi-stop route planning
  - Driver agent integration

- **[warehouse-clustering.md](./examples/logistics/warehouse-clustering.md)** - H3 spatial clustering
  - H3 indexing at multiple resolutions
  - Nearest warehouse assignment
  - Geographic coverage analysis

**Code Locations:**
- Full implementation: `crates/sim-logistics/`
- Integration test: `crates/sim-logistics/tests/integration_test.rs`

### Custom Implementation Templates

Well-commented boilerplate for new implementations:

- **[agent-template.rs](./examples/custom/agent-template.rs)** - Agent implementation
  - Status enum
  - Agent struct
  - Complete trait implementation
  - Arrow serialization
  - Unit tests

- **[resource-template.rs](./examples/custom/resource-template.rs)** - Resource implementation
  - Resource struct
  - Task definition
  - Custom allocation strategy
  - Integration example
  - Unit tests

- **[workflow-template.rs](./examples/custom/workflow-template.rs)** - Workflow implementation
  - Workflow struct
  - Builder pattern
  - TaskFactory implementation
  - Event handling
  - Unit tests

## Quick Start Guide

### 1. Understanding the Framework

Start with the API reference to understand core concepts:
1. Read [sim-core.md](./api-reference/sim-core.md) - Core abstractions
2. Review [integration.md](./api-reference/integration.md) - How components work together

### 2. Run a Working Example

Choose a domain example:
- **Kitchen**: `cd crates/sim-kitchen && cargo run --example basic`
- **Logistics**: `cd crates/sim-logistics && cargo test integration_test`

### 3. Build Your Own Simulation

Use the templates:
1. Copy [agent-template.rs](./examples/custom/agent-template.rs)
2. Copy [resource-template.rs](./examples/custom/resource-template.rs)
3. Follow [integration.md](./api-reference/integration.md) for setup

## Documentation Structure

```
docs/
├── api-reference/          # Complete API documentation
│   ├── sim-core.md         # Core framework
│   ├── sim-geo.md          # Geospatial
│   ├── sim-events.md       # Events
│   ├── sim-telemetry.md    # Telemetry
│   └── integration.md      # Cross-crate patterns
│
├── examples/               # Practical examples
│   ├── kitchen/            # Ghost kitchen domain
│   │   ├── basic.md
│   │   ├── multi-workflow.md
│   │   └── inventory-management.md
│   │
│   ├── logistics/          # Package delivery domain
│   │   ├── basic.md
│   │   ├── package-routing.md
│   │   └── warehouse-clustering.md
│   │
│   └── custom/             # Templates for new implementations
│       ├── agent-template.rs
│       ├── resource-template.rs
│       └── workflow-template.rs
│
└── API_REFERENCE_INDEX.md  # This file
```

## Additional Resources

### Existing Documentation
- [ARCHITECTURE.md](./ARCHITECTURE.md) - System architecture
- [concepts/](./concepts/) - Conceptual guides
- [guides/](./guides/) - How-to guides

### Code Examples
All examples reference actual working code in the crates:
- `crates/sim-kitchen/` - Full kitchen implementation
- `crates/sim-logistics/` - Full logistics implementation

### Tests
Integration tests provide additional working examples:
- Kitchen multi-workflow: `crates/sim-kitchen/tests/multi_workflow_e2e_test.rs`
- Logistics integration: `crates/sim-logistics/tests/integration_test.rs`
- Core framework: `crates/sim-core/src/*/tests`

## Quality Standards

All examples follow these standards:
- ✓ Runnable or clearly marked as templates
- ✓ Include expected output
- ✓ Show error handling
- ✓ Reference actual framework code
- ✓ Well-commented
- ✓ Unit tested (templates)

## Support

For questions or issues:
1. Check API reference for the relevant crate
2. Review integration patterns
3. Examine working examples
4. Consult template files for boilerplate
