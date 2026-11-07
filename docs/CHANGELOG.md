# Changelog

All notable changes to Tributary will be documented in this file. Nothing has
been released since 0.0.1, so everything below `[Unreleased]` is grouped by the
date the work landed.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Documentation
- Docs no longer reference the removed `set_workflow_engine` (examples use
  `WorkflowRegistry`), the old `migration/` tree, a nonexistent `sim-runner`
  metrics endpoint, or absolute paths from a previous checkout. Crate lists
  include `sim-artifacts`, `sim-validate` and `caspers-cli`; both new crates
  gained READMEs.

### Fixed
- The `caspers` Python package builds again: maturin's `module-name` is
  `caspers._internal`, but the Python sources still lived in
  `python/caspers_universe/`, so wheels shipped without `__init__.py` and
  `import caspers` failed. The package licence now matches the workspace
  (`MIT OR Apache-2.0`).
- `caspers run --no-tui` no longer hangs in range mode. The kitchen simulation
  pauses at the step limit to keep the TUI alive, and the headless driver now
  stops the run when it sees that pause.

### Changed: dependency refresh
- `arrow`/`parquet` 58 → 59 and `datafusion` 53.1 → 55, matching Hydrofoil and
  mangrove (arrow 60 is out, but datafusion 55 builds on 59).
- `rand` unified on 0.10 (sim-core was on 0.8, the domain crates on 0.9) with
  `rand_distr` 0.6. Call sites use the new `RngExt` extension trait. Seeded runs
  are still reproducible, but a given seed now produces a different stream than
  before.
- `geo` 0.31 → 0.33 and `h3o` 0.9 → 0.11, which also drops the duplicate `geo`
  that h3o used to pull in.
- `pyo3` 0.26 → 0.30, `ratatui` 0.29 → 0.30, `crossterm` 0.28 → 0.29,
  OpenTelemetry 0.31 → 0.33 (`tracing-opentelemetry` 0.34), `criterion` 0.5 → 0.8
  (benches use `std::hint::black_box`), plus a lockfile-wide `cargo update`.

### Changed: workspace tooling and CI brought in line with sibling repos
- Every external dependency is now declared once in `[workspace.dependencies]`;
  crates inherit with `{ workspace = true }`. Workspace `rust-version` 1.91 → 1.95.
- Dev toolchain pinned to 1.99 in `rust-toolchain.toml`. `[profile.dev]` keeps
  line tables for workspace crates only; `--profile debugging` gives full debug info.
- CI: `--locked` everywhere, dedicated `msrv`, `machete`, `doc` (`-D warnings`) and
  `ruff` jobs, a single `ci` roll-up check, refreshed action pins, and a `zizmor`
  workflow. nextest's `ci` profile no longer retries failing tests.
- `just lint` / `test` / `doc` / `msrv` / `check` / `fix` recipes mirror CI.
- `AGENTS.md` is the canonical agent guide; `CLAUDE.md` imports it.
- Root `pyproject.toml` renamed to `tributary-notebooks`, trimmed to what the
  notebooks import, with a ruff configuration.
- Removed dead files: `biome.json`, `prompts/`, `generate_test_data.py`,
  `test_data/`, `crates/cli/pyproject.toml`, the `build-py-cli` recipe and the
  `buf-format` pre-commit hook. Typos config moved into `Cargo.toml`.

### Changed — align dependencies with Hydrofoil for integration
- Bumped workspace dependencies to match the Hydrofoil sibling repository so
  Tributary can be consumed as a dependency from it without graph conflicts:
  `arrow` 57 → 58, `parquet` 57 → 58, `datafusion` 51 → 53.1,
  `chrono` ^0.4.40 → 0.4.44, and workspace `rust-version` 1.90 → 1.91.
- No source changes were required; the bumps are API-compatible for this
  workspace. Verified that the full arrow/datafusion graph resolves to a single
  version (arrow 58.3, datafusion 53.1, object_store 0.13.2, sqlparser 0.61)
  when `sim-core` is pulled into Hydrofoil.

### Removed — legacy `universe` crate and proto/buf pipeline
- Deleted `crates/universe` (legacy DataFusion-based engine; no remaining consumers).
- Deleted `proto/`, `buf.gen.yaml`, `proto/buf.yaml`, and the `just generate` recipe —
  the entire buf pipeline only generated code into the `universe` crate.
- Relocated the three brand recipe templates consumed by `sim-kitchen`
  (`fast_food.json`, `asian.json`, `mexican.json`) from `universe/templates/base/brands/`
  into `crates/sim-kitchen/src/recipes/data/`.

### Changed — renamed packages off "universe"
- `caspers-universe-cli` → `caspers-cli` (binary remains `caspers`).
- `caspers-universe-python` → `caspers-python`; Python module `caspers_universe` → `caspers`.
- Removed dead `build-ui`/`dev-ui`/`install-ui`/`docs`/`build-docs` justfile recipes
  (no `ui/` or `docs/` npm workspace exists).
- Cleaned all clippy warnings; `cargo clippy --workspace --all-targets -- -D warnings`
  is now clean.

---

## Unreleased work, 2026-05-17 (Python bindings migration)

### Changed — Python bindings migrated from legacy `universe` crate to `sim-*` framework

