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
├── agents/                  Agent implementations
│   ├── customer.rs          Customer state machine
│   ├── courier.rs           Delivery agents
│   └── driver.rs            Supply chain drivers
│
├── resources/               Resources and tasks
│   ├── kitchen.rs           Kitchen resource
│   ├── order.rs             Order tasks
│   ├── inventory.rs         Inventory management
│   └── station.rs           KitchenStation (typed prep stations)
│
├── recipes/                 Recipe system
│   ├── types.rs             Recipe, RecipeStep, StationKind
│   └── registry.rs          RecipeRegistry (loads bundled brand templates)
│
├── profiles/                Deterministic agent profiles
│   ├── customer.rs          CustomerProfile (payment, address, loyalty)
│   ├── courier.rs           CourierProfile
│   ├── supply_driver.rs     SupplyDriverProfile
│   └── registry.rs          ProfileRegistry (lazy, seeded by AgentId)
│
├── artifact_generators/     Business artifact production
│   ├── order_created.rs     Web events, email, SMS, payment auth
│   ├── customer_done.rs     Receipt, loyalty transaction, review email
│   ├── supply_order.rs      Purchase order, vendor API call, AP entry
│   ├── inventory.rs         Receiving manifest, ledger entry
│   └── order_completed.rs   Kitchen ticket, ingredient usage, QC record
│
├── events.rs                CloudEvent payloads (incl. recipe step events)
├── locations/               Geospatial
│   └── distribution.rs      Distribution sites
│
├── workflows/               Multi-stage processes
│   ├── supply.rs            Supply chain workflow
│   └── delivery.rs          Courier delivery workflow
│
└── simulation.rs            Orchestration, hooks, artifact wiring
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

## Artifact Pipeline

Each simulation event is routed through an `ArtifactCollectorProcessor<ProfileRegistry>` which dispatches to registered `ArtifactGenerator` implementations. Generators produce business-realistic artifacts:

```
EventDispatcher.emit(event)
  → KitchenSimulationHooks.process_new_events_through_artifacts()
      → ArtifactCollectorProcessor.process(&json)
          → OrderCreatedArtifactGenerator  → 8 artifacts (web, email, SMS, payment)
          → CustomerDoneArtifactGenerator  → 3 artifacts (receipt, loyalty, review)
          → SupplyOrderArtifactGenerator   → 3 artifacts (PO, vendor API, AP)
          → InventoryArtifactGenerator     → 2 artifacts (manifest, ledger)
          → OrderCompletedArtifactGenerator→ 3 artifacts (ticket, usage, QC)
```

At run end, `flush_artifacts()` writes all artifacts to `output/artifacts/` as JSONL files grouped by type.

## Profile Registry

Agent profiles are generated deterministically from the agent's UUID bytes — same `AgentId` always produces the same profile:

```rust
let profile = hooks.profiles.customer_profile(agent_id);
// profile.email, profile.payment.card_last_four, profile.loyalty_tier, ...
```

## Recipe Registry

Recipes are loaded at startup from the brand templates bundled in `src/recipes/data/`:

```rust
let registry = RecipeRegistry::load_defaults();
let recipe = registry.get("burger").unwrap();
// recipe.steps[0].station == StationKind::Grill
```

## Extension Examples

The codebase shows how to extend for:
- Multiple agent types (Customer, Courier, Driver)
- Complex workflows (supply chain + delivery)
- Inventory tracking
- Distribution networks
- New artifact generators: implement `ArtifactGenerator<ProfileRegistry>` and register in `KitchenSimulationHooks::new()`
