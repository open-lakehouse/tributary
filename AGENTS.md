# Agent Instructions

Instructions for AI coding agents working in this repository. This is the single
canonical file: `CLAUDE.md` imports it, and other agents read it directly. Human
contributor detail lives in [`CONTRIBUTING.md`](CONTRIBUTING.md); design records
live in [`docs/`](docs/).

## Quick Reference

| Task | Command | Notes |
|------|---------|-------|
| Run simulation (TUI) | `just run [duration]` | Default: 100 steps; opens live TUI |
| Run sim-kitchen | `just sim-kitchen [steps]` | Default: 100 steps, writes artifacts |
| Python dev | `just build-py` | Build Python bindings |
| Notebooks | `just scratch` | Launch Marimo notebook |
| Inspect events | `just inspect` | Open sim_events.py Marimo notebook |
| Kitchen notebook | `just kitchen` | Run + analyze in one Marimo notebook |
| Format/lint | `just fmt` | Runs pre-commit hooks + justfile format |
| Auto-fix | `just fix` | `clippy --fix`, then `cargo fmt` |
| Pre-push gate | `just check` | Everything CI gates on: lint, test, doc |
| MSRV check | `just msrv` | `cargo +1.95 check` on the declared floor |

## Project Identity

**What this is**: A high-performance agent-based simulation system for generating realistic operational data for a fictional ghost kitchen management company (project name: Tributary).

**What this is NOT**: A production restaurant management system.

### Core Technologies Stack
- **Rust** (Primary): Simulation engine, CLI, TUI. Edition, MSRV (`rust-version`) and dependency versions live in the workspace `Cargo.toml`; the dev toolchain is pinned in `rust-toolchain.toml`
- **Apache Arrow/DataFusion**: Columnar data processing
- **GeoArrow / H3**: Geospatial operations (H3 hexagonal indexing via h3o, geo)
- **Python**: Analysis tools and bindings (via PyO3/Maturin, requires Python 3.11+)
- **CloudEvents**: Event infrastructure (CloudEvents 1.0 spec)
- **OpenTelemetry**: Observability

---

## Architecture Overview

### Crate Structure

```
tributary/
├── crates/
│   ├── sim-core/          # Generic agent-based simulation framework (domain-agnostic)
│   ├── sim-events/        # CloudEvents 1.0 event infrastructure
│   ├── sim-artifacts/     # Artifact generation from events (emails, API calls, web events, etc.)
│   ├── sim-geo/           # Geospatial abstractions (H3, road networks, routing)
│   ├── sim-kitchen/       # Ghost kitchen domain — reference implementation
│   ├── sim-logistics/     # Package delivery domain — validates framework generality
│   ├── sim-telemetry/     # OpenTelemetry-compatible telemetry generation
│   ├── sim-validate/      # Output-validation harness (invariants over emitted events)
│   └── cli/               # Binary (caspers) — TUI runner (no server)
├── python/                # Python bindings (PyO3/Maturin) — wraps sim-kitchen & sim-logistics
├── notebooks/             # Marimo/Jupyter analysis notebooks
├── docs/                  # Documentation (ARCHITECTURE.md lives here)
└── output/                # Simulation output (artifacts written here; gitignored)
```

### The sim-* Framework

The `sim-*` crates form a layered, domain-agnostic framework:

```
sim-core        ← Core abstractions: agents, resources, workflows, runner, snapshots
sim-events      ← CloudEvents emission and Parquet persistence (depends on sim-core)
sim-artifacts   ← Artifact generation: converts events into domain artifacts
sim-geo         ← Geospatial: H3, road network routing, journeys
sim-telemetry   ← OTel-compatible telemetry from simulation events
sim-validate    ← Output validation: invariant rules over emitted events and artifacts
sim-kitchen     ← Ghost kitchen domain (reference impl, uses all above)
sim-logistics   ← Package delivery domain (validates framework generality)
```

**Key principle**: `sim-kitchen` and `sim-logistics` use the framework with zero modifications to `sim-core`/`sim-events` — domain logic lives entirely in the domain crates.

### sim-core Public API