#### python: New API
- `run_kitchen_simulation(steps, num_kitchens, kitchen_capacity, num_customers, output_dir) -> KitchenResult` — runs `KitchenSimulation` headlessly and writes events to Parquet
- `run_logistics_simulation(steps, output_dir) -> LogisticsResult` — runs `LogisticsSimulation` and writes events to Parquet; returns delivery metrics
- `KitchenResult` and `LogisticsResult` data classes with typed properties
- Removed: `Site`, `load_simulation_setup`, `run_simulation` (legacy `universe` wrappers), `prepare_site`, `site_routing_graph`, `plot_site`
- `python/pyproject.toml` dependencies trimmed to `pyarrow` + `polars` (removed geo/routing stack no longer needed)

#### sim-logistics: Event access
- `LogisticsSimulationHooks::dispatcher()` / `dispatcher_mut()` — new accessors for the internal `EventDispatcher`
- `LogisticsSimulation::save_events(path)` — writes emitted events to a Parquet file (mirrors `KitchenSimulation::save_events`)
- `LogisticsSimulationHooks` now re-exported from crate root

#### notebooks
- `notebooks/sim_kitchen.py` — new Marimo notebook: sim params as sliders, run button, full event analysis (summary, timeline, order Gantt, duration histograms, raw event browser)
- `just kitchen` command added to open the new notebook
- Deleted: `notebooks/prepare.py` (called legacy `run_simulation`)

---

## Unreleased work, 2026-05-17

### Added — Migration completion & CLI validation

#### sim-core: Snapshot fidelity
- `ResourceAllocator::import_batch()` now faithfully restores `allocated_at_millis` (col 8) and `estimated_duration_secs` (col 9) from the task RecordBatch — previously used `Utc::now()` and a hardcoded 10s default, causing tasks to complete at wrong times after a snapshot round-trip
- Added `test_14_snapshot_round_trip_preserves_timing_fields` verifying tasks do not complete early after restore

#### cli: Result validation
- Stats panel (right 35% of body area) shows cumulative event counts by type, rolling throughput, and completion ratio
- `event_color()` now uses a hash-based palette for any domain's event types (`io.logistics.*`, etc.) instead of returning `Color::White`
- `short_event_type()` strips any `io.<domain>.` prefix, not just `io.kitchen.`
- `--no-tui` flag: runs simulation headlessly and prints a JSON summary to stdout (`{"steps": N, "events_by_type": {...}}`)
- End-of-run summary printed to stderr after TUI closes: steps, sim time, per-type event counts, throughput, completion ratio

## Unreleased work, 2026-03-21

### Added — TUI Simulation & Snapshot Management

#### sim-core: Context
- `SimulationContext::set_simulation_id(id)` — override the generated UUID when loading a pre-registered simulation from the catalog

#### sim-core: Control Protocol
- `ControlCommand::RestoreSnapshot(Uuid)` — request restoration of a previously saved snapshot; runner pauses after restore
- `SimEvent::SnapshotRestored { snapshot_id, label, sim_time, step }` — emitted after successful restore
- `SimEvent::SnapshotsUpdated { snapshots }` — pushed after any snapshot operation so callers can refresh their list
- `SnapshotSummary` — lightweight struct carried by `SnapshotsUpdated`

#### sim-core: SimulationManager persistence
- `SimulationManager::load(backend)` — loads catalog from disk (JSON files) after calling `new()`; no-op for Memory backend
- `SimulationManager::new_with_persistence(base)` — convenience alias for `load(Parquet(base))`
- `SimulationManager::load_catalog_from_disk(base)` — scans `{base}/system/simulations/` and `{base}/system/snapshots/` JSON files and repopulates in-memory tables
- `persist_simulation` / `persist_snapshot` — write JSON records to `.caspers/system/{simulations,snapshots}/` on every `create_simulation` / `record_snapshot` call

#### cli: Multi-screen TUI
- `Screen::{Launch, Simulation, SnapshotBrowser}` — top-level enum; `App::render()` dispatches on current screen
- `App::new_launch(sim_list)` — creates an App starting on the Launch screen
- `Dialog::{NewSimName, TakeSnapshot, ConfirmRestore}` — new modal dialogs for snapshot/sim operations
- Launch screen: lists existing simulations, `[n]` creates new, `[Enter]` loads selected, `[↑/↓]` navigates
- Snapshot browser (`[S]` from simulation screen): lists snapshots, `[t]` takes snapshot, `[Enter]` triggers restore confirm, `[Esc]` returns to simulation
- `ConfirmRestore` dialog: `[y]` sends `ControlCommand::RestoreSnapshot`, `[n/Esc]` cancels
- Notification overlay: auto-dismissing 3-second notification shown after snapshot saved/restored
- `[S] Snapshots` hint added to simulation screen help bar

### Added — Snapshot Management & SimulationManager

