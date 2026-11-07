# Changelog

All notable changes to Tributary will be documented in this file. Nothing has
been released since 0.0.1, so everything below `[Unreleased]` is grouped by the
date the work landed.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Documentation: drop legacy docs
- Removed the concept pages, guides, `docs/api-reference/`, `docs/examples/`,
  `docs/decisions/`, `GETTING_STARTED.md`, `API_REFERENCE_INDEX.md`, the docs
  index, the per-crate `docs/` folders and the per-crate `CHANGELOG.md` files.
  They described APIs from before the migration. Rustdoc is the API reference;
  this file is the only changelog.
- `docs/ARCHITECTURE.md` and the sim-core, sim-events, sim-geo and
  sim-telemetry READMEs are rewritten against the current code.
- Dropped the changelog history from before the move to the `sim-*` crates.

### Documentation: remove pre-migration descriptions
- `DEPLOYMENT.md` describes what exists: the CLI, its outputs, Python,
  observability and `RUST_LOG`. It used to document a TOML config file,
  `SIM_*` environment variables, a `sim-runner` binary, a metrics port and a
  REST API, none of which exist.
- The sim-geo API reference documents the real `MovementHandler` and
  `VelocityModel` (it described `start_journey`/`tick`/`JourneyRequest` and a
  different trait), and the spatial concepts page uses real names
  (`movement::ConstantVelocity`, `TimeOfDayVelocity::new(..).with_utc_offset`,
  `save_to_file`/`load_from_file`).
- The sim-events reference shows the real `Event` API and marks its payloads
  as examples (sim-events ships none). sim-core's architecture page shows the
  registry-based runner and the actual tick order.
- Removed the uncompiled `docs/examples/custom/*-template.rs` files, which no
  longer matched the APIs; the domain crates are the templates.
- AGENTS.md: correct `move_towards` signature and test count.

### Changed: workspace hygiene
- sim-core's `runner::telemetry` (and its `sim-telemetry` dependency) is
  behind a new `telemetry` feature. Nothing in the products used it, but every
  build compiled it.
- sim-core no longer dev-depends on sim-kitchen: the population test that used
  the kitchen `Customer` moved to `sim-kitchen/tests`. The telemetry demo
  test no longer writes files into `/tmp`.
- Dropped the unused `async`/`object_store` features of `parquet`.

### Fixed: caspers-cli
- `caspers run --no-tui` exits non-zero when the simulation fails, its thread
  panics, or `--events-out` can't be written. It used to print the summary and
  exit 0.
- The TUI clock uses the simulation's real step (30 s). It assumed 60 s, so
  displayed times ran at double speed.
- `RUST_LOG` is honoured (the filter was hardcoded).
- A single OpenTelemetry tracer provider is installed and shut down on exit.
  A second, never-flushed provider used to be installed globally.
- Falling back to an in-memory catalog when `.caspers` can't be opened no
  longer panics (it called `block_on` inside the async runtime).
- `--events-out` also works with the TUI. New `--artifacts-out <DIR>`
  (default `output/artifacts`).

### Fixed (breaking): sim-geo
- `MovementHandler::advance_journeys(now, time_step)` takes the simulation
  time. It used to reconstruct "now" from the journey start and an assumed
  5 m/s, so time-of-day speeds were evaluated at the wrong time.
- `TimeOfDayVelocity::with_utc_offset(hours)` evaluates rush hours and night
  time in a local time zone (default UTC, as before); `is_rush_hour` /
  `is_night_time` accept any time zone.
- The cached routing graph is rebuilt after `add_node`/`add_edge` (new edges
  were ignored after the first query); `invalidate_route_cache()` covers
  direct edits of the public fields. Routing to a node with no edges returns
  `None` instead of panicking inside `fast_paths`.
- The module doctest no longer writes `network.json` into the working
  directory (the stray `crates/sim-geo/network.json` is gone), and the
  rush-hour integration test no longer depends on `HashMap` order.

### Fixed (breaking): sim-artifacts
- `ParquetArtifactStorage` is now `JsonlArtifactStorage`; it always wrote
  JSON Lines. Repeated flushes in a run append instead of overwriting the
  previous flush, and a failed write keeps the buffered artifacts instead of
  dropping them.
- `ArtifactCollectorProcessor::process` returns generator errors (they were
  discarded) and rejects events without a valid `time` (they were stamped
  1970). It takes `&mut self`, which removes an `unsafe` borrow split and the
  internal `Arc<Mutex>`; `set_storage` attaches storage after construction.
  The `LocalEventProcessor` trait is gone.

### Fixed: sim-logistics delivers packages
- Shipments never left their first step: only sorting stations were
  registered, but the workflow's five steps need receivers, sorting stations,
  docks, drivers and confirmers. All five are now `Station` resources (20
  drivers, one package each), and packages are delivered.
- One `PackageShipmentWorkflow` definition with an instance per package,
  instead of a new workflow definition registered for every package.
- Every stage emits its own event (`received`, `sorted`, `loaded`,
  `in_transit`, `delivered`) with the real station and driver names. Before,
  every step emitted `sorted` with a placeholder station, and `delivered` had
  `driver_id: "unknown"`.
- Driver utilization measures drivers carrying a package (it was pinned at
  100%) and `drivers_used` is filled in. Delivery time is measured from
  package creation for manual packages too (it used the wrong clock and
  `.abs()`), and manual and generated packages share one tracking-number
  sequence.
- Packages are agents whose stage follows the workflow. Package priority
  orders allocation.