```rust
// Context & Config
SimulationConfig, SimulationContext

// Agents & Population
Agent, AgentStatus, Demand, PopulationHandler

// Resources (strategies: LeastLoaded, FirstFit, RoundRobin, Random in sim_core::resources)
Resource, ResourceAllocator, Task, AllocationStrategy

// Workflows (registry is the only path — no single-engine mode)
Workflow, WorkflowEngine, WorkflowRegistry, WorkflowStep, WorkflowStepEvent

// Runner
SimulationRunner, SimulationHooks, NoOpHooks, SimulationStats

// Demand Generation
DemandGenerator, CompositeDemandGenerator, PoissonDemandGenerator,
SpatialDemandGenerator, TimeOfDayDemandGenerator

// Snapshots
SnapshotBackend, SnapshotData, SnapshotManager, SnapshotMetadata

// Utilities
AgentId, LocationId, ResourceId, TaskId, WorkflowId
move_towards(&mut location, target, speed_m_s, step_secs) -> bool  // true once arrived
```

### sim-artifacts Public API

```rust
// Core trait — implement this to generate artifacts from events
ArtifactGenerator<P>   // P = domain profiles (e.g. customer data, pricing tables)
ArtifactContext        // passed to each generator: event, sim_time, profiles, run_id

// Collection & processing
ArtifactCollector<P>              // accumulates artifacts from registered generators
ArtifactCollectorProcessor<P>     // process(&event_json) per event; flush_to_storage() to write

// Artifact types
Artifact, ApiCallArtifact, EmailArtifact, SmsArtifact,
PushArtifact, WebEventArtifact, DataArtifact, DocumentArtifact

// Storage
ArtifactStorage, JsonlArtifactStorage, NoOpStorage
```

Artifacts are written only where a storage is attached (for the kitchen:
`KitchenSimulation::with_artifact_dir(dir)`; the CLI uses `output/artifacts/`).

### SimulationRunner

The runner is the central orchestrator. Create it, then wire components with its setters:

```rust
let mut runner = SimulationRunner::new(config);
runner.set_population_handler(PopulationHandler::new());
runner.set_resource_allocator(ResourceAllocator::new());
runner.set_workflow_registry(registry);   // WorkflowRegistry is the ONLY workflow path
runner.set_task_factory(Box::new(factory));
runner.set_demand_generator(Box::new(generator));
runner.set_allocation_strategy(Box::new(LeastLoaded)); // optional; LeastLoaded is the default
runner.set_hooks(Box::new(MyHooks::new()));
```

Each simulation step (via `runner.tick()` or `runner.run_for_steps(n)`):
1. `on_before_tick()`
2. Generate demand, then `on_demands_generated()`
3. Tick all agents, then `on_agents_ticked()` — domain hook: process demands, emit events
4. Complete tasks whose duration has elapsed (releasing capacity), then
   `on_tasks_completed()`; workflow tasks advance their instance and fire
   `on_workflow_step_completed()`
5. Create a task (via the `TaskFactory`) for each workflow instance ready for its next step
6. Allocate pending tasks with the runner's strategy, then `on_resources_allocated()`
7. Advance time, then `on_after_tick()`

`runner.allocate_resources()` repeats step 6 on demand, for tasks a domain
wrapper adds between ticks.

### SimulationHooks Trait

```rust
pub trait SimulationHooks: Send + Sync + 'static {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_before_tick(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()>;
    fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) -> Result<()>;
    fn on_tasks_completed(&mut self, ctx: &SimulationContext, completed: &[CompletedTask]) -> Result<()>;
    fn on_workflow_step_completed(&mut self, ctx: &SimulationContext, event: &WorkflowStepEvent) -> Result<()>;
    fn on_demands_generated(&mut self, ctx: &SimulationContext, demands: &[Demand]) -> Result<()>;
}
```

### Data Flow

1. **Initialization**: Load configs → construct agents/resources/workflows → build runner
2. **Simulation Loop**: Runner ticks → hooks inject domain logic → events emitted via CloudEvents
3. **Artifacts**: `ArtifactCollectorProcessor` converts events → domain artifacts → JSON Lines files (e.g. `output/artifacts/`)

---