#### sim-core: Structured Snapshot IDs
- `SimulationContext::simulation_id()` — each context gets a `Uuid::now_v7()` on construction; exposed via accessor
- `SnapshotMetadata` — new fields: `snapshot_id: Uuid`, `simulation_id: Uuid`, `label: Option<String>`, `sim_time: DateTime<Utc>`
- `SnapshotMetadata::with_ids()` — constructor for callers that supply explicit IDs; `new()` still works and generates IDs internally
- `SnapshotManager::save_snapshot_with_id()` — saves with explicit IDs and registers in internal `snapshot_registry` MemTable
- `SnapshotManager::list_snapshots()` — list all snapshots recorded in this manager instance
- `SnapshotManager::describe_snapshot(id)` — look up a single snapshot by UUID string

#### sim-core: Snapshot via Control Channel
- `ControlCommand::Snapshot(label)` — request an immediate snapshot while the sim is running
- `SimEvent::SnapshotSaved { snapshot_id, label, sim_time, step }` — emitted after a successful snapshot
- `SimulationRunner::save_snapshot_with_id()` — async helper for explicit-ID snapshots
- `SimulationRunner::save_snapshot_with_ids_sync()` — sync wrapper used by the control-command handler

#### sim-core: SimulationManager
- New module `sim_core::manager` with `SimulationManager` — catalog layer tracking multiple simulation runs and their snapshots
- `SimulationManagerBackend::{Memory, Parquet}` — backend selector
- `SimulationManager::create_simulation()` → `Uuid` — register a new run; generates Uuid v7
- `SimulationManager::list_simulations()` → `Vec<SimulationRecord>`
- `SimulationManager::record_snapshot()` — record a snapshot taken by a runner
- `SimulationManager::list_snapshots(simulation_id)` → `Vec<SnapshotRecord>` — filtered by simulation
- `SimulationManager::snapshot_manager_for(simulation_id)` — create a scoped `SnapshotManager`
- `SimulationRecord` and `SnapshotRecord` plain-data structs (no Arrow dependency on callers)

#### cli: SnapshotSaved event handling
- `process_sim_event` in `caspers` CLI now handles the new `SimEvent::SnapshotSaved` variant (logs at info level)

### Added — TUI/CLI UX Improvements

#### sim-core: Speed Control and Pause-on-End
- `ControlCommand::SetSpeed(f32)` — set Live-mode speed multiplier (clamped to [0.125, 16.0])
- `SimEvent::SpeedChanged { multiplier: f32 }` — emitted when speed changes
- `SimulationContext::live_speed: f32` field — speed multiplier used by `sleep_duration()`
- `SimulationRunner::set_paused(bool)` — public method for domain wrappers to enter paused state

#### sim-kitchen: Pause on Range End
- `KitchenSimulation::step()` now returns `TickResult` so callers can inspect it
- `run_controlled_for`: when Range mode hits `end_time`, emits `Paused` and enters the pause loop rather than stopping — allows user to advance range in TUI without restarting

#### CLI: Human-Friendly Time Parsing
- New `time_parse::parse_sim_time()` — accepts `now`, `now+3h`, `now-30m`, `2026-01-01`, full RFC 3339
- `--start-time` and `--end-time` now use this parser; help text updated

#### CLI TUI: Guided Mode Dialog and Improved Layout
- `[m]` opens a `ModeSelect` dialog instead of blindly cycling — eliminates the undefined-behavior bug where switching back to Range sent `end_time: None`
- `[a]` key opens `AdvanceBy` dialog when in Range mode — extend the simulation end time without restarting
- `[+]`/`[-]` keys adjust Live-mode speed (doubles / halves, shown in help bar)
- New header row: title + colored mode badge (`[RANGE]`/`[LIVE]`/`[CATCHUP]`) + full datetime + step counter
- Body area horizontally split: left totals panel (one entry per event type) + right event log
- Help bar is context-sensitive: shows `[a]`/`[+/-] Speed Nx` only when relevant
- Mode badge colors: Range=Blue, Live=Green, Catchup=Yellow

### Added — Interactive TUI with Bidirectional Control Channel

#### sim-core: Control Channel Enhancements
- `SimEvent::TickCompleted` now carries `event_counts: HashMap<String, u64>` — per-tick domain event deltas flow through the same channel, eliminating a second side-channel
- `SimulationRunner` gains three public methods: `process_control_commands()`, `is_paused()`, `emit_event()` — enabling domain wrappers to drive the control loop around their own `step()` implementations

#### sim-kitchen: `run_controlled_for()`
- New `KitchenSimulation::run_controlled_for(max_steps)` drives the simulation via the control channel while preserving full domain bookkeeping (order delivery, artifact generation, customer state)
- Removed `SimulationMessage`, `tui_tx`, `tui_total_steps`, and `set_tui_channel()` — the control channel is now the single transport for all simulation events

#### CLI TUI: Interactive Controls
All time modes (Range, Live, Catchup) now use a unified sim thread via `run_controlled_for()`.

New keyboard controls:
| Key | Action |
|-----|--------|
| `Space` | Pause / Resume |
| `s` | Step 1 tick (while paused) |
| `f` | Step 10 ticks (while paused) |
| `m` | Cycle time mode: Range → Live → Catchup → Range |
| `q` / `Esc` / `^C` | Stop and quit |

UI additions:
- Progress bar title shows current time mode badge (`[Range]`, `[Live]`, `[Catchup]`) and `[■ PAUSED]` indicator
- Gauge turns yellow when paused
- New **Controls** status bar at bottom with context-sensitive key hints



