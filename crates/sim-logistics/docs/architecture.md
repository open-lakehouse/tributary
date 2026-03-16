# sim-logistics Architecture

## Design as Framework Validation

`sim-logistics` validates the **generality** of the simulation framework by implementing a complete logistics domain with **ZERO** modifications to sim-core, sim-geo, or sim-events.

## Key Validation Points

### 1. Generic Abstractions Work
- Agent trait: Package and Driver implementations
- Resource trait: SortingStation and LoadingDock
- Task trait: ShipmentTask for workflow steps
- Workflow trait: PackageShipmentWorkflow

**Result**: No framework changes needed.

### 2. Allocation Strategies Reusable
- LeastLoaded: Assign packages to least-busy sorting stations
- RoundRobin: Distribute across loading docks
- Custom: Priority-based package routing

**Result**: Standard strategies work out-of-box.

### 3. Event Patterns Transferable
- CloudEvents 1.0 compliance maintained
- Same EventPayload trait pattern
- Same EventDispatcher usage

**Result**: Event infrastructure fully reusable.

### 4. Geospatial Components Generic
- LocationHandler: Warehouses and delivery addresses
- MovementHandler: Driver routes
- H3 indexing: Delivery zone optimization

**Result**: sim-geo abstractions are domain-agnostic.

## Module Organization

```
sim-logistics/
├── agents/             Package and Driver agents
│   ├── package.rs      Package lifecycle
│   └── driver.rs       Driver behavior
│
├── resources/          Logistics resources
│   ├── sorting.rs      SortingStation
│   └── loading.rs      LoadingDock
│
├── workflows/          Shipment pipeline
│   └── shipment.rs     PackageShipmentWorkflow
│
├── locations/          Facilities
│   ├── warehouse.rs    Distribution centers
│   └── address.rs      Delivery destinations
│
├── events.rs           CloudEvent payloads
└── simulation.rs       Orchestration
```

## Package Lifecycle

```
Received (at warehouse)
  ↓
Sorted (by sorting station)
  ↓
Loaded (onto vehicle)
  ↓
InTransit (with driver)
  ↓
Delivered (at destination)
```

**Pattern**: Same state machine approach as sim-kitchen, different domain.

## Shipment Workflow Steps

```rust
pub struct PackageShipmentWorkflow {
    steps: [
        WorkflowStep { required_resource_type: "sorting_station", ... },
        WorkflowStep { required_resource_type: "loading_dock", ... },
        WorkflowStep { required_resource_type: "driver", ... },
        WorkflowStep { required_resource_type: "delivery", ... },
        WorkflowStep { required_resource_type: "confirmation", ... },
    ],
}
```

**Pattern**: Same WorkflowStep structure, different resources.

## Design Decisions

| Decision | Rationale | Framework Impact |
|----------|-----------|------------------|
| 5-step workflow | Real logistics pipeline | None - Workflow trait handles it |
| Priority packages | Domain requirement | None - Custom Task metadata |
| Capacity constraints | Real-world limits | None - Resource trait supports it |
| Driver routing | Geospatial needs | None - sim-geo provides it |

## Framework Generality Proof

### Before sim-logistics

Framework components:
- sim-core: Generic agent/resource/workflow abstractions
- sim-geo: Generic location/movement/routing
- sim-events: Generic CloudEvents infrastructure

### After sim-logistics

Framework components: **UNCHANGED**

New code: **100% domain-specific** (no framework modifications)

**Conclusion**: Framework abstractions are truly domain-agnostic.

## Comparison with sim-kitchen

| Aspect | sim-kitchen | sim-logistics | Framework Changes |
|--------|-------------|---------------|-------------------|
| Agents | Customer, Courier | Package, Driver | None |
| Resources | Kitchen | SortingStation, LoadingDock | None |
| Tasks | Order | ShipmentTask | None |
| Workflows | Supply chain | Package shipment | None |
| Events | 5 types | 5 types | None |
| Locations | Distribution sites | Warehouses, Addresses | None |

**Result**: Same framework, different domain, no modifications.
