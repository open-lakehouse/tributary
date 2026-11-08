# sim-core

Domain-agnostic runtime for agent-based simulations. It provides agents,
capacity-limited resources, multi-step workflows, demand generators, a clock,
snapshots, and a SQL view of the state. It defines no domain types.

[`sim-kitchen`](../sim-kitchen) is the reference domain. For signatures and
semantics, run `cargo doc -p sim-core --open`.

## Main types

| Module | Types |
|---|---|
| `population` | `Agent`, `AgentStatus`, `AgentTickResult`, `Demand`, `PopulationHandler<A>` |
| `resources` | `Resource`, `Task`, `ResourceAllocator<R, T>`, `TaskFactory<T>`, `AllocationStrategy`, `LeastLoaded`, `FirstFit`, `RoundRobin`, `Random` |
| `workflows` | `Workflow`, `WorkflowStep`, `WorkflowEngine<W>`, `WorkflowRegistry<W>`, `WorkflowStepEvent` |
| `demand` | `DemandGenerator`, `PoissonDemandGenerator`, `SpatialDemandGenerator`, `TimeOfDayDemandGenerator`, `CompositeDemandGenerator` |
| `context` | `SimulationConfig`, `SimulationContext`, `TimeMode`, `TickResult`, `derive_seed` |
| `runner` | `SimulationRunner<A, R, T, W>`, `SimulationHooks`, `NoOpHooks`, `SimulationStats`, control channel |
| `snapshot` | `SnapshotManager`, `SnapshotBackend`, `SnapshotData`, `SnapshotMetadata`, `hook_state_batch`, `hook_state_from_batch` |
| `manager` | `SimulationManager`, a catalog of runs and snapshots |

## Components

- **Agents.** `PopulationHandler` stores agents as Arrow rows. Each tick it
  decodes every agent, calls `Agent::tick`, and writes the rows back.
- **Resources and tasks.** A task names the resource type it needs. Pending
  tasks are assigned in priority order, then insertion order. The
  `AllocationStrategy` picks among resources of that type with free capacity.
  `LeastLoaded` is the default.
- **Workflows.** Each registry entry is a named `WorkflowEngine`. Domain code
  starts instances on an engine. When a task belonging to an instance
  completes, the instance advances, and `TaskFactory` creates the next task.
- **Demand.** A generator runs once per tick. Its demands go to
  `on_demands_generated`; the runner does not act on them.
- **Time.** `TimeMode::Range` (the default) advances by `time_step` per tick.
  `Live` and `Catchup` follow the wall clock.

## Wiring a runner

```rust,ignore
use sim_core::resources::{LeastLoaded, ResourceAllocator};
use sim_core::{
    PoissonDemandGenerator, PopulationHandler, SimulationConfig, SimulationRunner,
    WorkflowEngine, WorkflowRegistry,
};

// MyAgent: Agent, MyResource: Resource, MyTask: Task, MyWorkflow: Workflow,
// MyTaskFactory: TaskFactory<MyTask>, MyHooks: SimulationHooks. Not shown.
let config = SimulationConfig::default(); // 60 s steps, no seed
let mut runner: SimulationRunner<MyAgent, MyResource, MyTask, MyWorkflow> =
    SimulationRunner::new(config); // start time is the wall clock; see with_start_time
let mut population = PopulationHandler::new();
population.add_agents(agents)?; // Vec<MyAgent>
runner.set_population_handler(population);
let mut resources = ResourceAllocator::new();
resources.add_resources(machines)?; // Vec<MyResource>
runner.set_resource_allocator(resources);
let mut engine = WorkflowEngine::new();
engine.register_workflow(my_workflow);
let mut registry = WorkflowRegistry::new();
registry.register("pipeline".to_string(), engine)?;
runner.set_workflow_registry(registry);
runner.set_task_factory(Box::new(MyTaskFactory));
runner.set_demand_generator(Box::new(PoissonDemandGenerator::with_seed(10.0, "order", 42)));
runner.set_allocation_strategy(Box::new(LeastLoaded)); // the default
runner.set_hooks(Box::new(MyHooks::new()));
runner.run_for_steps(100)?; // on_start, 100 ticks, on_end
```

## Tick order

`SimulationRunner::tick` runs these steps in order:

1. `on_before_tick`
2. Demand generation, then `on_demands_generated`
3. Agent ticks, then `on_agents_ticked`
4. Completion of allocated tasks whose duration has elapsed (capacity is
   released, then `on_tasks_completed`). Their workflow instances advance, and
   each `WorkflowStepEvent` goes to `on_workflow_step_completed`.
5. A task from `TaskFactory` for each instance ready for its next step
6. Allocation of pending tasks, then `on_resources_allocated`
7. Clock advance, then `on_after_tick`

Workflow steps need a resource allocator; step 5 also needs a task factory.
`on_start` and `on_end` run only from `run_for_steps`, `run_until`,
`run_for_duration` and `run_controlled_for`, not from a bare `tick`.

## Snapshots

`save_snapshot(name)` and `restore_snapshot(name)` capture and restore agents,
resources, tasks, workflow instances, the clock and domain hook state. Call `init_snapshot_manager()`
(in-memory) or `set_snapshot_backend(SnapshotBackend::Parquet(dir))` first.

```rust,ignore
runner.init_snapshot_manager().await?;
runner.save_snapshot("checkpoint").await?;
runner.restore_snapshot("checkpoint").await?;
```

Restore needs the same components and workflow definitions as the saved run.

Hooks that keep state across ticks implement `SimulationHooks::snapshot_state`
and `restore_state`; `snapshot::hook_state_batch` and `hook_state_from_batch`
turn any serde type into the one-row batch they exchange. Restore calls
`restore_state` after the rest of the snapshot validates, with `None` for a
snapshot that has no hook state.

Random-number generator state is not captured. `PoissonDemandGenerator` draws
each step from `(seed, step)`, so it needs none; `SpatialDemandGenerator` and
the `Random` strategy continue from their current state.

## SQL

`runner.query(sql).await` runs DataFusion SQL over the current state. The
`agents`, `resources`, `tasks` and `workflow_instances` tables exist when the
matching component is set.

## Feature: `telemetry`

Off by default. It compiles `sim_core::runner::telemetry`
(`SimulationTelemetry`, `TelemetryHooks`), which turns simulation events into
`sim-telemetry` records. Two integration tests need it: run
`cargo test -p sim-core --all-features` to include them, as CI does.
