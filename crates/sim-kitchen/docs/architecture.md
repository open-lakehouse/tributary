# sim-kitchen Architecture

## Design as Reference Implementation

`sim-kitchen` serves as the **canonical example** of building domain-specific simulations with the framework. It demonstrates:

1. **Agent State Machines**: Customer lifecycle
2. **Resource Allocation**: Kitchen assignment strategies
3. **Event Emission**: CloudEvents for observability
4. **Workflow Orchestration**: Multi-stage supply chain
5. **Geospatial Integration**: Location-based demand

## Module Organization

```
sim-kitchen/
├── agents/             Agent implementations
│   ├── customer.rs     Customer state machine
│   ├── courier.rs      Delivery agents
│   └── driver.rs       Supply chain drivers
│
├── resources/          Resources and tasks
│   ├── kitchen.rs      Kitchen resource
│   ├── order.rs        Order tasks
│   └── inventory.rs    Inventory management
│
├── events.rs           CloudEvent payloads
├── locations/          Geospatial
│   └── distribution.rs Distribution sites
│
├── workflows/          Multi-stage processes
│   ├── supply.rs       Supply chain workflow
│   └── delivery.rs     Courier delivery workflow
│
└── simulation.rs       Orchestration and hooks
```

## Customer State Machine

```
Idle (probabilistic)
  ↓
Ordering (emit demand)
  ↓
WaitingForFood (passive)
  ↓
Eating (timed duration)
  ↓
Done (terminal)
```

**Key Pattern**: Agents emit demands, framework handles allocation.

## Multi-Workflow Patterns

The simulation demonstrates running multiple workflow types concurrently:

### Supply Chain Workflow
1. Order from supplier
2. Transport to distribution center
3. Distribute to kitchens

### Courier Delivery Workflow
1. Pick up prepared order
2. Transport to customer
3. Complete delivery

**Integration Points**:
- Shared resource pools (kitchens)
- Event-driven coordination
- Spatial demand patterns

## Design Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| Probabilistic ordering | Realistic customer behavior | Non-deterministic unless seeded |
| Capacity-based kitchens | Real-world constraint | Requires allocation strategy |
| CloudEvents for all transitions | Complete observability | Event volume overhead |
| Multi-workflow support | Demonstrates complexity | Implementation overhead |

## Reference Patterns

### Pattern 1: Agent-Driven Demand
Customers emit demands during tick, framework allocates to resources.

### Pattern 2: Event-Driven Coordination
State transitions trigger events, enabling downstream reactions.

### Pattern 3: Geospatial Demand
Location-based order generation using H3 indexing.

### Pattern 4: Inventory Management
Track consumable resources (ingredients) with automatic depletion.

## Extension Examples

The codebase shows how to extend for:
- Multiple agent types (Customer, Courier, Driver)
- Complex workflows (supply chain + delivery)
- Inventory tracking
- Distribution networks