- `run_for_hours` derives steps from the 30 s step instead of assuming it.
- Removed the parallel models that duplicated allocator and workflow timing
  and had no production caller: the `Driver` agent, `SortingStation` and
  `LoadingDock` (with hardcoded 30 s ticks), and the old `Package` state
  machine. `build_logistics_runner*` are gone; `LogisticsSimulation` is the
  entry point.

### Changed (breaking): sim-kitchen runs orders through workflows
- Orders are `FoodPrepWorkflow` instances (prep 2 min → cook 5 min → package
  1 min) driven by the `WorkflowRegistry`, with `PrepTask` steps allocated to
  the order's kitchen. Before, the workflow was never registered and orders
  "completed" on a hardcoded 5-minute timer regardless of kitchen work.
- Orders are routed to the kitchen with the fewest open orders; later steps
  outrank new orders, so a kitchen never has more orders in progress than its
  capacity (extra orders queue). Kitchen IDs are derived from their names.
- All events go through one dispatcher, so `customer.eating`/`customer.done`
  now reach the artifact generators. Events carry the fields the generators
  read: `order.created` has `total_price`, `order.completed` has `kitchen_id`,
  `customer.done` has `order_id`, `total_price` and a real
  `total_time_seconds` (it was always 0). Order and customer events use the
  order/customer ID as subject consistently.
- Seeded runs reproduce the event stream exactly, in order: seeded customers
  get IDs derived from their seed and order IDs are `order-<customer>-<step>`.
  Profiles no longer stamp `account_created_at` with the wall clock.
- Artifacts are only written when you opt in with
  `KitchenSimulation::with_artifact_dir`; the CLI and example write to
  `output/artifacts` and `<output_dir>/artifacts`, and the Python binding to
  `<output_dir>/artifacts`. Writing artifacts can now fail a run instead of
  printing a warning.
- `save_events` no longer makes the next step panic.
- `KitchenSimulation::event_records` replaces `collect_all_event_records`;
  `hooks()` exposes the hooks. `build_simulation`, `add_customers(&mut runner)`
  and the other free functions are gone (the raw runner never delivered food).
- Removed code that only ran in tests: inventory and supply chain, couriers,
  supply drivers, distribution sites, recipes, `KitchenStation`, the courier
  and supply-chain workflows, their events and artifact generators, and the
  `Order` task (replaced by `PrepTask`). About 6k lines; recover them from git
  history when they are wired into the simulation.

### Changed (breaking): sim-events
- `Event::new(payload, time)` takes the event time. It used to stamp the wall
  clock, and `with_time` (now removed) was easy to forget: sim-logistics
  events carried wall-clock times. Pass `ctx.current_time()`.
- Removed `ParquetEventProcessor`. Its `flush` needed `&mut self`, which a
  processor owned by the dispatcher can never get, and it lost buffered events
  on a write error. Write `dispatcher.to_records()` with `ParquetEventWriter`
  instead.

### Fixed: sim-core allocation, tick and snapshot correctness
- Tasks without a workflow instance now complete and release their resource.
  Previously they held capacity forever. Completing or removing a task releases
  capacity exactly once (completed tasks are removed), so `available` can no
  longer exceed `capacity`.
- `AllocationStrategy` is honoured. The allocator used to ignore it and always
  pick the first fitting resource. Pending tasks are allocated highest priority
  first.
- `SimulationRunner::tick()` now allocates pending tasks and calls
  `on_resources_allocated` itself, matching the documented tick order. A failed
  `add_task` for a workflow step is now an error instead of leaving the
  instance stuck.
- `restore_snapshot` restores workflow instances (routed to the engine that
  owns each workflow) and the simulation clock, and is all-or-nothing. Before,
  instances were silently dropped and the clock was left where it was.
- Workflow engines iterate in name order, so task and event order no longer
  vary between processes.
- Snapshot metadata carries the simulation time instead of the wall clock.
- An agent that fails to decode is now an error instead of being skipped every
  tick. Unreadable simulation/snapshot catalog files are reported instead of
  ignored (their timestamps used to fall back to 1970).

### Changed (breaking): sim-core
- `AllocationStrategy` is no longer generic. It receives a `TaskCandidate` and
  `ResourceCandidate` views (the allocator never held the domain objects). New
  `FirstFit` strategy; `SimulationRunner::set_allocation_strategy` (default
  `LeastLoaded`). `allocate_resources()` takes no strategy.
- `ResourceAllocator` keeps state in plain collections (Arrow only at the
  snapshot boundary); mutations are O(1)/O(log n) instead of rebuilding a
  `RecordBatch`. `get_completed_tasks` and `complete_task` return
  `CompletedTask`; `remove_task`/`remove_resource` return `bool`;
  `assignments()` returns `Vec<Assignment>`; `export_batch()` returns `Result`;
  `deallocate_batch` is gone. New `resource_capacity`, `resource_available`,
  `allocated_task_count`. The task snapshot column `estimated_duration_secs` is
  now `estimated_duration_millis`, and the `completed` status is gone.
- New `SimulationHooks::on_tasks_completed` (default no-op). `Assignment`
  carries the task's `workflow_instance_id`.
- `SnapshotData` has a `clock: Option<SnapshotClock>` field.
- Removed: `WorkflowEngine::tick` and `WorkflowRegistry::tick_all` (no-ops),
  `SimulationRunner::reset` (only reset the clock), `PopulationHandler::get_agent_mut`
  (returned a copy; use `get_agent` + `update_agent`), `SnapshotMetadata::new`
  (stamped wall-clock time; use `with_ids`), and sim-kitchen's `process_demands`
  (an unused duplicate that allocated against wall-clock time).

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

