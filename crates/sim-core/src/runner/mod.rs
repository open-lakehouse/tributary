//! Simulation orchestration
//!
//! The runner coordinates all simulation components through a tick-based loop:
//! - Agent population management
//! - Resource allocation
//! - Workflow execution
//! - Time advancement
//! - Telemetry generation (optional)
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_core::{SimulationRunner, SimulationConfig};
//!
//! let config = SimulationConfig::default();
//! let mut runner = SimulationRunner::new(config);
//!
//! // Register components
//! runner.set_population_handler(population);
//! runner.set_resource_allocator(resources);
//!
//! // Run simulation
//! runner.run_for_steps(100)?;
//! ```

pub mod control;
mod queries;
pub mod telemetry;

pub use control::{ControlCommand, SimEvent, SimulationController, control_channel};
pub use queries::query_simulation;

use crate::Result;
use crate::context::{SimulationConfig, SimulationContext, TickResult};
use crate::demand::DemandGenerator;
use crate::population::{Agent, AgentTickResult, Demand, PopulationHandler};
use crate::resources::{
    AllocationStrategy, Assignment, CompletedTask, LeastLoaded, Resource, ResourceAllocator, Task,
    TaskFactory,
};
use crate::snapshot::{
    SnapshotBackend, SnapshotClock, SnapshotData, SnapshotManager, SnapshotMetadata,
};
use crate::workflows::{Workflow, WorkflowRegistry, WorkflowStepEvent};
use arrow::array::RecordBatch;
pub use control::ControlReceiver;
use std::marker::PhantomData;