### Added — Realistic Multi-Type Data Artifact Generation

#### New Crate: `sim-artifacts`
Generic framework for generating business artifacts from simulation events. Mirrors `sim-telemetry` architecture — no dependency on `sim-kitchen`.

- `Artifact` enum (7 variants: Document, StructuredData, WebEvent, EmailMessage, SmsMessage, PushNotification, ApiCall)
- `ArtifactGenerator<P>` trait for domain-specific generators
- `ArtifactCollector<P>` aggregating multiple generators
- `ArtifactCollectorProcessor<P>` bridging to the event pipeline via `LocalEventProcessor`
- `ArtifactStorage` trait + `ParquetArtifactStorage` (JSONL output, grouped by type)

#### sim-events: EventProcessor Pipeline
- `EventProcessor` trait: pluggable, non-blocking processors registered on `EventDispatcher`
- `ChannelSenderProcessor`: fire-and-forget event forwarding on mpsc channel
- `ParquetEventProcessor`: processor-based Parquet persistence (replaces direct flush calls)
- Tick index: `mark_tick_start()` / `current_tick_events()` for per-tick event slices
- `flush_to_parquet()` / `flush_to_jsonl()` deprecated in favour of processor-based persistence

#### sim-kitchen: Business Artifact Generators
Five generators producing ~268 artifacts per 50-tick run:
- Orders: web analytics funnel (4 events), email confirmation, SMS, payment API call + authorization record
- Customer done: HTML receipt, loyalty transaction, review request email
- Supply orders: purchase order HTML, vendor API call, accounts payable entry
- Inventory: receiving manifest HTML, inventory ledger entry
- Order completed: kitchen ticket, ingredient usage, quality control record

#### sim-kitchen: Profile Registry
Deterministic agent profiles seeded by `AgentId` bytes — reproducible across runs.
`CustomerProfile`, `CourierProfile`, `SupplyDriverProfile` with realistic fake fields.

#### sim-kitchen: Recipe System
`RecipeRegistry::load_defaults()` parses bundled brand templates.
`StationKind` enum maps template strings to typed prep stations.
New events: `io.kitchen.recipe.step.started`, `io.kitchen.recipe.step.completed`.

#### sim-telemetry: TelemetryCollectorProcessor
Adapts `TelemetryCollector` to the event pipeline without introducing a circular dependency.

### Planned
- Phase 12-14: Python bindings (PyO3), CLI tool, comprehensive benchmarks
- Multi-language support for simulation definitions
- Real-time visualization dashboard
- Distributed simulation support

---

## [0.0.1] - 2026-03-16

### Summary

Initial release of the Generic Agent Simulation Framework, a Rust-based system for building large-scale agent-based simulations with focus on performance, maintainability, and domain flexibility.

**Project Status:** Production-ready foundation (71% complete, Phases 1-11)

**Key Metrics:**
- **Performance:** tick time comfortably under the 100ms/tick target (specific
  speedup figures from this period are unverified and have been omitted)
- **Test Coverage:** full workspace test suite passing across all crates
- **Code Quality:** Zero clippy warnings on library code
- **Documentation:** Comprehensive API docs, guides, examples, and architecture documentation
- **Domains:** 2 complete domain implementations (kitchen, logistics)

---

### Added - Core Framework (Phase 1-6)

#### Simulation Engine (sim-core)
- **SimulationRunner**: Tick-based simulation loop with lifecycle hooks
- **PopulationHandler**: Agent management with Arrow RecordBatch storage and HashMap index
- **WorkflowEngine**: Multi-step business process orchestration
  - WorkflowDefinition and WorkflowInstance types
  - Step-based progression with resource requirements
  - Batch update operations (785x speedup for 1,000 instances)
- **WorkflowRegistry**: Multi-workflow orchestration for concurrent workflow types
- **ResourceAllocator**: Task-resource matching and allocation
  - Capacity-aware resource management
  - Priority-based task queuing
  - Dual RecordBatch storage (resources + tasks)
- **TaskFactory**: Trait for domain-specific task creation from workflow steps
- **SnapshotManager**: State save/restore with dual backend support
  - Memory backend for testing (zero-copy)
  - Parquet backend for persistence (9x compression)
  - 15ms save, 7ms restore for 1,500 agents
- **Query Engine**: SQL interface with DataFusion
  - Cross-entity JOIN operations
  - Aggregation and filtering
  - 3.75x faster than manual iteration

#### Geospatial Operations (sim-geo)
- **LocationHandler**: Point location tracking with H3 spatial indexing
  - Haversine distance calculations
  - Radius-based proximity queries
  - Nearest-neighbor search
  - Dual-index (RecordBatch + HashMap + H3)
- **MovementHandler**: Journey and velocity-based movement
  - Journey planning and tracking
  - Progress-based position updates
  - Route geometry support
- **RoadNetwork**: Graph-based road network with routing
  - Node and edge management
  - Fast routing with Contraction Hierarchies (fast_paths library)
  - 5,645x speedup with graph caching (1.86ms → 330ns per query)
  - JSON serialization/deserialization