## Domain Models

### sim-kitchen (Ghost Kitchen)

- **Customer agents**: State machine — `Idle → Ordering → WaitingForFood → Eating → Done → Idle`
- **Kitchen resources**: capacity = orders prepared at once; resource type `kitchen:<name>`
- **Orders**: one `FoodPrepWorkflow` instance per order (prep → cook → package, all in the
  order's kitchen) via `WorkflowRegistry`; steps are `PrepTask`s from `PrepTaskFactory`
- **Events**: `io.kitchen.order.{created,assigned,completed}`, `io.kitchen.customer.{eating,done}`
- **Artifacts**: web sessions, confirmation email/SMS, payment records, kitchen tickets, receipts
- **Entry point**: `KitchenSimulation::new(num_kitchens, capacity, verbose)` /
  `new_seeded(.., seed)`, then `.with_artifact_dir(dir)` (optional) and `add_customers(n)`
- **Run methods**: `step()`, `run_for_steps(n)`, or `run_controlled_for(max)` (control channel).
  Don't tick the runner directly: order start, delivery and `customer.done` happen in `step()`

### sim-logistics (Package Delivery)

- **Package agents**: passive; stage follows the workflow (`created → received → sorted → in_transit → delivered`)
- **Station resources**: receivers, sorting stations, loading docks, drivers (capacity 1), delivery confirmers
- **Workflows**: `PackageShipmentWorkflow` — 5-step pipeline via `WorkflowRegistry`, one instance per package
- **Locations**: `Warehouse`, `DeliveryAddress` (H3-indexed)
- **Entry point**: `LogisticsSimulation::new()` / `new_seeded(seed)`; drive with `step()` or `run_for_hours(h)`

---

## Commits, PRs and stacks

- Conventional commits: `<type>(<scope>): <subject>`, types `feat fix refactor
  docs test chore` (plus `perf build ci`). Scope is the crate's package name
  (`sim-core`, `sim-kitchen`, `caspers-cli`, …), or omit it for workspace-wide
  changes.
- **PRs are squash-merged; the PR title is the commit that lands on `main`.**
  CI lints it (`pr-title.yml`). Commits inside a PR are for review only.
- **One coherent change = one PR.** Use a [`gh stack`](https://github.com/github/gh-stack)
  (one layer per `type(scope)`) when work should land as separate commits.
- **Never commit to `main`.** Stage files by name; never `git add -A` / `git add .`.
- Commits are GPG-signed. Agents commit unsigned (`git commit --no-gpg-sign`), push and
  open the PR, then hand over a single sign + force-push command (see the
  maintainer's machine-wide conventions).
- Before staging, update affected docs in the same commit:
  `docs/CHANGELOG.md` (`[Unreleased]`, every user-visible change), rustdoc
  and the crate `README.md` (changed public interfaces), `docs/ARCHITECTURE.md`
  (design changes), this file (commands, rules, public APIs), the root
  `README.md` (entry points).

### Pre-push check (mirrors CI)

```bash
just fix     # cargo clippy --fix, then cargo fmt (in that order: clippy may rewrite code)
just check   # fmt check, clippy -D warnings, machete, ruff, typos, nextest, doctests, rustdoc
```

---

## Critical Rules

### NEVER DO THIS
1. **Use non-workspace dependencies** — must be defined in root `Cargo.toml` `[workspace.dependencies]`
2. **Commit `.caspers/` or `output/` directories** — runtime data only, gitignored
3. **Commit `.claude/worktrees/` or `.claude/projects/`** — agent working state, gitignored
4. **Mix time zones** — all timestamps are UTC, use `ctx.current_time()` for simulation time
5. **Hardcode step duration as `30.0`** — use `ctx.time_step().num_seconds() as f64`
6. **Stage files before running clippy and fmt**: run `just fix` (clippy, then fmt) first

### ALWAYS DO THIS
1. **Use `WorkflowRegistry`** when wiring workflows in `SimulationRunner`
2. **Use `ctx.time_step()`** for step duration in agent tick logic
3. **Use `ctx.current_time()`** for simulation timestamps; never `Utc::now()` in simulation code. Wall-clock reads belong only at the edges: the Live/Catchup anchor and default start time in `sim-core` (context, `SimulationRunner::new`), and catalog/snapshot metadata in `manager`/`snapshot`
4. **Use `sim_core::move_towards()`** for movement — don't duplicate in domain crates
5. **Add tests** in `*_tests.rs` files alongside implementation

---

## Development Workflows

### Adding a New Domain Simulation

Model after `sim-kitchen` (reference implementation):

1. Create a new crate: `crates/sim-<domain>/`
2. Add to workspace in root `Cargo.toml`
3. Define agents implementing `sim_core::Agent`
4. Define resources implementing `sim_core::Resource`
5. Define tasks implementing `sim_core::Task`
6. Define workflows implementing `sim_core::Workflow` (registered via `WorkflowRegistry`)
7. Implement `SimulationHooks` for domain-specific logic
8. Create a top-level `<Domain>Simulation` wrapper struct
9. Add integration tests

### Adding a New Agent (within a domain crate)

1. Create `src/agents/my_agent.rs`
2. Implement `sim_core::Agent` (requires `tick(&mut self, ctx: &SimulationContext)`)
3. Use `ctx.time_step().num_seconds() as f64` for step duration
4. Use `sim_core::move_towards()` for position updates
5. Register in `src/agents/mod.rs`
6. Add corresponding tests in `my_agent_tests.rs`

### Python Integration

- Build bindings: `just build-py`
- Python code in `python/` exposes `sim-kitchen` and `sim-logistics` to Python
  (`run_kitchen_simulation`, `run_logistics_simulation`); importable as `caspers`
- Use for analysis, NOT for core simulation logic
- Marimo notebooks in `notebooks/` for interactive exploration (`just scratch`, `just inspect`, `just kitchen`)

---

## Testing

```bash
just test                           # All tests, as in CI (nextest + doctests)
cargo test                          # All tests
cargo test -p sim-core              # Package-specific
cargo test -p sim-kitchen           # Ghost kitchen tests
cargo test -p sim-logistics         # Logistics tests
cargo test kitchen                  # Pattern match

# Sim-kitchen integration (writes artifact output)
just sim-kitchen 500
```

The workspace has about 650 tests. Run `just check` before pushing.

---

## Running Simulations

```bash
# Run CLI simulation (new sim-kitchen framework, shows live TUI)
just run 500       # or: cargo run --bin caspers -- run --duration 500

# Custom parameters
cargo run --bin caspers -- run --duration 200 --num-kitchens 5 --num-customers 30

# Sim-kitchen library example — writes events and artifacts under .caspers/sim-kitchen (no TUI)
just sim-kitchen 100

# With tracing (requires Docker — starts Jaeger)
just run-tracing
# Then visit http://localhost:16686
```

---

## Troubleshooting

### Clippy must stay clean
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
The whole workspace is warning-clean; keep it that way before committing.

### Local build fails with "failed to select a version" for a fresh crate
The maintainer's machine-level Cargo config routes crates.io through an
age-filtering proxy that hides versions published in the last 7 days. CI builds
against crates.io and is unaffected. Wait for the proxy to catch up, or use the
`/crates-proxy` skill. Never commit a `[patch]` or a lock resolved under one.

### Builds and disk
Dev builds keep line tables for workspace crates and no debug info for
dependencies (`[profile.dev]` in `Cargo.toml`), so panic locations and backtraces
still work. For a debugger use `cargo build --profile debugging`. Run
`cargo clean` in a worktree before removing it.

### Python bindings fail to build
```bash
uv pip install maturin[zig] && just build-py
```

---

## Related Documentation

- `README.md` — Project overview and crate list
- `docs/ARCHITECTURE.md` — Crate layering, tick loop, known limitations
- `justfile` — All available commands and build tasks
- `Cargo.toml` — Workspace dependencies and versions (every external dependency is declared here)
- `CONTRIBUTING.md` — Setup and the pre-push gate

## File Conventions

| Pattern | Usage |
|---------|-------|
| `*_tests.rs` | Unit/integration tests |
| `output/artifacts/` | Simulation artifact output (JSON Lines) |
