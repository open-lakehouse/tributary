# AI Agent Instructions for Chef Casper's Management System

> **Purpose**: Essential context and guidelines for AI coding assistants working in this simulation codebase.

## Quick Reference

| Task | Command | Notes |
|------|---------|-------|
| Code generation | `just generate` | After any `.proto` changes |
| Run simulation (TUI) | `just run [duration]` | Default: 100 steps; opens live TUI |
| Run sim-kitchen | `just sim-kitchen [steps]` | Default: 100 steps, writes artifacts |
| Python dev | `just build-py` | Build Python bindings |
| Notebooks | `just scratch` | Launch Marimo notebook |
| Inspect events | `just inspect` | Open sim_events.py Marimo notebook |
| Format/lint | `just fmt` | Runs pre-commit hooks + justfile format |

## Project Identity

**What this is**: A high-performance agent-based simulation system for generating realistic operational data for a fictional ghost kitchen management company (project name: Tributary).

**What this is NOT**: A production restaurant management system.

### Core Technologies Stack
- **Rust** (Primary): Simulation engine, CLI, TUI — Rust edition 2024, MSRV 1.90
- **Apache Arrow/DataFusion**: Columnar data processing (arrow 57, datafusion 51)
- **Protocol Buffers**: Schema definitions and type generation (prost 0.13, buf toolchain)
- **GeoArrow / H3**: Geospatial operations (H3 hexagonal indexing, geo 0.31, h3o 0.9)
- **Python**: Analysis tools and bindings (via PyO3/Maturin, requires Python 3.11+)
- **CloudEvents**: Event infrastructure (CloudEvents 1.0 spec)
- **OpenTelemetry**: Observability (opentelemetry 0.31)

---

## Architecture Overview

### Crate Structure

```
management/
├── crates/
│   ├── sim-core/          # Generic agent-based simulation framework (domain-agnostic)
│   ├── sim-events/        # CloudEvents 1.0 event infrastructure
│   ├── sim-artifacts/     # Artifact generation from events (emails, API calls, web events, etc.)
│   ├── sim-geo/           # Geospatial abstractions (H3, road networks, routing)
│   ├── sim-kitchen/       # Ghost kitchen domain — reference implementation
│   ├── sim-logistics/     # Package delivery domain — validates framework generality
│   ├── sim-telemetry/     # OpenTelemetry-compatible telemetry generation
│   ├── universe/          # Legacy simulation engine (Arrow/DataFusion, Python bindings)
│   └── cli/               # Binary (caspers) — TUI runner (no server)
├── proto/                 # Protocol buffer schemas (buf toolchain)
├── python/                # Python bindings (PyO3/Maturin) for universe crate
├── notebooks/             # Marimo/Jupyter analysis notebooks
├── ui/                    # Frontend UI
├── docs/                  # Documentation site (ARCHITECTURE.md lives here)
└── output/                # Simulation output (artifacts written here; gitignored)
```

### The sim-* Framework

The `sim-*` crates form a layered, domain-agnostic framework:

```
sim-core        ← Core abstractions: agents, resources, workflows, runner, snapshots
sim-events      ← CloudEvents emission and Parquet persistence
sim-artifacts   ← Artifact generation: converts events into domain artifacts
sim-geo         ← Geospatial: H3, road network routing, journeys
sim-telemetry   ← OTel-compatible telemetry from simulation events
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

// Resources
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
move_towards(location, target, speed, step_secs) -> Point
```

### sim-artifacts Public API

```rust
// Core trait — implement this to generate artifacts from events
ArtifactGenerator<P>   // P = domain profiles (e.g. customer data, pricing tables)
ArtifactContext        // passed to each generator: event, sim_time, profiles, run_id

// Collection & processing
ArtifactCollector<P>              // accumulates artifacts from registered generators
ArtifactCollectorProcessor<P>     // LocalEventProcessor adapter; call flush_to_storage() at run end

// Artifact types
Artifact, ApiCallArtifact, EmailArtifact, SmsArtifact,
PushArtifact, WebEventArtifact, DataArtifact, DocumentArtifact

// Storage
ArtifactStorage, ParquetArtifactStorage, NoOpStorage
```

Output is written to `output/artifacts/` by default.

### SimulationRunner

The runner is the central orchestrator. Build it with the builder pattern:

```rust
SimulationRunner::new(config)
    .with_population(PopulationHandler::new())
    .with_resources(ResourceAllocator::new())
    .set_workflow_registry(registry)      // WorkflowRegistry is the ONLY workflow path
    .set_task_factory(factory)
    .set_demand_generator(generator)
    .set_hooks(MyHooks::new())
    .set_snapshot_manager(manager)
```

Each simulation step (via `runner.tick()` or `runner.run_for_steps(n)`):
1. `on_before_tick()`
2. Tick all agents
3. `on_agents_ticked()` — domain hook: process demands, emit events
4. Tick workflow registry — advance time-based steps, create/complete tasks
5. Allocate tasks to resources
6. `on_resources_allocated()` — domain hook: track assignments
7. `on_after_tick()` — domain hook: check completions

### SimulationHooks Trait

```rust
pub trait SimulationHooks: Send + Sync + 'static {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_before_tick(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()>;
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()>;
    fn on_resources_allocated(&mut self, ctx: &SimulationContext, assignments: &[Assignment]) -> Result<()>;
    fn on_workflow_step_completed(&mut self, ctx: &SimulationContext, event: &WorkflowStepEvent) -> Result<()>;
    fn on_demands_generated(&mut self, ctx: &SimulationContext, demands: &[Demand]) -> Result<()>;
}
```

### Data Flow

1. **Initialization**: Load configs → construct agents/resources/workflows → build runner
2. **Simulation Loop**: Runner ticks → hooks inject domain logic → events emitted via CloudEvents
3. **Artifacts**: `ArtifactCollectorProcessor` converts events → domain artifacts → Parquet files in `output/artifacts/`

---

## Domain Models

### sim-kitchen (Ghost Kitchen)

- **Customer agents**: State machine — `Idle → Ordering → WaitingForFood → Eating → Done`
- **Kitchen resources**: Capacity-constrained, auto-tracked load via `ResourceAllocator`
- **Order tasks**: Flow through kitchen pipeline
- **Events**: `io.kitchen.order.{created,assigned,completed}`, `io.kitchen.customer.{eating,done}`
- **Artifacts**: payment authorizations, order confirmations, web events, inventory reports
- **Entry points**: `KitchenSimulation::new(num_kitchens, capacity, verbose)` or `build_simulation()`
- **Run methods**: `run_for_steps(n)` (no channel) or `run_controlled_for(max)` (control channel, all modes)

### sim-logistics (Package Delivery)

- **Package agents**: Passive — represent packages being processed
- **Driver agents**: Active — transport packages between locations
- **Resources**: `SortingStation` (capacity), `LoadingDock` (vehicle loading)
- **Workflows**: `PackageShipmentWorkflow` — 5-step pipeline via `WorkflowRegistry`
- **Locations**: `Warehouse`, `DeliveryAddress` (H3-indexed)
- **Entry point**: `LogisticsSimulation::new(config)`

### universe (Legacy)

The `universe` crate is the **legacy** simulation engine. It is not being extended — new domain work should use the `sim-*` framework. It continues to run alongside the new framework but will eventually be replaced.

---

## Git Commits

GPG commit signing is required on this repository. The GPG PIN prompt requires an interactive terminal, which AI agents cannot provide. **Never attempt `git commit` directly** — it will time out and fail.

### Correct workflow

1. Run `cargo clippy --workspace --fix --allow-dirty` to apply automatic lint fixes (may change code)
2. Run `cargo fmt --all` to format all code (including anything clippy rewrote)
3. **Update documentation** — before staging, check and update any affected docs:
   - `docs/CHANGELOG.md` — add an entry under `[Unreleased]` for every user-visible change
   - `docs/api-reference/` — update API signatures for any changed public interfaces
   - `CLAUDE.md` — update if commands, rules, or public APIs documented here have changed
   - `README.md` — update if project-level usage or entry points changed
4. Stage files with `git add <specific files>` — do this programmatically (via Bash tool)
5. Output a ready-to-paste `git commit` command for the user to run in their terminal

Steps 1 and 2 must always run in this order before staging — clippy may rewrite code that then needs formatting, and the pre-commit hook enforces `cargo fmt`. Documentation updates (step 3) must happen before staging so all changes land in the same commit. Step 4 (staging) must be done by the agent so the user only needs to paste and run a single commit command.

### Template

Use an inline `-m` string — **not** the `$(cat <<'EOF' ...)` heredoc pattern, which fails in this environment:

```bash
git commit -m "<type>: <short summary>

<body explaining what and why>

Co-authored-by: Isaac"
```

