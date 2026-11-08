# Architecture

Tributary runs agent-based simulations and writes out the events and synthetic
artifacts they produce. `sim-core` and the crates around it are generic.
Domain crates implement them: `sim-kitchen` is the reference, and
`sim-logistics` checks that the framework is not kitchen-specific.

## Crate layering

| Crate | Workspace dependencies | Role |
|---|---|---|
| `sim-core` | `sim-telemetry` (optional, `telemetry` feature) | Runner, agents, resources, workflows, demand, clock, snapshots |
| `sim-events` | `sim-core` | Typed events, dispatcher, Parquet persistence |
| `sim-artifacts` | none | Generators turn CloudEvents JSON into artifacts; JSONL storage |
| `sim-telemetry` | none | Log, trace and metric records from events |
| `sim-geo` | `sim-core` | Road network, H3 locations, routing (see limitations) |
| `sim-validate` | `sim-events` | Invariant checks over a run's events and artifacts |
| `sim-kitchen` | `sim-core`, `sim-events`, `sim-artifacts` | Ghost kitchen domain |
| `sim-logistics` | `sim-core`, `sim-events` | Package delivery domain |
| `caspers-cli` | `sim-core`, `sim-kitchen`, `sim-events`, `sim-validate` | `caspers` binary: TUI run, `validate` |
| `caspers-python` | `sim-kitchen`, `sim-logistics` | PyO3 bindings |

Domains depend on `sim-core`; `sim-core` depends on no other `sim-*` crate
except the optional `sim-telemetry`. `sim-events` depends on `sim-core` (for
`EventId` and `Error`), so it sits above `sim-core`, and `sim-telemetry`
cannot depend on `sim-events` without a cycle. `sim-geo` is a
dev-dependency of `sim-core` only, and no product crate uses it.

## Runner tick loop

`SimulationRunner::tick` runs these steps in order, calling the matching
`SimulationHooks` callback after each.

1. `on_before_tick`
2. Demand generation, then `on_demands_generated`
3. Agent ticks, then `on_agents_ticked`
4. Completion of tasks whose duration has elapsed. Capacity is released, then
   `on_tasks_completed`. Workflow instances advance and emit `WorkflowStepEvent`s.
5. `TaskFactory` creates a task for each instance ready for its next step
6. Pending tasks are allocated with the `AllocationStrategy`, then `on_resources_allocated`
7. Clock advances, then `on_after_tick`

`on_start` and `on_end` run only from the `run_*` methods. Domain wrappers call
`runner.tick()` and then do their own work (starting workflow instances, for
example). Callers must use the wrapper, not the runner, or that work is skipped.

## Plugging in a domain

The kitchen supplies these pieces. The logistics domain has the same shape.

- **Agents** (`Customer`) tick and return demands. The kitchen turns
  `food_order` demands into orders in `on_agents_ticked`.
- **Resources** (`Kitchen`) have type `kitchen:<name>`. Capacity is the number
  of orders the kitchen works on at once.
- **Tasks** (`PrepTask`) come from a `TaskFactory`. Each carries its
  `workflow_instance_id`, which links it back to its instance.
- **Workflows** (`FoodPrepWorkflow`): prep, cook and package. Each order is one
  instance, and all of its steps run in that order's kitchen.
- **Registry**: one named engine (`FOOD_PREP_ENGINE`) holds every definition.
  The domain calls `start_instance` on it.
- **Hooks** (`KitchenSimulationHooks`) create and track orders and emit events.
  The wrapper reads hook state through `runner.hooks_as_mut`.
- **Wrapper** (`KitchenSimulation`) owns the runner and builds it in
  `new_seeded`. `LogisticsSimulation` uses `PackageShipmentWorkflow` (five
  steps), `Station` resources, `Package` agents and a Poisson demand generator.

## Events and artifacts

1. Hooks emit typed `sim_events::Event`s into an `EventDispatcher`. It buffers
   them, counts them by type, and passes them to any registered
   `EventProcessor`. The kitchen registers none.