- **VelocityModel**: Pluggable velocity calculation
  - ConstantVelocity: Fixed speed
  - TimeOfDayVelocity: Rush hour patterns (60% and 50% speed reductions)
- **SpatialIndex**: H3 hexagonal hierarchical indexing
  - Sub-millisecond proximity queries
  - Cluster identification
  - Zone-based operations

#### Event Infrastructure (sim-events)
- **Event**: CloudEvents 1.0 compliant event types
  - AgentStateChanged, ResourceAllocated, WorkflowStepCompleted, etc.
  - Standard metadata (id, source, type, time, subject)
- **EventDispatcher**: Event buffering and emission
  - Batch event collection
  - Filtered dispatch
  - Parquet storage support
- **EventStorage**: Parquet-based event persistence
  - Columnar storage for efficient querying
  - Time-series analysis support

#### Telemetry Framework (sim-telemetry)
- **TelemetryGenerator**: Trait for log/trace/metric generation from events
  - EventToLogGenerator: Structured logs with error injection
  - EventToTraceGenerator: Distributed traces with span hierarchy
  - EventToMetricGenerator: Counters, gauges, histograms
- **TelemetryCollector**: Multi-generator aggregation
  - Batch collection (43ms for 10K events)
  - <1% simulation overhead
- **Export Formats**:
  - OTLP (OpenTelemetry Protocol): Industry-standard format
  - JSON: Structured logs and traces
  - Prometheus: Metrics exposition format
- **Performance**: 4-36x better than targets (1.07µs logs, 2.27µs traces, 502ns metrics)

---

### Added - Demand Generation (Phase 10)

#### Demand Generators (sim-core)
- **PoissonDemandGenerator**: Realistic random arrivals with exponential inter-arrival times
  - Configurable base rate (λ parameter)
  - Statistical validation (chi-squared goodness-of-fit)
  - Thread-safe RNG with seeding
- **TimeOfDayDemandGenerator**: Time-based demand patterns
  - Configurable peak periods with rate multipliers
  - Smooth transitions between periods
  - 24-hour cycle support (rush hours, meal times, business hours)
- **SpatialDemandGenerator**: H3-based location clustering
  - Zone-specific density multipliers
  - Optional capacity limits per zone
  - Hot-spot modeling for high-demand areas
- **CompositeDemandGenerator**: Multi-pattern composition
  - Multiple composition strategies (All, First, Weighted)
  - Extensible for complex scenarios
  - Chain multiple generators with priority ordering
- **Performance**: 85% better than 5ms/tick target (0.75ms actual overhead)

---

### Added - Domain Implementations

#### Kitchen Domain (sim-kitchen)
- **Agents**:
  - Customer: Order placement and consumption (5-state lifecycle)
  - SupplyDriver: Delivery agent for supply chain (4-state machine)
  - Courier: Food delivery agent (4-state machine, 12 m/s movement)
- **Resources**:
  - Stove: Cooking resource with capacity constraints
  - Table: Seating resource for customers
- **Workflows**:
  - FoodPrepWorkflow: 3-step cooking process (Prep → Cook → Package, 480s total)
  - SupplyChainWorkflow: 5-step replenishment (OrderPlaced → DriverAssigned → PickingUp → InTransit → Delivered, 50 min)
  - CourierDeliveryWorkflow: 4-step delivery (OrderAccepted → PickingUp → EnRoute → Delivered, 17 min)
- **Inventory System**:
  - Thread-safe stock management (Arc<Mutex<HashMap>>)
  - Atomic batch operations (all-or-nothing)
  - Reorder point tracking with auto-replenishment
  - Recipe system (burger, fries, salad, pizza)
- **Locations**:
  - DistributionSite: Unlimited supply sources with H3 indexing
- **Events**: 10+ domain-specific events (OrderCreated, InventoryConsumed, SupplyOrderCreated, etc.)
- **Tests**: 112 tests passing (30 customer, 23 inventory, 19 courier, etc.)

#### Logistics Domain (sim-logistics) - **Proves Framework Generality**
- **Agents**:
  - Package: Shipment tracking (8-state lifecycle: Created → Delivered)
  - Driver: Delivery driver with truck capacity (30 packages, 5-state machine)
- **Resources**:
  - SortingStation: Multi-lane sorting with zone assignment
  - LoadingDock: Bay management for truck loading
- **Workflows**:
  - PackageShipmentWorkflow: 5-step delivery (Receiving → Sorting → Staging → Loading → Delivery, 44 min)
  - TaskFactory: Step-to-task conversion with location awareness
- **Locations**:
  - Warehouse: H3-indexed distribution centers
- **Tests**: 77 tests passing (12 package, 14 driver, 9 sorting, 8 dock, 11 workflow, etc.)
- **Critical Achievement**: ZERO framework modifications needed for second domain

---

### Changed

#### Phase 3: Code Simplification
- **Refactored** sim-geo distance calculations into reusable helpers
  - `find_nearest_in_candidates()`: Single nearest location
  - `filter_candidates_by_distance()`: All locations within threshold
  - Eliminated 40 lines of duplicated code
  - Added 7 comprehensive unit tests