/// Lifecycle hooks for simulation events
///
/// Implement this trait to receive callbacks at key points in the simulation.
pub trait SimulationHooks: Send + Sync + 'static {
    /// Called before the simulation starts
    fn on_start(&mut self, _ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    /// Called before each tick
    fn on_before_tick(&mut self, _ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    /// Called after each tick
    fn on_after_tick(&mut self, _ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    /// Called when the simulation ends
    fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
        Ok(())
    }

    /// Called when agents complete their ticks
    fn on_agents_ticked(
        &mut self,
        _ctx: &SimulationContext,
        _results: &[AgentTickResult],
    ) -> Result<()> {
        Ok(())
    }

    /// Called when resources are allocated
    fn on_resources_allocated(
        &mut self,
        _ctx: &SimulationContext,
        _assignments: &[Assignment],
    ) -> Result<()> {
        Ok(())
    }

    /// Called with the tasks that finished this tick (capacity already released)
    fn on_tasks_completed(
        &mut self,
        _ctx: &SimulationContext,
        _completed: &[CompletedTask],
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a workflow step completes
    fn on_workflow_step_completed(
        &mut self,
        _ctx: &SimulationContext,
        _event: &WorkflowStepEvent,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when demands are generated
    fn on_demands_generated(
        &mut self,
        _ctx: &SimulationContext,
        _demands: &[Demand],
    ) -> Result<()> {
        Ok(())
    }

    /// Downcast to concrete type for domain-specific access
    fn as_any(&self) -> &dyn std::any::Any;

    /// Downcast to concrete type for domain-specific mutation
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Default implementation that does nothing
#[derive(Debug, Default)]
pub struct NoOpHooks;

impl SimulationHooks for NoOpHooks {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Simulation runner statistics
#[derive(Debug, Clone, Default)]
pub struct SimulationStats {
    /// Total number of steps executed
    pub total_steps: usize,
    /// Total agent ticks processed
    pub total_agent_ticks: usize,
    /// Total resource assignments made
    pub total_assignments: usize,
    /// Total workflow steps completed
    pub total_workflow_steps: usize,
}

/// Main simulation orchestrator
///
/// The runner coordinates all simulation components through a tick-based loop.
/// It manages the lifecycle of agents, resources, workflows, and time progression.
///
/// # Type Parameters
///
/// - `A` - Agent type implementing the Agent trait
/// - `R` - Resource type implementing the Resource trait
/// - `T` - Task type implementing the Task trait
/// - `W` - Workflow type implementing the Workflow trait
pub struct SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    /// Simulation context (time, config, etc.)
    context: SimulationContext,
    /// Population handler (optional)
    population: Option<PopulationHandler<A>>,
    /// Resource allocator (optional)
    resources: Option<ResourceAllocator<R, T>>,
    /// Workflow registry (multi-workflow support)
    workflow_registry: Option<WorkflowRegistry<W>>,
    /// Task factory for creating tasks from workflow steps (optional)
    task_factory: Option<Box<dyn TaskFactory<T>>>,
    /// Strategy used to allocate pending tasks each tick
    allocation_strategy: Box<dyn AllocationStrategy>,
    /// Demand generator (optional)
    demand_generator: Option<Box<dyn DemandGenerator>>,
    /// Lifecycle hooks
    hooks: Box<dyn SimulationHooks>,
    /// Statistics
    stats: SimulationStats,
    /// Snapshot manager for state persistence
    snapshot_manager: Option<SnapshotManager>,
    /// Optional control channel receiver (attached via `attach_control`)
    control: Option<ControlReceiver>,
    /// Whether the simulation is currently in the paused state
    paused: bool,
    /// Type markers
    _phantom: PhantomData<(A, R, T, W)>,
}

impl<A, R, T, W> SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    /// Create a new simulation runner
    ///
    /// # Arguments
    ///
    /// * `config` - Simulation configuration
    ///
    /// # Returns
    ///
    /// A new SimulationRunner instance
    ///
    /// # Note
    ///
    /// The snapshot manager is not initialized by default. Call `init_snapshot_manager()`
    /// or `set_snapshot_backend()` to enable snapshot functionality.
    pub fn new(config: SimulationConfig) -> Self {
        Self::with_start_time(chrono::Utc::now(), config)
    }

    /// Create a runner with a specific start time
    pub fn with_start_time(
        start_time: chrono::DateTime<chrono::Utc>,
        config: SimulationConfig,
    ) -> Self {
        Self {
            context: SimulationContext::new(start_time, config),
            population: None,
            resources: None,
            workflow_registry: None,
            task_factory: None,
            allocation_strategy: Box::new(LeastLoaded),
            demand_generator: None,
            hooks: Box::new(NoOpHooks),
            stats: SimulationStats::default(),
            snapshot_manager: None,
            control: None,
            paused: false,
            _phantom: PhantomData,
        }
    }

    /// Set the population handler
    pub fn set_population_handler(&mut self, handler: PopulationHandler<A>) {
        self.population = Some(handler);
    }

    /// Set the resource allocator
    pub fn set_resource_allocator(&mut self, allocator: ResourceAllocator<R, T>) {
        self.resources = Some(allocator);
    }

    /// Set the task factory for creating tasks from workflow steps
    pub fn set_task_factory(&mut self, factory: Box<dyn TaskFactory<T>>) {
        self.task_factory = Some(factory);
    }

    /// Set the strategy used to allocate pending tasks each tick
    ///
    /// Defaults to [`LeastLoaded`].
    pub fn set_allocation_strategy(&mut self, strategy: Box<dyn AllocationStrategy>) {
        self.allocation_strategy = strategy;
    }

    /// Set the demand generator
    pub fn set_demand_generator(&mut self, generator: Box<dyn DemandGenerator>) {
        self.demand_generator = Some(generator);
    }

    /// Set lifecycle hooks
    pub fn set_hooks(&mut self, hooks: Box<dyn SimulationHooks>) {
        self.hooks = hooks;
    }

    /// Downcast hooks to a concrete type for domain-specific access
    pub fn hooks_as<H: SimulationHooks>(&self) -> Option<&H> {
        self.hooks.as_any().downcast_ref::<H>()
    }

    /// Downcast hooks to a concrete mutable type for domain-specific access
    pub fn hooks_as_mut<H: SimulationHooks>(&mut self) -> Option<&mut H> {
        self.hooks.as_any_mut().downcast_mut::<H>()
    }

    /// Get the current simulation context
    pub fn context(&self) -> &SimulationContext {
        &self.context
    }

    /// Get mutable reference to context
    pub fn context_mut(&mut self) -> &mut SimulationContext {
        &mut self.context
    }

    /// Get the population handler
    pub fn population(&self) -> Option<&PopulationHandler<A>> {
        self.population.as_ref()
    }

    /// Get mutable reference to population handler
    pub fn population_mut(&mut self) -> Option<&mut PopulationHandler<A>> {
        self.population.as_mut()
    }

    /// Get the resource allocator
    pub fn resources(&self) -> Option<&ResourceAllocator<R, T>> {
        self.resources.as_ref()
    }

    /// Get mutable reference to resource allocator
    pub fn resources_mut(&mut self) -> Option<&mut ResourceAllocator<R, T>> {
        self.resources.as_mut()
    }

    /// Set the workflow registry for multi-workflow support
    ///
    /// # Arguments
    ///
    /// * `registry` - WorkflowRegistry instance
    ///
    /// # Note
    ///
    /// Each named engine in the registry handles a distinct workflow type.
    /// Use `WorkflowRegistry::register` to add engines before calling this method.
    pub fn set_workflow_registry(&mut self, registry: WorkflowRegistry<W>) {
        self.workflow_registry = Some(registry);
    }

    /// Get the workflow registry
    pub fn workflow_registry(&self) -> Option<&WorkflowRegistry<W>> {
        self.workflow_registry.as_ref()
    }

    /// Get mutable reference to workflow registry
    pub fn workflow_registry_mut(&mut self) -> Option<&mut WorkflowRegistry<W>> {
        self.workflow_registry.as_mut()
    }

    /// Get simulation statistics
    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    /// Execute a single simulation tick
    ///
    /// Each tick runs, in order:
    ///
    /// 1. [`SimulationHooks::on_before_tick`]
    /// 2. Demand generation, then [`SimulationHooks::on_demands_generated`]
    /// 3. Agent ticks, then [`SimulationHooks::on_agents_ticked`]
    /// 4. Task completion: allocated tasks whose duration has elapsed release
    ///    their resource ([`SimulationHooks::on_tasks_completed`]); tasks that
    ///    belong to a workflow advance that instance, and each resulting
    ///    [`WorkflowStepEvent`] goes to
    ///    [`SimulationHooks::on_workflow_step_completed`]
    /// 5. Workflow instances ready for their next step get a task from the
    ///    [`TaskFactory`]
    /// 6. Pending tasks are allocated with the runner's
    ///    [`AllocationStrategy`], then
    ///    [`SimulationHooks::on_resources_allocated`]
    /// 7. Time advances, then [`SimulationHooks::on_after_tick`]
    ///
    /// # Returns
    ///
    /// `Ok(TickResult)` describing whether the simulation should continue,
    /// has reached its configured end time, or has transitioned from Catchup
    /// to Live mode.
    pub fn tick(&mut self) -> Result<TickResult> {
        self.hooks.on_before_tick(&self.context)?;

        if let Some(generator) = &self.demand_generator {
            let demands = generator.generate(&self.context);
            self.hooks.on_demands_generated(&self.context, &demands)?;
        }

        if let Some(population) = &mut self.population {
            let results = population.tick_agents(&self.context)?;
            self.stats.total_agent_ticks += results.len();
            self.hooks.on_agents_ticked(&self.context, &results)?;
        }

        let current_time = self.context.current_time();
        let step = i64::try_from(self.context.step()).unwrap_or(i64::MAX);

        if let Some(resources) = &mut self.resources {
            // Complete due tasks and advance their workflow instances.
            let completed = resources.get_completed_tasks(current_time)?;
            let mut workflow_events = Vec::new();
            for task in &completed {
                let Some(instance_id) = task.workflow_instance_id else {
                    continue;
                };
                let engine = self
                    .workflow_registry
                    .as_mut()
                    .and_then(|r| r.engine_for_instance_mut(instance_id))
                    .ok_or_else(|| {
                        crate::Error::Workflow(format!(
                            "Task {} belongs to unknown workflow instance {instance_id}",
                            task.task_id
                        ))
                    })?;
                engine.instance_table.set_current_tick(step);
                if let Some(event) = engine.advance_instance_step(instance_id, current_time)? {
                    workflow_events.push(event);
                    self.stats.total_workflow_steps += 1;
                }
            }
            if !completed.is_empty() {
                self.hooks.on_tasks_completed(&self.context, &completed)?;
            }
            for event in &workflow_events {
                self.hooks
                    .on_workflow_step_completed(&self.context, event)?;
            }

            // Create tasks for workflow instances waiting on their next step.
            if let (Some(registry), Some(task_factory)) =
                (&mut self.workflow_registry, &self.task_factory)
            {
                for engine in registry.engines_mut() {
                    engine.instance_table.set_current_tick(step);
                    for instance in engine.instance_table.get_ready_instances() {
                        let workflow_step = engine
                            .get_workflow(instance.workflow_id)
                            .and_then(|w| w.get_step(instance.current_step))
                            .ok_or_else(|| {
                                crate::Error::Workflow(format!(
                                    "Instance {} is at step {} of workflow {}, which does not exist",
                                    instance.id, instance.current_step, instance.workflow_id
                                ))
                            })?;
                        let task =
                            task_factory.create_task(instance.id, workflow_step, current_time);
                        resources.add_task(task)?;
                        engine.mark_task_assigned(instance.id)?;
                    }
                }
            }
        }

        self.allocate_resources()?;

        let tick_result = self.context.advance();
        self.stats.total_steps += 1;

        self.hooks.on_after_tick(&self.context)?;

        self.post_tick_sleep();

        Ok(tick_result)
    }

    /// Sleep for the duration requested by the context's time mode, if any.
    ///
    /// This is called automatically at the end of every `tick()`.  It is a
    /// no-op in `Range` and pre-transition `Catchup` modes.
    fn post_tick_sleep(&self) {
        if let Some(dur) = self.context.sleep_duration() {
            std::thread::sleep(dur);
        }
    }

    /// Set the time mode on the underlying context.
    ///
    /// Convenience wrapper so callers don't need to call `context_mut()`.
    pub fn set_time_mode(&mut self, mode: crate::context::TimeMode) {
        self.context.set_time_mode(mode);
    }

    /// Attach a [`ControlReceiver`] to this runner.
    ///
    /// Must be called before [`Self::run_controlled`].  Only one receiver can
    /// be attached at a time; calling this again replaces the previous one.
    pub fn attach_control(&mut self, rx: ControlReceiver) {
        self.control = Some(rx);
    }

    /// Public wrapper around `apply_control_commands` for use by domain wrappers.
    ///
    /// Returns `true` if a `Stop` command was received (caller should break).
    pub fn process_control_commands(&mut self) -> Result<bool> {
        self.apply_control_commands()
    }

    /// Returns whether the simulation is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set the paused state directly (used by domain wrappers).
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Emit a [`SimEvent`] via the attached control channel (no-op if none).
    pub fn emit_event(&self, event: SimEvent) {
        if let Some(c) = &self.control {
            c.emit(event);
        }
    }

    /// Drain pending [`ControlCommand`]s and apply them.
    ///
    /// Returns `true` if a `Stop` command was received (caller should break).
    fn apply_control_commands(&mut self) -> Result<bool> {
        let control = match &self.control {
            Some(c) => c,
            None => return Ok(false),
        };

        let commands = control.drain_commands();

        for cmd in commands {
            match cmd {
                ControlCommand::Stop => {
                    return Ok(true);
                }
                ControlCommand::Pause => {
                    if !self.paused {
                        self.paused = true;
                        let step = self.context.step();
                        if let Some(c) = &self.control {
                            c.emit(SimEvent::Paused { at_step: step });
                        }
                    }
                }
                ControlCommand::Resume => {
                    self.paused = false;
                }
                ControlCommand::StepN(n) => {
                    // Execute N steps regardless of paused state, then re-pause
                    for _ in 0..n {
                        let result = self.tick()?;
                        let step = self.context.step();
                        let sim_time = self.context.current_time();
                        if let Some(c) = &self.control {
                            c.emit(SimEvent::TickCompleted {
                                step,
                                sim_time,
                                event_counts: std::collections::HashMap::new(),
                            });
                            if result == TickResult::SwitchedToLive {
                                c.emit(SimEvent::ModeChanged {
                                    mode: "live".to_string(),
                                });
                            }
                        }
                        if result == TickResult::ReachedEnd {
                            return Ok(true); // treat as stop
                        }
                    }
                    // Re-pause after stepping
                    let step = self.context.step();
                    if let Some(c) = &self.control {
                        c.emit(SimEvent::Paused { at_step: step });
                    }
                }
                ControlCommand::SetMode(mode) => {
                    let mode_str = format!("{mode:?}");
                    self.context.set_time_mode(mode);
                    if let Some(c) = &self.control {
                        c.emit(SimEvent::ModeChanged { mode: mode_str });
                    }
                }
                ControlCommand::Start => {
                    // No-op: already running
                }
                ControlCommand::SetSpeed(speed) => {
                    let clamped = speed.clamp(0.125, 16.0);
                    self.context.live_speed = clamped;
                    if let Some(c) = &self.control {
                        c.emit(SimEvent::SpeedChanged {
                            multiplier: clamped,
                        });
                    }
                }
                ControlCommand::Snapshot(label) => {
                    let snapshot_id = uuid::Uuid::now_v7();
                    let step = self.context.step();
                    let sim_time = self.context.current_time();
                    let simulation_id = self.context.simulation_id();
                    match self.save_snapshot_with_ids_sync(
                        snapshot_id,
                        simulation_id,
                        &label,
                        sim_time,
                    ) {
                        Ok(_) => {
                            if let Some(c) = &self.control {
                                c.emit(SimEvent::SnapshotSaved {
                                    snapshot_id,
                                    label,
                                    sim_time,
                                    step,
                                });
                                c.emit(SimEvent::SnapshotsUpdated { snapshots: vec![] });
                            }
                        }
                        Err(e) => {
                            if let Some(c) = &self.control {
                                c.emit(SimEvent::Error(format!("Snapshot failed: {e}")));
                            }
                        }
                    }
                }
                ControlCommand::RestoreSnapshot(snapshot_id) => {
                    match tokio::runtime::Handle::current()
                        .block_on(self.restore_snapshot(&snapshot_id.to_string()))
                    {
                        Ok(_metadata) => {
                            self.paused = true;
                            let step = self.context.step();
                            let sim_time = self.context.current_time();
                            if let Some(c) = &self.control {
                                c.emit(SimEvent::SnapshotRestored {
                                    snapshot_id,
                                    label: None,
                                    sim_time,
                                    step,
                                });
                                c.emit(SimEvent::Paused { at_step: step });
                                c.emit(SimEvent::SnapshotsUpdated { snapshots: vec![] });
                            }
                        }
                        Err(e) => {
                            if let Some(c) = &self.control {
                                c.emit(SimEvent::Error(format!("Restore failed: {e}")));
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    /// Synchronously save a snapshot with explicit IDs by blocking on the async runtime.
    ///
    /// Safe to call from the synchronous tick loop because:
    /// - Commands are processed between ticks (no concurrent mutation)
    /// - The CLI runs under `#[tokio::main]`, so `Handle::current()` is available
    fn save_snapshot_with_ids_sync(
        &self,
        snapshot_id: uuid::Uuid,
        simulation_id: uuid::Uuid,
        label: &str,
        sim_time: chrono::DateTime<chrono::Utc>,
    ) -> crate::Result<SnapshotMetadata> {
        let label = label.to_string();
        tokio::runtime::Handle::current().block_on(async move {
            self.save_snapshot_with_id(snapshot_id, simulation_id, Some(&label), sim_time)
                .await
        })
    }

    /// Save a snapshot under a caller-supplied ID and record it in the
    /// manager's catalog
    pub async fn save_snapshot_with_id(
        &self,
        snapshot_id: uuid::Uuid,
        simulation_id: uuid::Uuid,
        label: Option<&str>,
        sim_time: chrono::DateTime<chrono::Utc>,
    ) -> crate::Result<SnapshotMetadata> {
        let manager = self.snapshot_manager()?;
        let data = self.capture_snapshot_data()?;
        manager
            .save_snapshot_with_id(snapshot_id, simulation_id, label, sim_time, data)
            .await
    }

    fn snapshot_manager(&self) -> crate::Result<&SnapshotManager> {
        self.snapshot_manager.as_ref().ok_or_else(|| {
            crate::Error::InvalidState("Snapshot manager not initialized".to_string())
        })
    }

    /// Collect the state of every component (and the clock) for a snapshot
    fn capture_snapshot_data(&self) -> crate::Result<SnapshotData> {
        let agents = match &self.population {
            Some(pop) => pop.export_batch(),
            None => RecordBatch::new_empty(crate::population::agent_schema()),
        };

        let (resources, tasks) = match &self.resources {
            Some(res) => res.export_batch()?,
            None => (
                RecordBatch::new_empty(crate::resources::resource_schema()),
                RecordBatch::new_empty(crate::resources::task_schema()),
            ),
        };

        let schema = crate::workflows::workflow_instance_schema();
        let batches: Vec<RecordBatch> = self
            .workflow_registry
            .iter()
            .flat_map(|registry| registry.engines())
            .map(|engine| engine.instance_table.export_batch())
            .collect();
        let workflow_instances = arrow::compute::concat_batches(&schema, &batches)?;

        Ok(SnapshotData {
            agents,
            resources,
            tasks,
            workflow_instances,
            locations: None,
            clock: Some(SnapshotClock {
                sim_time: self.context.current_time(),
                step: self.context.step(),
            }),
        })
    }

    /// Run the simulation driven by the attached control channel.
    ///
    /// The caller must have called [`Self::attach_control`] first.  The run
    /// loop continues until a [`ControlCommand::Stop`] is received, the time
    /// mode signals [`TickResult::ReachedEnd`], or an error occurs.
    ///
    /// This method calls `on_start` and `on_end` hooks internally.
    pub fn run_controlled(&mut self) -> Result<()> {
        self.run_controlled_for(usize::MAX)
    }

    /// Like [`Self::run_controlled`] but stops after at most `max_steps` ticks.
    ///
    /// Primarily useful for testing without an explicit Stop command.
    pub fn run_controlled_for(&mut self, max_steps: usize) -> Result<()> {
        self.hooks.on_start(&self.context)?;

        let mut steps_taken = 0;

        loop {
            if steps_taken >= max_steps {
                break;
            }

            // If paused, spin waiting for commands (low-CPU wait)
            if self.paused {
                std::thread::sleep(std::time::Duration::from_millis(10));
                if self.apply_control_commands()? {
                    break;
                }
                continue;
            }

            // Process any pending commands before the tick
            if self.apply_control_commands()? {
                break;
            }

            // Re-check pause (a command above may have paused us)
            if self.paused {
                continue;
            }

            // Execute one tick
            let result = self.tick()?;
            steps_taken += 1;

            let step = self.context.step();
            let sim_time = self.context.current_time();

            if let Some(c) = &self.control {
                c.emit(SimEvent::TickCompleted {
                    step,
                    sim_time,
                    event_counts: std::collections::HashMap::new(),
                });
                if result == TickResult::SwitchedToLive {
                    c.emit(SimEvent::ModeChanged {
                        mode: "live".to_string(),
                    });
                }
            }

            if result == TickResult::ReachedEnd {
                break;
            }
        }

        self.hooks.on_end(&self.context)?;

        let total_steps = self.context.step();
        let final_time = self.context.current_time();
        if let Some(c) = &self.control {
            c.emit(SimEvent::SimulationEnded {
                total_steps,
                final_time,
            });
        }

        Ok(())
    }

    /// Allocate pending tasks now, then call
    /// [`SimulationHooks::on_resources_allocated`]
    ///
    /// [`tick`](Self::tick) already does this every step. Call it directly
    /// only to allocate tasks a domain wrapper added between ticks without
    /// waiting for the next one.
    ///
    /// Returns the number of assignments made.
    pub fn allocate_resources(&mut self) -> Result<usize> {
        let Some(resources) = &mut self.resources else {
            return Ok(0);
        };
        let assignments = resources.assign_tasks(
            self.allocation_strategy.as_ref(),
            self.context.current_time(),
        )?;
        self.stats.total_assignments += assignments.len();
        self.hooks
            .on_resources_allocated(&self.context, &assignments)?;
        Ok(assignments.len())
    }

    /// Run the simulation for a specified number of steps
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of steps to execute
    ///
    /// # Returns
    ///
    /// Ok(()) if all steps completed successfully (may stop early if `ReachedEnd`)
    pub fn run_for_steps(&mut self, steps: usize) -> Result<()> {
        // Call start hook
        self.hooks.on_start(&self.context)?;

        // Execute ticks
        for _ in 0..steps {
            if self.tick()? == TickResult::ReachedEnd {
                break;
            }
        }

        // Call end hook
        self.hooks.on_end(&self.context)?;

        Ok(())
    }

    /// Run the simulation until a condition is met
    ///
    /// # Arguments
    ///
    /// * `condition` - Function that returns true when simulation should stop
    ///
    /// # Returns
    ///
    /// Ok(()) if simulation completed successfully
    pub fn run_until<F>(&mut self, mut condition: F) -> Result<()>
    where
        F: FnMut(&SimulationContext, &SimulationStats) -> bool,
    {
        // Call start hook
        self.hooks.on_start(&self.context)?;

        // Execute ticks until condition met or ReachedEnd
        while !condition(&self.context, &self.stats) {
            if self.tick()? == TickResult::ReachedEnd {
                break;
            }
        }

        // Call end hook
        self.hooks.on_end(&self.context)?;

        Ok(())
    }

    /// Run the simulation for a specified duration
    ///
    /// # Arguments
    ///
    /// * `duration` - Simulation time duration to run
    ///
    /// # Returns
    ///
    /// Ok(()) if simulation completed successfully
    pub fn run_for_duration(&mut self, duration: chrono::Duration) -> Result<()> {
        let end_time = self.context.current_time() + duration;
        self.run_until(|ctx, _stats| ctx.current_time() >= end_time)
    }

    /// Initialize the snapshot manager with Memory backend
    ///
    /// This is a convenience method to initialize snapshot functionality with
    /// the default Memory backend. For persistent storage, use `set_snapshot_backend()`.
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Error if initialization fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// runner.init_snapshot_manager().await?;
    /// ```
    pub async fn init_snapshot_manager(&mut self) -> crate::Result<()> {
        self.snapshot_manager = Some(SnapshotManager::new(SnapshotBackend::Memory).await?);
        Ok(())
    }

    /// Set the snapshot backend
    ///
    /// Configures where snapshots are stored. Use this to switch from the default
    /// Memory backend to persistent Parquet storage.
    ///
    /// # Arguments
    ///
    /// * `backend` - The storage backend to use (Memory or Parquet)
    ///
    /// # Returns
    ///
    /// Ok(()) on success, Error if initialization fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sim_core::snapshot::SnapshotBackend;
    ///
    /// runner.set_snapshot_backend(
    ///     SnapshotBackend::Parquet("./snapshots".to_string())
    /// ).await?;
    /// ```
    pub async fn set_snapshot_backend(&mut self, backend: SnapshotBackend) -> crate::Result<()> {
        self.snapshot_manager = Some(SnapshotManager::new(backend).await?);
        Ok(())
    }

    /// Save a snapshot of the current simulation state under `name`
    ///
    /// Captures agents, resources, tasks, workflow instances and the clock.
    /// Restore it with [`restore_snapshot`](Self::restore_snapshot) using the
    /// same name. The returned metadata carries `name` as its label.
    ///
    /// # Errors
    ///
    /// Returns an error if no snapshot manager is configured or the backend
    /// write fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// runner.run_for_steps(100)?;
    /// let metadata = runner.save_snapshot("checkpoint_100").await?;
    /// println!("Saved snapshot with {} agents", metadata.agent_count);
    /// ```
    pub async fn save_snapshot(&self, name: &str) -> crate::Result<SnapshotMetadata> {
        let manager = self.snapshot_manager()?;
        let data = self.capture_snapshot_data()?;
        let metadata = Self::describe(&data, &self.context, Some(name));
        manager.save_snapshot(name, data).await?;
        Ok(metadata)
    }

    fn describe(
        data: &SnapshotData,
        ctx: &SimulationContext,
        label: Option<&str>,
    ) -> SnapshotMetadata {
        SnapshotMetadata::with_ids(
            uuid::Uuid::now_v7(),
            ctx.simulation_id(),
            label.map(str::to_string),
            data.clock.map_or(ctx.current_time(), |c| c.sim_time),
            "1.0".to_string(),
            data.agents.num_rows(),
            data.resources.num_rows(),
            data.tasks.num_rows(),
            data.workflow_instances.num_rows(),
            data.locations.as_ref().map_or(0, |l| l.num_rows()),
        )
    }

    /// Restore simulation state from a snapshot
    ///
    /// Replaces agents, resources, tasks and workflow instances, and rewinds
    /// the clock if the snapshot captured it. Workflow instances are routed to
    /// the registered engine that owns their workflow definition, so the
    /// runner must have the same engines and workflows registered as when the
    /// snapshot was taken.
    ///
    /// Restore is all-or-nothing: everything is validated into staging copies
    /// first, and the runner is only modified once all of it succeeds.
    ///
    /// # Errors
    ///
    /// Returns an error if no snapshot manager is configured, the snapshot
    /// doesn't exist, a batch fails to import, a workflow instance belongs to
    /// a workflow no engine knows, or the snapshot holds data for a component
    /// this runner doesn't have.
    pub async fn restore_snapshot(&mut self, snapshot_id: &str) -> crate::Result<SnapshotMetadata> {
        let data = self
            .snapshot_manager()?
            .restore_snapshot(snapshot_id)
            .await?;
        let metadata = Self::describe(&data, &self.context, Some(snapshot_id));

        // Stage every component; nothing below mutates `self` until all succeed.
        let population = match &self.population {
            Some(_) => {
                let mut staged = PopulationHandler::<A>::new();
                staged.import_batch(data.agents.clone())?;
                Some(staged)
            }
            None if data.agents.num_rows() > 0 => {
                return Err(crate::Error::InvalidState(
                    "Snapshot has agents but the runner has no population handler".into(),
                ));
            }
            None => None,
        };

        let resources = match &self.resources {
            Some(_) => {
                let mut staged = ResourceAllocator::<R, T>::new();
                staged.import_batch(data.resources.clone(), data.tasks.clone())?;
                Some(staged)
            }
            None if data.resources.num_rows() + data.tasks.num_rows() > 0 => {
                return Err(crate::Error::InvalidState(
                    "Snapshot has resources or tasks but the runner has no resource allocator"
                        .into(),
                ));
            }
            None => None,
        };

        let instance_tables = self.stage_workflow_instances(&data.workflow_instances)?;

        if let Some(population) = population {
            self.population = Some(population);
        }
        if let Some(resources) = resources {
            self.resources = Some(resources);
        }
        if let (Some(registry), Some(tables)) = (&mut self.workflow_registry, instance_tables) {
            for (engine, table) in registry.engines_mut().zip(tables) {
                engine.instance_table = table;
            }
        }
        if let Some(clock) = data.clock {
            self.context.restore_clock(clock.sim_time, clock.step);
        }

        Ok(metadata)
    }

    /// Split a flat workflow-instance batch into one table per registered engine
    ///
    /// Returns tables in `engines()` order, or `None` if there is no registry.
    fn stage_workflow_instances(
        &self,
        batch: &RecordBatch,
    ) -> crate::Result<Option<Vec<crate::workflows::WorkflowInstanceTable>>> {
        use arrow::array::{Array, BinaryArray, BooleanArray};

        let Some(registry) = &self.workflow_registry else {
            if batch.num_rows() > 0 {
                return Err(crate::Error::InvalidState(
                    "Snapshot has workflow instances but the runner has no workflow registry"
                        .into(),
                ));
            }
            return Ok(None);
        };

        let workflow_ids = batch
            .column_by_name("workflow_id")
            .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
            .ok_or_else(|| {
                crate::Error::Workflow("workflow_instances batch has no workflow_id column".into())
            })?;
        let owners: Vec<usize> = (0..batch.num_rows())
            .map(|row| {
                let id = crate::WorkflowId::from_bytes(workflow_ids.value(row)).map_err(|e| {
                    crate::Error::Workflow(format!("Invalid workflow ID at row {row}: {e}"))
                })?;
                registry
                    .engines()
                    .position(|e| e.get_workflow(id).is_some())
                    .ok_or_else(|| {
                        crate::Error::Workflow(format!(
                            "Snapshot instance at row {row} uses workflow {id}, which no registered engine knows"
                        ))
                    })
            })
            .collect::<crate::Result<_>>()?;

        let mut tables = Vec::with_capacity(registry.count());
        for idx in 0..registry.count() {
            let mask: BooleanArray = owners.iter().map(|&o| Some(o == idx)).collect();
            let rows = arrow::compute::filter_record_batch(batch, &mask)?;
            let mut table = crate::workflows::WorkflowInstanceTable::new();
            table.import_batch(rows)?;
            tables.push(table);
        }
        Ok(Some(tables))
    }

    /// Execute SQL query across simulation entities
    ///
    /// Provides a SQL interface to query across multiple entity types (agents, resources,
    /// tasks, workflow instances) using DataFusion. This enables complex analytical queries
    /// with JOINs, aggregations, and filtering.
    ///
    /// # Available Tables
    ///
    /// - `agents` - Agent population (if PopulationHandler exists)
    /// - `resources` - Available resources (if ResourceAllocator exists)
    /// - `tasks` - Task queue (if ResourceAllocator exists)
    /// - `workflow_instances` - Workflow execution state (if WorkflowEngine exists)
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL query string (supports full DataFusion SQL syntax)
    ///
    /// # Returns
    ///
    /// Vector of RecordBatch results from the query
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - SQL is invalid or malformed
    /// - Query references non-existent tables
    /// - DataFusion execution fails
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Find agents with assigned tasks
    /// let result = runner.query("
    ///     SELECT a.agent_type, COUNT(*) as count
    ///     FROM agents a
    ///     JOIN tasks t ON a.id = t.assigned_agent_id
    ///     WHERE t.status = 'active'
    ///     GROUP BY a.agent_type
    /// ").await?;
    ///
    /// // Analyze resource utilization
    /// let result = runner.query("
    ///     SELECT
    ///         r.resource_type,
    ///         ROUND(100.0 * (r.capacity - r.available) / r.capacity, 2) as utilization_pct
    ///     FROM resources r
    ///     ORDER BY utilization_pct DESC
    /// ").await?;
    /// ```
    pub async fn query(&self, sql: &str) -> Result<Vec<arrow::record_batch::RecordBatch>> {
        queries::query_simulation(self, sql).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::AgentStatus;
    use crate::utils::idents::{AgentId, ResourceId, TaskId, WorkflowId};
    use crate::workflows::{WorkflowEngine, WorkflowStep};
    use geo::Point;
    use std::sync::{Arc, Mutex};

    // Test agent
    #[derive(Debug, Clone)]
    struct TestAgent {
        id: AgentId,
        tick_count: usize,
    }

    #[derive(Debug, Clone)]
    struct TestStatus;

    impl AgentStatus for TestStatus {
        fn as_str(&self) -> &str {
            "test"
        }
    }

    impl Agent for TestAgent {
        fn id(&self) -> AgentId {
            self.id
        }

        fn agent_type(&self) -> &str {
            "test"
        }

        fn status(&self) -> &dyn AgentStatus {
            &TestStatus
        }

        fn position(&self) -> Option<Point<f64>> {
            None
        }

        fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
            self.tick_count += 1;
            Ok(AgentTickResult::default())
        }

        fn from_arrow_row(row: &crate::population::ArrowAgentRow) -> Result<Self> {
            let id = AgentId::from_uuid(
                uuid::Uuid::from_slice(&row.id)
                    .map_err(|e| crate::Error::Agent(format!("Invalid UUID: {}", e)))?,
            );

            Ok(TestAgent { id, tick_count: 0 })
        }
    }

    // Test resource
    #[derive(Debug, Clone)]
    struct TestResource {
        id: ResourceId,
        capacity: usize,
        load: usize,
    }

    impl Resource for TestResource {
        fn id(&self) -> ResourceId {
            self.id
        }

        fn resource_type(&self) -> &str {
            "test"
        }

        fn capacity(&self) -> usize {
            self.capacity
        }

        fn current_load(&self) -> usize {
            self.load
        }
    }

    // Test task
    #[derive(Debug, Clone)]
    struct TestTask {
        id: TaskId,
        workflow_instance_id: Option<crate::utils::idents::WorkflowInstanceId>,
        duration: chrono::Duration,
    }

    impl Task for TestTask {
        fn id(&self) -> TaskId {
            self.id
        }

        fn required_resource_type(&self) -> &str {
            "test"
        }

        fn estimated_duration(&self) -> chrono::Duration {
            self.duration
        }

        fn workflow_instance_id(&self) -> Option<crate::utils::idents::WorkflowInstanceId> {
            self.workflow_instance_id
        }
    }

    // Test workflow
    #[derive(Debug, Clone)]
    struct TestWorkflow {
        id: WorkflowId,
        steps: Vec<WorkflowStep>,
    }

    impl Workflow for TestWorkflow {
        fn id(&self) -> WorkflowId {
            self.id
        }

        fn workflow_type(&self) -> &str {
            "test"
        }

        fn steps(&self) -> &[WorkflowStep] {
            &self.steps
        }
    }

    // Test hooks
    #[derive(Default)]
    struct TestHooks {
        start_called: Arc<Mutex<bool>>,
        tick_count: Arc<Mutex<usize>>,
        end_called: Arc<Mutex<bool>>,
    }

    impl SimulationHooks for TestHooks {
        fn on_start(&mut self, _ctx: &SimulationContext) -> Result<()> {
            *self.start_called.lock().unwrap() = true;
            Ok(())
        }

        fn on_after_tick(&mut self, _ctx: &SimulationContext) -> Result<()> {
            *self.tick_count.lock().unwrap() += 1;
            Ok(())
        }

        fn on_end(&mut self, _ctx: &SimulationContext) -> Result<()> {
            *self.end_called.lock().unwrap() = true;
            Ok(())
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    #[test]
    fn test_runner_creation() {
        let config = SimulationConfig::default();
        let runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        assert_eq!(runner.context().step(), 0);
        assert_eq!(runner.stats().total_steps, 0);
    }

    #[test]
    fn test_single_tick() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        runner.tick().unwrap();

        assert_eq!(runner.context().step(), 1);
        assert_eq!(runner.stats().total_steps, 1);
    }

    #[test]
    fn test_run_for_steps() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        runner.run_for_steps(10).unwrap();

        assert_eq!(runner.context().step(), 10);
        assert_eq!(runner.stats().total_steps, 10);
    }

    #[test]
    fn test_run_with_population() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        let mut population = PopulationHandler::new();
        population.add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });
        population.add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });

        runner.set_population_handler(population);
        runner.run_for_steps(5).unwrap();

        assert_eq!(runner.stats().total_steps, 5);
        assert_eq!(runner.stats().total_agent_ticks, 10); // 2 agents * 5 steps
    }

    #[test]
    fn test_lifecycle_hooks() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        let start_called = Arc::new(Mutex::new(false));
        let tick_count = Arc::new(Mutex::new(0));
        let end_called = Arc::new(Mutex::new(false));

        let hooks = TestHooks {
            start_called: start_called.clone(),
            tick_count: tick_count.clone(),
            end_called: end_called.clone(),
        };

        runner.set_hooks(Box::new(hooks));
        runner.run_for_steps(3).unwrap();

        assert!(*start_called.lock().unwrap());
        assert_eq!(*tick_count.lock().unwrap(), 3);
        assert!(*end_called.lock().unwrap());
    }

    #[test]
    fn test_run_until_condition() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Run until 5 steps
        runner
            .run_until(|_ctx, stats| stats.total_steps >= 5)
            .unwrap();

        assert_eq!(runner.stats().total_steps, 5);
    }

    #[test]
    fn test_run_for_duration() {
        let config = SimulationConfig {
            time_step: chrono::Duration::seconds(60), // 1 minute per step
            ..Default::default()
        };

        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        let start_time = runner.context().current_time();

        // Run for 5 minutes
        runner
            .run_for_duration(chrono::Duration::minutes(5))
            .unwrap();

        let elapsed = runner.context().current_time() - start_time;
        assert_eq!(elapsed, chrono::Duration::minutes(5));
        assert_eq!(runner.stats().total_steps, 5);
    }

    // Workflow integration tests

    #[test]
    fn test_runner_advances_workflows_on_task_completion() {
        use crate::resources::ResourceAllocator;
        use crate::workflows::{WorkflowEngine, WorkflowRegistry};
        use std::collections::HashMap;

        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Create workflow engine
        let mut workflow_engine = WorkflowEngine::new();
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![
                WorkflowStep {
                    step_index: 0,
                    required_resource_type: "test".to_string(),
                    expected_duration: chrono::Duration::seconds(10),
                    metadata: HashMap::new(),
                },
                WorkflowStep {
                    step_index: 1,
                    required_resource_type: "test".to_string(),
                    expected_duration: chrono::Duration::seconds(5),
                    metadata: HashMap::new(),
                },
            ],
        };
        let workflow_id = workflow.id();
        workflow_engine.register_workflow(workflow);

        // Start a workflow instance
        let instance = workflow_engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let mut registry = WorkflowRegistry::new();
        registry
            .register("test".to_string(), workflow_engine)
            .unwrap();
        runner.set_workflow_registry(registry);

        // Create resource allocator with a resource
        let mut allocator = ResourceAllocator::new();
        allocator
            .add_resource(TestResource {
                id: ResourceId::new(),
                capacity: 5,
                load: 0,
            })
            .unwrap();

        // Create a task for the workflow
        let task = TestTask {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            duration: chrono::Duration::seconds(10),
        };
        allocator.add_task(task).unwrap();

        runner.set_resource_allocator(allocator);

        // Allocate the task
        runner.allocate_resources().unwrap();

        // Verify workflow is at step 0
        assert_eq!(
            runner
                .workflow_registry
                .as_ref()
                .unwrap()
                .get("test")
                .unwrap()
                .instance_table
                .get_instance(instance_id)
                .unwrap()
                .current_step,
            0
        );

        // Advance time past task completion
        for _ in 0..12 {
            runner.tick().unwrap();
        }

        // Verify workflow advanced to step 1
        let instance = runner
            .workflow_registry
            .as_ref()
            .unwrap()
            .get("test")
            .unwrap()
            .instance_table
            .get_instance(instance_id)
            .unwrap();
        assert_eq!(instance.current_step, 1);
        assert!(!instance.is_complete);

        // Verify stats incremented
        assert_eq!(runner.stats().total_workflow_steps, 1);
    }

    #[test]
    fn test_runner_creates_tasks_for_ready_instances() {
        use crate::resources::{ResourceAllocator, TaskFactory};
        use crate::utils::idents::WorkflowInstanceId;
        use crate::workflows::{WorkflowEngine, WorkflowRegistry, WorkflowStep};
        use std::collections::HashMap;

        // Define a simple task factory
        struct SimpleTaskFactory;
        impl TaskFactory<TestTask> for SimpleTaskFactory {
            fn create_task(
                &self,
                instance_id: WorkflowInstanceId,
                step: &WorkflowStep,
                _current_time: chrono::DateTime<chrono::Utc>,
            ) -> TestTask {
                TestTask {
                    id: TaskId::new(),
                    workflow_instance_id: Some(instance_id),
                    duration: step.expected_duration,
                }
            }
        }

        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Create workflow engine
        let mut workflow_engine = WorkflowEngine::new();
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![WorkflowStep {
                step_index: 0,
                required_resource_type: "test".to_string(),
                expected_duration: chrono::Duration::seconds(10),
                metadata: HashMap::new(),
            }],
        };
        let workflow_id = workflow.id();
        workflow_engine.register_workflow(workflow);

        // Start a workflow instance
        workflow_engine.start_instance(workflow_id).unwrap();

        let mut registry = WorkflowRegistry::new();
        registry
            .register("test".to_string(), workflow_engine)
            .unwrap();
        runner.set_workflow_registry(registry);
        runner.set_resource_allocator(ResourceAllocator::new());
        runner.set_task_factory(Box::new(SimpleTaskFactory));

        // Tick - should create task for ready instance
        runner.tick().unwrap();

        // Verify task was created
        assert_eq!(runner.resources.as_ref().unwrap().pending_task_count(), 1);
    }

    #[test]
    fn test_runner_emits_workflow_events() {
        use crate::resources::ResourceAllocator;
        use crate::workflows::{WorkflowEngine, WorkflowRegistry, WorkflowStepEvent};
        use std::collections::HashMap;

        // Create a hook that captures workflow events
        struct EventCapturingHooks {
            events: Arc<Mutex<Vec<WorkflowStepEvent>>>,
        }

        impl SimulationHooks for EventCapturingHooks {
            fn on_workflow_step_completed(
                &mut self,
                _ctx: &SimulationContext,
                event: &WorkflowStepEvent,
            ) -> Result<()> {
                self.events.lock().unwrap().push(event.clone());
                Ok(())
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }

            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }

        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        let captured_events = Arc::new(Mutex::new(Vec::new()));
        runner.set_hooks(Box::new(EventCapturingHooks {
            events: captured_events.clone(),
        }));

        // Create workflow engine
        let mut workflow_engine = WorkflowEngine::new();
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![WorkflowStep {
                step_index: 0,
                required_resource_type: "test".to_string(),
                expected_duration: chrono::Duration::seconds(10),
                metadata: HashMap::new(),
            }],
        };
        let workflow_id = workflow.id();
        workflow_engine.register_workflow(workflow);

        // Start instance
        let instance = workflow_engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let mut registry = WorkflowRegistry::new();
        registry
            .register("test".to_string(), workflow_engine)
            .unwrap();
        runner.set_workflow_registry(registry);

        // Create resource allocator
        let mut allocator = ResourceAllocator::new();
        allocator
            .add_resource(TestResource {
                id: ResourceId::new(),
                capacity: 5,
                load: 0,
            })
            .unwrap();

        // Create task
        allocator
            .add_task(TestTask {
                id: TaskId::new(),
                workflow_instance_id: Some(instance_id),
                duration: chrono::Duration::seconds(10),
            })
            .unwrap();

        runner.set_resource_allocator(allocator);
        runner.allocate_resources().unwrap();

        // Tick until task completes
        for _ in 0..12 {
            runner.tick().unwrap();
        }

        // Verify event was emitted
        let events = captured_events.lock().unwrap();
        assert_eq!(events.len(), 1);

        match &events[0] {
            WorkflowStepEvent::WorkflowCompleted {
                instance_id: id,
                workflow_id: wf_id,
                ..
            } => {
                assert_eq!(*id, instance_id);
                assert_eq!(*wf_id, workflow_id);
            }
            _ => panic!("Expected WorkflowCompleted event"),
        }
    }

    // Snapshot integration tests

    #[tokio::test]
    async fn test_save_snapshot_memory_backend() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Initialize snapshot manager
        runner.init_snapshot_manager().await.unwrap();

        // Add population
        let mut population = PopulationHandler::new();
        population.add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });
        population.add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });
        runner.set_population_handler(population);

        // Add resources
        let mut allocator = ResourceAllocator::new();
        let _ = allocator.add_resource(TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
        });
        runner.set_resource_allocator(allocator);

        // Add workflows
        let registry = WorkflowRegistry::<TestWorkflow>::new();
        runner.set_workflow_registry(registry);

        // Run a few steps
        runner.run_for_steps(5).unwrap();

        // Save snapshot
        let metadata = runner.save_snapshot("test_snapshot").await.unwrap();

        // Verify metadata
        assert_eq!(metadata.agent_count, 2);
        assert_eq!(metadata.resource_count, 1);
        assert_eq!(metadata.version, "1.0");
    }

    #[tokio::test]
    async fn test_restore_snapshot_memory_backend() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Initialize snapshot manager
        runner.init_snapshot_manager().await.unwrap();

        // Setup initial state
        let mut population = PopulationHandler::new();
        let agent1_id = AgentId::new();
        let agent2_id = AgentId::new();
        population.add_agent(TestAgent {
            id: agent1_id,
            tick_count: 0,
        });
        population.add_agent(TestAgent {
            id: agent2_id,
            tick_count: 0,
        });
        runner.set_population_handler(population);

        let mut allocator = ResourceAllocator::new();
        let resource_id = ResourceId::new();
        let _ = allocator.add_resource(TestResource {
            id: resource_id,
            capacity: 10,
            load: 0,
        });
        runner.set_resource_allocator(allocator);

        runner.set_workflow_registry(WorkflowRegistry::new());

        // Save snapshot
        runner.save_snapshot("restore_test").await.unwrap();

        // Modify state
        runner.population_mut().unwrap().add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });

        // Verify state changed
        assert_eq!(runner.population().unwrap().count(), 3);

        // Restore snapshot
        let metadata = runner.restore_snapshot("restore_test").await.unwrap();

        // Verify state restored
        assert_eq!(metadata.agent_count, 2);
        assert_eq!(metadata.resource_count, 1);
        assert_eq!(runner.population().unwrap().count(), 2);
    }

    #[tokio::test]
    async fn test_save_restore_parquet_backend() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let snapshot_dir = temp_dir.path().to_str().unwrap().to_string();

        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Configure Parquet backend
        runner
            .set_snapshot_backend(crate::snapshot::SnapshotBackend::Parquet(
                snapshot_dir.clone(),
            ))
            .await
            .unwrap();

        // Setup state
        let mut population = PopulationHandler::new();
        population.add_agent(TestAgent {
            id: AgentId::new(),
            tick_count: 0,
        });
        runner.set_population_handler(population);

        let mut allocator = ResourceAllocator::new();
        let _ = allocator.add_resource(TestResource {
            id: ResourceId::new(),
            capacity: 5,
            load: 0,
        });
        runner.set_resource_allocator(allocator);

        runner.set_workflow_registry(WorkflowRegistry::new());

        // Save snapshot
        let save_metadata = runner.save_snapshot("parquet_test").await.unwrap();
        assert_eq!(save_metadata.agent_count, 1);

        // Verify files exist
        let snapshot_path = std::path::PathBuf::from(&snapshot_dir).join("parquet_test");
        assert!(snapshot_path.exists());
        assert!(snapshot_path.join("agents.parquet").exists());
        assert!(snapshot_path.join("resources.parquet").exists());
        assert!(snapshot_path.join("tasks.parquet").exists());
        assert!(snapshot_path.join("workflow_instances.parquet").exists());

        // NOTE: Restoration from Parquet may fail due to string type changes (Utf8 -> Utf8View)
        // This is a known issue with Arrow/Parquet serialization that will be addressed
        // in future schema evolution work. For now, we just verify the save operation works.
        // Uncomment below when schema normalization is implemented:

        // // Clear state
        // runner.set_population_handler(PopulationHandler::new());
        //
        // // Restore snapshot
        // let restore_metadata = runner.restore_snapshot("parquet_test").await.unwrap();
        // assert_eq!(restore_metadata.agent_count, 1);
        // assert_eq!(runner.population().unwrap().count(), 1);
    }

    #[tokio::test]
    async fn test_snapshot_consistency_validation() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Initialize snapshot manager
        runner.init_snapshot_manager().await.unwrap();

        // Setup handlers
        runner.set_population_handler(PopulationHandler::new());
        runner.set_resource_allocator(ResourceAllocator::new());
        runner.set_workflow_registry(WorkflowRegistry::new());

        // Save empty snapshot
        runner.save_snapshot("empty_test").await.unwrap();

        // Restore and verify consistency
        let metadata = runner.restore_snapshot("empty_test").await.unwrap();
        assert_eq!(metadata.agent_count, 0);
        assert_eq!(metadata.resource_count, 0);
    }

    #[tokio::test]
    async fn test_snapshot_without_manager() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Remove snapshot manager
        runner.snapshot_manager = None;

        // Attempt to save should fail
        let result = runner.save_snapshot("should_fail").await;
        assert!(result.is_err());

        // Attempt to restore should fail
        let result = runner.restore_snapshot("should_fail").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_snapshot_with_workflows() {
        use std::collections::HashMap;

        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Initialize snapshot manager
        runner.init_snapshot_manager().await.unwrap();

        // Setup workflow registry with engine containing instances
        let mut workflow_engine = WorkflowEngine::new();
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![WorkflowStep {
                step_index: 0,
                required_resource_type: "test".to_string(),
                expected_duration: chrono::Duration::seconds(10),
                metadata: HashMap::new(),
            }],
        };
        let workflow_id = workflow.id();
        workflow_engine.register_workflow(workflow);
        workflow_engine.start_instance(workflow_id).unwrap();
        workflow_engine.start_instance(workflow_id).unwrap();

        let mut registry = WorkflowRegistry::new();
        registry
            .register("test".to_string(), workflow_engine)
            .unwrap();
        runner.set_workflow_registry(registry);
        runner.set_population_handler(PopulationHandler::new());
        runner.set_resource_allocator(ResourceAllocator::new());

        // Save snapshot - should export 2 workflow instances
        let save_metadata = runner.save_snapshot("workflow_test").await.unwrap();
        assert_eq!(save_metadata.workflow_instance_count, 2);

        // Restore snapshot and verify the instances are really back in the engine
        runner
            .workflow_registry_mut()
            .unwrap()
            .get_mut("test")
            .unwrap()
            .start_instance(workflow_id)
            .unwrap();
        let restore_metadata = runner.restore_snapshot("workflow_test").await.unwrap();
        assert_eq!(restore_metadata.workflow_instance_count, 2);
        assert_eq!(
            runner.workflow_registry().unwrap().total_instance_count(),
            2
        );
    }

    #[tokio::test]
    async fn test_restore_resumes_workflow_execution() {
        use crate::resources::TaskFactory;
        use crate::utils::idents::WorkflowInstanceId;
        use std::collections::HashMap;

        struct StepTasks;
        impl TaskFactory<TestTask> for StepTasks {
            fn create_task(
                &self,
                instance_id: WorkflowInstanceId,
                step: &WorkflowStep,
                _now: chrono::DateTime<chrono::Utc>,
            ) -> TestTask {
                TestTask {
                    id: TaskId::new(),
                    workflow_instance_id: Some(instance_id),
                    duration: step.expected_duration,
                }
            }
        }

        let config = SimulationConfig {
            time_step: chrono::Duration::seconds(5),
            ..Default::default()
        };
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);
        runner.init_snapshot_manager().await.unwrap();

        let step = |secs| WorkflowStep {
            step_index: 0,
            required_resource_type: "test".to_string(),
            expected_duration: chrono::Duration::seconds(secs),
            metadata: HashMap::new(),
        };
        let mut engine = WorkflowEngine::new();
        let workflow = TestWorkflow {
            id: WorkflowId::new(),
            steps: vec![step(10), step(10)],
        };
        let workflow_id = workflow.id();
        engine.register_workflow(workflow);
        let instance_id = engine.start_instance(workflow_id).unwrap().id;
        let mut registry = WorkflowRegistry::new();
        registry.register("test".to_string(), engine).unwrap();
        runner.set_workflow_registry(registry);
        let mut allocator = ResourceAllocator::new();
        allocator
            .add_resource(TestResource {
                id: ResourceId::new(),
                capacity: 1,
                load: 0,
            })
            .unwrap();
        runner.set_resource_allocator(allocator);
        runner.set_task_factory(Box::new(StepTasks));

        let current_step =
            |r: &SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow>| {
                let i = r
                    .workflow_registry()
                    .unwrap()
                    .get("test")
                    .unwrap()
                    .instance_table
                    .get_instance(instance_id)
                    .unwrap();
                (i.current_step, i.is_complete)
            };

        // T=0: step-0 task allocated. Snapshot mid-step at T=5.
        runner.tick().unwrap();
        let saved_step = runner.context().step();
        runner.save_snapshot("mid_step").await.unwrap();

        // Run to completion: step 0 ends at T=10, step 1 at T=20.
        let mut ticks_to_finish = 0;
        while current_step(&runner) != (2, true) {
            runner.tick().unwrap();
            ticks_to_finish += 1;
            assert!(ticks_to_finish < 20, "workflow never completed");
        }
        assert_eq!(
            runner.resources().unwrap().resource_available(
                runner
                    .resources()
                    .unwrap()
                    .find_available_resources_by_type("test")[0]
            ),
            Some(1)
        );

        // Restore: instance is back at step 0 with its task still allocated.
        runner.restore_snapshot("mid_step").await.unwrap();
        assert_eq!(runner.context().step(), saved_step);
        assert_eq!(current_step(&runner), (0, false));
        assert_eq!(runner.resources().unwrap().allocated_task_count(), 1);
        assert!(
            runner
                .resources()
                .unwrap()
                .find_available_resources_by_type("test")
                .is_empty()
        );

        // Replaying from the snapshot takes exactly as long as the first time.
        for _ in 0..ticks_to_finish {
            runner.tick().unwrap();
        }
        assert_eq!(current_step(&runner), (2, true));
        assert_eq!(runner.resources().unwrap().allocated_task_count(), 0);
    }
}