The `Co-authored-by: Isaac` trailer must be included on every commit.

### Commit message conventions

- **Types**: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`
- Subject line ≤ 72 characters, imperative mood ("add", not "added")
- Body explains *what* changed and *why*
- List affected crates when changes span multiple packages

---

## Critical Rules

### NEVER DO THIS
1. **Edit generated code** in `crates/universe/src/models/gen/` — changes will be overwritten
2. **Skip `just generate`** after modifying `.proto` files — will cause compilation errors
3. **Use non-workspace dependencies** — must be defined in root `Cargo.toml` `[workspace.dependencies]`
4. **Commit `.caspers/` or `output/` directories** — runtime data only, gitignored
5. **Commit `.claude/worktrees/` or `.claude/projects/`** — agent working state, gitignored
6. **Mix time zones** — all timestamps are UTC, use `ctx.current_time()` for simulation time
7. **Use `set_workflow_engine()`** — removed; use `set_workflow_registry()` only
8. **Hardcode step duration as `30.0`** — use `ctx.time_step().num_seconds() as f64`
9. **Run `git commit` directly** — GPG signing requires interactive TTY; prepare the command for the user instead
10. **Stage files before running clippy and fmt** — always run clippy then fmt first

### ALWAYS DO THIS
1. **Run `just generate`** after proto changes
2. **Use `WorkflowRegistry`** when wiring workflows in `SimulationRunner`
3. **Use `ctx.time_step()`** for step duration in agent tick logic
4. **Use `ctx.current_time()`** for simulation timestamps — never `Utc::now()` in simulation code
5. **Use `sim_core::move_towards()`** for movement — don't duplicate in domain crates
6. **Add tests** in `*_tests.rs` files alongside implementation

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

### Modifying Data Schema (universe / legacy path)

1. Edit `.proto` files in `proto/`
2. Run `just generate` (regenerates Rust code via buf)
3. Update builders in `crates/universe/src/builders/`
4. Update state management in `crates/universe/src/state/`
5. Test with `cargo test`

### Python Integration

- Build bindings: `just build-py`
- Python code in `python/` exposes the legacy `universe` crate to Python
- Use for analysis, NOT for core simulation logic
- Marimo notebooks in `notebooks/` for interactive exploration (`just scratch`, `just inspect`)

---

## Testing

```bash
cargo test                          # All tests
cargo test -p sim-core              # Package-specific
cargo test -p sim-kitchen           # Ghost kitchen tests
cargo test -p sim-logistics         # Logistics tests
cargo test kitchen                  # Pattern match

# Sim-kitchen integration (writes artifact output)
just sim-kitchen 500
```

The workspace has 700+ tests. Run `cargo test --workspace` before committing.

---

## Running Simulations

```bash
# Run CLI simulation (new sim-kitchen framework, shows live TUI)
just run 500       # or: cargo run --bin caspers -- run --duration 500

# Custom parameters
cargo run --bin caspers -- run --duration 200 --num-kitchens 5 --num-customers 30

# Sim-kitchen library example — writes artifacts to output/artifacts/ (no TUI)
just sim-kitchen 100

# With tracing (requires Docker — starts Jaeger)
just run-tracing
# Then visit http://localhost:16686
```

---

## Troubleshooting

### "Generated code out of sync"
```bash
just generate && cargo clean && cargo build
```

### Clippy warnings in sim-* crates
```bash
cargo clippy -p sim-core -p sim-kitchen -p sim-logistics -p sim-geo -p sim-events -p sim-artifacts -p sim-telemetry -- -D warnings
```
The legacy `universe` crate has pre-existing warnings that are out of scope.

### Python bindings fail to build
```bash
uv pip install maturin[zig] && just build-py
```

---

## Related Documentation

- `README.md` — Entity relationships and legacy Python system
- `docs/ARCHITECTURE.md` — High-level design decisions
- `justfile` — All available commands and build tasks
- `Cargo.toml` — Workspace dependencies and versions

## File Conventions

| Pattern | Usage |
|---------|-------|
| `*.proto` | Protocol buffer definitions |
| `*_tests.rs` | Unit/integration tests |
| `gen/*.rs` | Generated code — DO NOT EDIT |
| `output/artifacts/` | Simulation artifact output (Parquet) |

---

**Last Updated**: 2026-03-21
**Simulation Version**: 0.0.1
**Rust Edition**: 2024 / MSRV 1.90