#### Phase 6: Arrow Migration (10-100x speedup)
- **Migrated** all handlers from HashMap to Arrow RecordBatch storage
  - PopulationHandler: Agent storage (~600 lines modified)
  - ResourceAllocator: Resources + tasks (~900 lines modified)
  - WorkflowEngine: Workflow instances (~800 lines modified)
  - LocationHandler: Spatial data with H3 index (~500 lines modified)
- **Implemented** dual-index architecture (RecordBatch + HashMap)
  - Fast bulk operations (columnar storage)
  - O(1) single-row lookups (HashMap index)
  - 20% memory overhead, 10-100x performance gain
- **Performance Results**:
  - Bulk agent status update: 30x faster (15ms → 0.5ms)
  - Batch task allocation: 60x faster (12ms → 0.2ms)
  - Snapshot save: 66x better than target (15ms vs 1s)
  - Parquet compression: 9x reduction (1.8 MB → 200 KB)

---

### Performance Improvements

> **Note:** the specific speedup multipliers and microbenchmark figures in this
> historical section were recorded during the original migration and have not been
> re-validated. Treat them as directional, not authoritative; re-benchmark before
> relying on any number here.

#### Phase 3: Batch Operations (7-785x speedup)
- **Optimized** WorkflowEngine batch updates
  - Small batch (10 updates): 7.5x speedup
  - Medium batch (100 updates): 53.8x speedup
  - Large batch (500 updates): 209x speedup
  - **Large batch on large table (1,000 updates): 785x speedup**
- **Impact**: Eliminates O(N²) RecordBatch rebuilds, reduces to O(N)

#### Phase 6: Arrow Columnar Storage (10-100x speedup)
- **Optimized** bulk operations with columnar storage
  - Bulk agent updates: 30x faster
  - Batch task allocation: 60x faster
  - SQL aggregations: 3.75x faster
- **Impact**: Columnar access patterns excel at filtering and aggregation

#### Phase 7: Cached Routing (5,645x speedup)
- **Optimized** road network routing with OnceLock caching
  - Before: 1.86ms per query (FastGraph rebuilt every query)
  - After: 330ns per query (FastGraph cached, built once)
  - **Impact: 5,645x speedup (537 → 3.03M queries/sec)**
- **Lesson**: Separate preprocessing from execution, cache expensive setup

#### Phase 8: Telemetry Performance (4-36x better than targets)
- **Optimized** telemetry generation overhead
  - Log generation: 9.3x better (10µs → 1.07µs)
  - Trace generation: 4.4x better (10µs → 2.27µs)
  - Metric generation: 20x better (10µs → 502ns)
  - **Total overhead: <1% (5x better than 5% target)**
- **Impact**: Infrastructure code has negligible performance cost

#### Phase 9: Multi-Workflow Compound Effect (1,667x better than target)
- **Achieved** exceptional multi-workflow performance
  - 24-hour simulation: 0.06ms average tick time
  - Target: <100ms per tick
  - **Achievement: 1,667x better than target**
- **Impact**: All prior optimizations compound multiplicatively

#### Phase 10: Demand Generation (85% better than target)
- **Optimized** demand generation overhead
  - Poisson: 85% better (1ms → 0.15ms)
  - TimeOfDay: 70% better (0.1ms → 0.03ms)
  - Spatial: 76% better (0.5ms → 0.12ms)
  - **Total: 0.75ms/tick (85% better than 5ms target)**
- **Impact**: Performance budget from prior phases allows feature additions

#### Cumulative Performance Summary
| Metric | Baseline | Phase 11 | Improvement |
|--------|----------|----------|-------------|
| Tick time (1,500 agents) | ~150ms | 0.06ms | **2,500x** |
| Batch updates (1,000) | 14.1ms | 18µs | **785x** |
| Routing query (cached) | 1.86ms | 330ns | **5,645x** |
| Snapshot save | N/A | 15ms | New feature |
| Telemetry overhead | N/A | <1% | New feature |

---

### Fixed

#### Phase 1: Bug Fixes and Stabilization
- **Fixed** 3x task multiplication bug in workflow integration
  - Root cause: Task creation called 3 times per workflow step
  - Solution: Moved task creation to correct lifecycle hook
- **Fixed** H3 schema mismatch in movement handler
  - Root cause: node_index field type mismatch (Int32 vs Int64)
  - Solution: Static schema definition with Int64
- **Stabilized** 240 tests, all passing
  - Resolved pre-existing flaky tests
  - Established performance baseline

#### Phase 6: Arrow Migration Breaking Change
- **Extended** Task schema from 8 to 10 columns (BREAKING)
  - Added: `allocated_at_millis` (timestamp of allocation)
  - Added: `estimated_duration_secs` (task duration estimate)
  - Migration guide provided: `migration/guides/PHASE_6_MIGRATION_GUIDE.md`
  - Impact: Old snapshots incompatible, must regenerate

---

### Breaking Changes

#### [0.0.1] - Task Schema Extension (Phase 6)

**What Changed:**
Task schema extended from 8 to 10 columns.

**Before (8 columns):**
```rust
id, task_type, status, required_capacity,
assigned_resource_id, workflow_instance_id,
priority, metadata
```

**After (10 columns):**
```rust
id, task_type, status, required_capacity,
assigned_resource_id, workflow_instance_id,
priority, metadata,
allocated_at_millis,        // NEW
estimated_duration_secs     // NEW
```

