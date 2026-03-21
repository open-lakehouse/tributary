# Changelog

All notable changes to `sim-kitchen` will be documented in this file.

## [Unreleased] - 2026-03-21

### Added

#### sim-artifacts crate (new)
- **`Artifact` enum**: 7 variants — `Document`, `StructuredData`, `WebEvent`, `EmailMessage`, `SmsMessage`, `PushNotification`, `ApiCall`
- **`ArtifactGenerator<P>` trait**: `handles(&str)` + `generate(&mut ArtifactContext<P>)` — mirrors `TelemetryGenerator`
- **`ArtifactCollector<P>`**: Aggregates generators; `process_event()` / `flush()`
- **`ArtifactCollectorProcessor<P>`**: Wraps collector as a `LocalEventProcessor` (Arc+Mutex interior mutability)
- **`ArtifactStorage` trait** + **`NoOpStorage`**: Persist/flush interface
- **`ParquetArtifactStorage`**: Writes JSONL files grouped by artifact type under a configurable base directory

#### Artifact Generators (in sim-kitchen)
- **`OrderCreatedArtifactGenerator`** (`io.kitchen.order.created`): 4 web events + order confirmation email + SMS + payment API call + payment authorization record
- **`CustomerDoneArtifactGenerator`** (`io.kitchen.customer.done`): HTML receipt document + loyalty transaction + review request email
- **`SupplyOrderArtifactGenerator`** (`io.kitchen.supply.order.created`): Purchase order HTML + vendor API call + accounts payable entry
- **`InventoryArtifactGenerator`** (`io.kitchen.inventory.replenished`): Receiving manifest HTML + inventory ledger entry
- **`OrderCompletedArtifactGenerator`** (`io.kitchen.order.completed`): Kitchen ticket + ingredient usage + quality control record

#### Profile Registry
- **`ProfileRegistry`**: Lazy, deterministic agent profiles keyed by `AgentId`; same ID always yields same profile
- **`CustomerProfile`**: Name, email, phone (E.164), address, `PaymentProfile` (card type, last four, expiry), loyalty tier
- **`CourierProfile`**: Name, contact, vehicle, license plate, rating, delivery count
- **`SupplyDriverProfile`**: Name, company, CDL class, route IDs, experience

#### Recipe System
- **`RecipeRegistry::load_defaults()`**: Parses bundled universe brand templates (fast_food, asian, mexican) into `Recipe` / `RecipeStep`
- **`StationKind`** enum: `Workstation`, `Stove`, `Fryer`, `Oven`, `Grill`, `Prep`, `Unknown`
- **`KitchenStation`** resource: Typed prep station with capacity

#### New Events
- `io.kitchen.recipe.step.started` (`RecipeStepStartedEvent`)
- `io.kitchen.recipe.step.completed` (`RecipeStepCompletedEvent`)

#### Simulation Wiring
- `KitchenSimulationHooks` now holds `ArtifactCollectorProcessor<ProfileRegistry>` with all 5 generators registered
- Per-tick artifact processing via `process_new_events_through_artifacts()` (uses `current_tick_events()`)
- `flush_artifacts()` called at run end; writes to `output/artifacts/`
- A 50-tick run produces ~268 artifacts across 8 output files

#### sim-telemetry
- **`TelemetryCollectorProcessor`**: Adapts `TelemetryCollector` to a local `EventProcessor` interface (avoids `sim-core → sim-telemetry → sim-events → sim-core` cycle)

---

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
