//! Simulation orchestration
//!
//! The runner coordinates all simulation components through a tick-based loop:
//! - Agent population management
//! - Resource allocation
//! - Workflow execution
//! - Time advancement
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

use crate::context::{SimulationConfig, SimulationContext};
use crate::population::{Agent, AgentTickResult, PopulationHandler};
use crate::resources::{AllocationStrategy, Assignment, Resource, ResourceAllocator, Task};
use crate::workflows::{Workflow, WorkflowEngine};
use crate::Result;
use std::marker::PhantomData;

/// Lifecycle hooks for simulation events
///
/// Implement this trait to receive callbacks at key points in the simulation.
pub trait SimulationHooks: Send + Sync {
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
    fn on_agents_ticked(&mut self, _ctx: &SimulationContext, _results: &[AgentTickResult]) -> Result<()> {
        Ok(())
    }

    /// Called when resources are allocated
    fn on_resources_allocated(&mut self, _ctx: &SimulationContext, _assignments: &[Assignment]) -> Result<()> {
        Ok(())
    }
}

/// Default implementation that does nothing
#[derive(Debug, Default)]
pub struct NoOpHooks;

impl SimulationHooks for NoOpHooks {}

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
    /// Workflow engine (optional)
    workflows: Option<WorkflowEngine<W>>,
    /// Lifecycle hooks
    hooks: Box<dyn SimulationHooks>,
    /// Statistics
    stats: SimulationStats,
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
    pub fn new(config: SimulationConfig) -> Self {
        Self {
            context: SimulationContext::from_now(config),
            population: None,
            resources: None,
            workflows: None,
            hooks: Box::new(NoOpHooks),
            stats: SimulationStats::default(),
            _phantom: PhantomData,
        }
    }

    /// Create a runner with a specific start time
    pub fn with_start_time(start_time: chrono::DateTime<chrono::Utc>, config: SimulationConfig) -> Self {
        Self {
            context: SimulationContext::new(start_time, config),
            population: None,
            resources: None,
            workflows: None,
            hooks: Box::new(NoOpHooks),
            stats: SimulationStats::default(),
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

    /// Set the workflow engine
    pub fn set_workflow_engine(&mut self, engine: WorkflowEngine<W>) {
        self.workflows = Some(engine);
    }

    /// Set lifecycle hooks
    pub fn set_hooks(&mut self, hooks: Box<dyn SimulationHooks>) {
        self.hooks = hooks;
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

    /// Get the workflow engine
    pub fn workflows(&self) -> Option<&WorkflowEngine<W>> {
        self.workflows.as_ref()
    }

    /// Get mutable reference to workflow engine
    pub fn workflows_mut(&mut self) -> Option<&mut WorkflowEngine<W>> {
        self.workflows.as_mut()
    }

    /// Get simulation statistics
    pub fn stats(&self) -> &SimulationStats {
        &self.stats
    }

    /// Execute a single simulation tick
    ///
    /// # Returns
    ///
    /// Ok(()) if the tick completed successfully
    pub fn tick(&mut self) -> Result<()> {
        // Call before-tick hook
        self.hooks.on_before_tick(&self.context)?;

        // Tick agents if population exists
        if let Some(population) = &mut self.population {
            let results = population.tick_agents(&self.context)?;
            self.stats.total_agent_ticks += results.len();
            self.hooks.on_agents_ticked(&self.context, &results)?;
        }

        // TODO: Process workflows if engine exists
        // Will be implemented when WorkflowEngine has a tick method

        // Advance time
        self.context.advance_time();
        self.stats.total_steps += 1;

        // Call after-tick hook
        self.hooks.on_after_tick(&self.context)?;

        Ok(())
    }

    /// Allocate resources using a strategy
    ///
    /// # Arguments
    ///
    /// * `strategy` - Allocation strategy to use
    ///
    /// # Returns
    ///
    /// Number of assignments made
    pub fn allocate_resources(&mut self, strategy: &dyn AllocationStrategy<R, T>) -> Result<usize> {
        if let Some(resources) = &mut self.resources {
            let assignments = resources.assign_tasks(strategy)?;
            let count = assignments.len();
            self.stats.total_assignments += count;
            self.hooks.on_resources_allocated(&self.context, &assignments)?;
            Ok(count)
        } else {
            Ok(0)
        }
    }

    /// Run the simulation for a specified number of steps
    ///
    /// # Arguments
    ///
    /// * `steps` - Number of steps to execute
    ///
    /// # Returns
    ///
    /// Ok(()) if all steps completed successfully
    pub fn run_for_steps(&mut self, steps: usize) -> Result<()> {
        // Call start hook
        self.hooks.on_start(&self.context)?;

        // Execute ticks
        for _ in 0..steps {
            self.tick()?;
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

        // Execute ticks until condition met
        while !condition(&self.context, &self.stats) {
            self.tick()?;
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

    /// Reset the simulation to its initial state
    pub fn reset(&mut self) {
        let config = self.context.config().clone();
        self.context = SimulationContext::from_now(config);
        self.stats = SimulationStats::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::population::AgentStatus;
    use crate::utils::idents::{AgentId, ResourceId, TaskId, WorkflowId};
    use crate::workflows::WorkflowStep;
    use geo::Point;
    use std::sync::{Arc, Mutex};

    // Test agent
    #[derive(Debug)]
    struct TestAgent {
        id: AgentId,
        tick_count: usize,
    }

    #[derive(Debug)]
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
    }

    // Test resource
    #[derive(Debug, Clone)]
    struct TestResource {
        id: ResourceId,
        capacity: usize,
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
            0
        }
    }

    // Test task
    #[derive(Debug, Clone)]
    struct TestTask {
        id: TaskId,
    }

    impl Task for TestTask {
        fn id(&self) -> TaskId {
            self.id
        }

        fn required_resource_type(&self) -> &str {
            "test"
        }

        fn estimated_duration(&self) -> chrono::Duration {
            chrono::Duration::seconds(10)
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
        runner.run_until(|_ctx, stats| stats.total_steps >= 5).unwrap();

        assert_eq!(runner.stats().total_steps, 5);
    }

    #[test]
    fn test_run_for_duration() {
        let mut config = SimulationConfig::default();
        config.time_step = chrono::Duration::seconds(60); // 1 minute per step

        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        let start_time = runner.context().current_time();

        // Run for 5 minutes
        runner.run_for_duration(chrono::Duration::minutes(5)).unwrap();

        let elapsed = runner.context().current_time() - start_time;
        assert_eq!(elapsed, chrono::Duration::minutes(5));
        assert_eq!(runner.stats().total_steps, 5);
    }

    #[test]
    fn test_reset() {
        let config = SimulationConfig::default();
        let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        runner.run_for_steps(10).unwrap();
        assert_eq!(runner.stats().total_steps, 10);

        runner.reset();
        assert_eq!(runner.context().step(), 0);
        assert_eq!(runner.stats().total_steps, 0);
    }
}