**Impact:**
- Old snapshots with 8-column tasks cannot be restored
- Task implementations must provide `estimated_duration()` method
- SQL queries get two new columns

**Migration:**
See `migration/guides/PHASE_6_MIGRATION_GUIDE.md`

**Workaround:**
Re-run simulations to generate new snapshots, or manually migrate Parquet files (add zero-filled columns).

---

### Documentation

#### Guides
- **Getting Started**: `docs/GETTING_STARTED.md`
  - Installation and setup
  - First simulation (kitchen domain)
  - Core concepts walkthrough
- **Building a Custom Domain**: `docs/guides/building-domain.md`
  - Domain structure and patterns
  - Agent, Resource, Workflow trait implementations
  - TaskFactory pattern
  - Integration with SimulationRunner
- **Custom Agents**: `docs/guides/custom-agents.md`
  - Agent trait implementation
  - State machine design
  - Arrow serialization
- **Custom Workflows**: `docs/guides/custom-workflows.md`
  - WorkflowDefinition creation
  - Step configuration
  - TaskFactory implementation
- **Custom Resources**: `docs/guides/custom-resources.md`
  - Resource trait implementation
  - Capacity constraints
  - Allocation logic
- **Performance Tuning**: `docs/guides/performance-tuning.md`
  - Profiling strategies
  - Optimization patterns
  - Common pitfalls
- **Testing Strategies**: `docs/guides/testing-strategies.md`
  - Unit testing patterns
  - Integration test setup
  - Benchmarking with Criterion

#### Architecture Documentation
- **Architecture Overview**: `docs/ARCHITECTURE.md`
  - Core design decisions
  - Framework architecture
  - Integration patterns
  - Extension points
- **Performance Profiling Guide**: `docs/performance/profiling.md`
  - Profiling tools (Criterion, perf, Valgrind, flamegraph)
  - Optimization workflow
  - Case studies from migration phases
- **Optimization History**: `docs/performance/optimization-history.md`
  - Phase-by-phase performance timeline
  - Key optimization breakthroughs
  - Lessons learned
  - Future opportunities

#### API Reference
- **sim-core**: `docs/api-reference/sim-core.md`
  - SimulationRunner, PopulationHandler, WorkflowEngine, ResourceAllocator
- **sim-geo**: `docs/api-reference/sim-geo.md`
  - LocationHandler, MovementHandler, RoadNetwork, SpatialIndex
- **sim-events**: `docs/api-reference/sim-events.md`
  - Event, EventDispatcher, EventStorage
- **sim-telemetry**: `docs/api-reference/sim-telemetry.md`
  - TelemetryGenerator, TelemetryCollector, Export formats

#### Migration Documentation
- **Phase 6 Migration Guide**: `migration/guides/PHASE_6_MIGRATION_GUIDE.md`
  - Arrow migration step-by-step
  - Breaking changes and workarounds
  - Testing strategy
- **Phase 6 SQL Cookbook**: `migration/guides/PHASE_6_SQL_COOKBOOK.md`
  - 33 SQL query recipes
  - Performance tips
  - Common patterns

#### Examples
- **Kitchen Domain**:
  - Basic simulation: `docs/examples/kitchen/basic.md`
  - Multi-workflow: `docs/examples/kitchen/multi-workflow.md`
  - Inventory management: `docs/examples/kitchen/inventory-management.md`

---

### Benchmarks

#### Criterion Benchmarks
- **Workflow Performance**: `crates/sim-core/benches/workflow_performance.rs`
  - 8 benchmark groups (single update, sequential vs batch, status/progress/mixed updates, etc.)
  - Validates 7-785x speedup for batch operations
- **Batch Operations**: `crates/sim-core/benches/batch_operations.rs`
  - RecordBatch rebuild performance
  - Validates columnar storage efficiency
- **Routing Performance**: `crates/sim-geo/benches/routing_performance.rs`
  - Cached vs uncached routing queries
  - Validates 5,645x speedup with caching
- **Telemetry Performance**: `crates/sim-telemetry/benches/telemetry_performance.rs`
  - Log, trace, metric generation
  - Validates <1% overhead

**Running Benchmarks:**
```bash
# All benchmarks
cargo bench

# Specific benchmark file
cargo bench --bench workflow_performance

# Specific group
cargo bench --bench workflow_performance -- sequential_vs_batch

# Save baseline
cargo bench -- --save-baseline phase11

# Compare against baseline
cargo bench -- --baseline phase11
```

---

### Testing

#### Test Coverage
- **Total Tests**: 464+ tests passing across all crates
- **Pass Rate**: 100% for library tests
- **Coverage**: >85% code coverage per crate

#### Test Breakdown by Crate
| Crate | Tests | Status |
|-------|-------|--------|
| sim-core | 157 | ✅ 100% passing |
| sim-geo | 66 | ✅ 100% passing |
| sim-events | 16 | ✅ 100% passing |
| sim-telemetry | 104 | ✅ 100% passing |
| sim-kitchen | 112 | ✅ 100% passing |
| sim-logistics | 77 | ✅ 100% passing |
| **Total** | **532+** | **✅ 100% passing** |