2. `KitchenSimulation::step` gives the events it has not yet seen to an
   `ArtifactCollectorProcessor` as CloudEvents JSON. Each event needs an
   RFC 3339 `time`. Domain `ArtifactGenerator`s turn events into artifacts.
3. `KitchenSimulation::run_for_steps` and `run_controlled_for` flush artifacts to storage. With
   `with_artifact_dir`, `JsonlArtifactStorage` writes one JSONL file per family
   (`web_events/`, `emails/`, `sms/`, `api_calls/`, ...). Without storage,
   artifacts are generated and counted but not written.
4. `save_events(path)` writes buffered events to Parquet and clears them.
5. `caspers validate` checks a finished run with `sim-validate`.

## Determinism

- `sim-core` does not read `SimulationConfig::seed`. A domain derives a
  sub-seed per component with `derive_seed(global, label)` (FNV-1a) and passes
  it to a seeded constructor, such as `PoissonDemandGenerator::with_seed`,
  `SpatialDemandGenerator::with_seed` or `Random::with_seed`.
- Simulation timestamps come from `ctx.current_time()`. Wall-clock reads are the
  `Live` and `Catchup` modes, the default start of `SimulationRunner::new` and
  `SimulationContext::from_now`, and the creation timestamps in
  `SimulationManager` and `SnapshotMetadata`, which are catalog data.
  Use `with_start_time` for a fixed start.
- Order is fixed where it matters. The allocator keys tasks by insertion
  sequence in a `BTreeMap`, the registry is a `BTreeMap` by name, and agents
  and instances are processed in row order.
- `new()` ID constructors use UUID v7 and differ between runs. Seeded kitchen
  customers take IDs from `from_name` (UUID v5), and their decisions depend only
  on `(rng_seed, step)`, so a seeded kitchen run repeats its decisions.
- Per-step draws make runs restore-safe. Kitchen customers, Poisson demand
  and logistics routing seed each step from `derive_seed(seed, "<label>/<step>")`
  rather than carrying RNG state between steps.

## Snapshots

- `save_snapshot` captures agents, resources, tasks, all workflow instances
  (concatenated across engines) and the clock.
- Backends: `Memory` (DataFusion `MemTable`) or `Parquet`, which writes one file
  per table under `<base>/<snapshot_id>/`.
- Restore is all-or-nothing: every component is staged and validated before the
  runner changes. The runner needs the same components and workflow definitions
  as when the snapshot was taken.
- Domain hook state is captured through `SimulationHooks::snapshot_state` and
  handed back to `restore_state` (`None` for a snapshot without it, which
  resets the hooks). It is stored as one more table, `hook_state`; the
  `hook_state_batch` / `hook_state_from_batch` helpers encode any serde type
  as a one-row JSON batch. The kitchen saves its in-flight orders and routing
  load, logistics its shipments, tracking-number counter, metrics and task
  priorities, so a restored seeded run emits the same events as an
  uninterrupted one.
- Random state is not captured. `PoissonDemandGenerator` and logistics package
  routing derive each step's RNG from `(seed, step)`, so they need none.
  `SpatialDemandGenerator` and the `Random` allocation strategy keep a running
  RNG, which a restored run continues from its current state.

## Known limitations

- **Batch rebuilds.** `PopulationHandler` and `WorkflowInstanceTable` rebuild
  their whole Arrow batch on each update. `tick_agents` does this every tick,
  so the cost follows total population, not rows changed ([#29](https://github.com/open-lakehouse/tributary/issues/29)).
- **Finished instances stay.** `complete_instance` marks an instance complete,
  and nothing removes it. The instance table, snapshots and SQL results grow
  with each run ([#29](https://github.com/open-lakehouse/tributary/issues/29)).
- **Journeys are not consumed.** `AgentTickResult::new_journeys` has no reader,
  so journey requests have no effect. `sim-geo` is not on any product path ([#30](https://github.com/open-lakehouse/tributary/issues/30)).