#### Integration Tests
- **Workflow Integration**: 24-hour simulation with 3 concurrent workflows
- **Multi-Domain**: Kitchen + Logistics cross-domain validation
- **E2E Tests**: Complete lifecycle validation (customer order → delivery → completion)
- **Performance Tests**: Regression detection with baseline comparison

**Running Tests:**
```bash
# All tests
cargo test

# Specific crate
cargo test -p sim-core

# Integration tests only
cargo test --test integration_test

# With output
cargo test -- --nocapture

# Release mode (performance validation)
cargo test --release
```

---

### Dependencies

#### Core Dependencies
- **arrow**: 53.x - Apache Arrow columnar memory format
- **datafusion**: 42.x - SQL query engine
- **parquet**: 53.x - Columnar storage format
- **fast_paths**: 0.4.x - Contraction Hierarchies routing
- **h3o**: 0.6.x - H3 hexagonal hierarchical spatial indexing
- **chrono**: 0.4.x - Date and time library
- **geo**: 0.28.x - Geospatial data types and algorithms
- **serde**: 1.0.x - Serialization framework
- **anyhow**: 1.0.x - Error handling
- **uuid**: 1.x - Unique identifier generation

#### Development Dependencies
- **criterion**: 0.5.x - Statistical benchmarking
- **tokio**: 1.x - Async runtime (for tests)
- **rand**: 0.8.x - Random number generation
- **rand_distr**: 0.4.x - Statistical distributions (Poisson, exponential)

---

### Security

#### Known Issues
- None currently

#### Recommendations
- **Snapshot files**: Treat as sensitive data (may contain simulation state)
- **Event logs**: May contain domain-specific information, secure accordingly
- **SQL queries**: Validate untrusted input before execution
- **File paths**: Use absolute paths, validate user-provided paths

---

### Deprecations

None in 0.0.1 (initial release).

---

### Contributors

**Phase Execution:**
- Phase 1-2: Framework extraction and stabilization
- Phase 3: Code simplification (3 parallel agents)
- Phase 4: Documentation
- Phase 5: Workflow integration
- Phase 6: Arrow migration (3-week effort, 3 sub-phases)
- Phase 7-8: Road networks + Telemetry (parallel execution)
- Phase 9: Multi-workflow (11 agents: 2 parallel + 3 sequential + 1 E2E)
- Phase 10: Demand generation
- Phase 11: Logistics domain (framework generality proof)

**Acknowledgments:**
- Apache Arrow community for columnar storage
- DataFusion team for SQL query engine
- fast_paths library for Contraction Hierarchies
- H3 (Uber) for hexagonal spatial indexing
- OpenTelemetry for telemetry standards

---

### Migration Path

For users migrating from previous versions or the original universe crate:

#### From universe crate (pre-framework)
1. **Read** `migration/guides/PHASE_6_MIGRATION_GUIDE.md`
2. **Update** to RecordBatch-based handlers (dual-index pattern)
3. **Add** batch operation APIs to your code
4. **Regenerate** snapshots (old snapshots incompatible)
5. **Test** with comprehensive test suite
6. **Benchmark** to validate performance improvements

#### Phase 6 Breaking Changes (Task Schema)
1. **Update** Task trait implementations to provide `estimated_duration()` method
2. **Regenerate** snapshots with new 10-column task schema
3. **Update** SQL queries if accessing `allocated_at_millis` or `estimated_duration_secs`
4. **Test** snapshot save/restore roundtrip

See migration guides in `migration/guides/` for detailed step-by-step instructions.

---

### Future Roadmap

#### Phase 12-14 (Planned)
- **Python Bindings**: PyO3 bindings for Python integration
- **CLI Tool**: Command-line interface for running simulations
- **Comprehensive Benchmarks**: Expanded benchmark suite for regression detection
- **Deployment Guides**: Production deployment documentation

#### Post-1.0 (Ideas)
- **Distributed Simulation**: Multi-node simulation support
- **Real-Time Visualization**: Web dashboard for live monitoring
- **Machine Learning Integration**: RL agent support
- **GPU Acceleration**: CUDA/OpenCL for large-scale simulations
- **Cloud Integration**: AWS/GCP/Azure deployment templates

---

### Links

- **Repository**: [Internal link to repository]
- **Documentation**: `docs/`
- **Migration Guides**: `migration/`
- **API Reference**: `docs/api-reference/`
- **Examples**: `docs/examples/`

---

### Notes

#### Performance Notes
- All performance metrics measured on [specify hardware]
- Benchmarks run with Criterion default settings (statistical significance)
- Production performance may vary based on domain complexity and data volume

#### API Stability
- **0.0.1**: Initial release, expect API changes
- **1.0.0**: Stable API guarantee (Semantic Versioning)
- Breaking changes documented with migration guides

#### Support
- **Community**: [Link to community forum/chat]
- **Issues**: [Link to issue tracker]
- **Commercial Support**: [Contact information]

---

**Release Date**: March 16, 2026
**Project Status**: Production-ready foundation (71% complete)
**Next Milestone**: Version 1.0.0 (Phases 12-14)

---

[Unreleased]: https://github.com/yourorg/sim-framework/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/yourorg/sim-framework/releases/tag/v0.0.1
