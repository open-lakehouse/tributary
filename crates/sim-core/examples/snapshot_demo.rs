//! Demonstration of snapshot save/restore functionality
//!
//! This example shows how to:
//! - Initialize a simulation with agents, resources, and workflows
//! - Save snapshots at different points during simulation
//! - Restore from a snapshot to replay or branch the simulation

use sim_core::{
    context::SimulationConfig,
    population::{Agent, AgentStatus, AgentTickResult, PopulationHandler},
    resources::{Resource, ResourceAllocator, Task},
    runner::SimulationRunner,
    snapshot::SnapshotBackend,
    utils::idents::{AgentId, ResourceId, TaskId},
    workflows::{Workflow, WorkflowEngine},
};
use geo::Point;

// Example agent implementation
#[derive(Debug, Clone)]
struct DemoAgent {
    id: AgentId,
    name: String,
    tick_count: usize,
    status: DemoStatus,
}

#[derive(Debug, Clone)]
struct DemoStatus(String);

impl AgentStatus for DemoStatus {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Agent for DemoAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "demo"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        None
    }

    fn tick(&mut self, _ctx: &sim_core::context::SimulationContext) -> sim_core::Result<AgentTickResult> {
        self.tick_count += 1;
        self.status = DemoStatus(format!("Active (ticked {} times)", self.tick_count));
        Ok(AgentTickResult::default())
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> sim_core::Result<Self> {
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        Ok(DemoAgent {
            id,
            name: "Restored Agent".to_string(),
            tick_count: 0,
            status: DemoStatus("Active".to_string()),
        })
    }
}

// Example resource implementation
#[derive(Debug, Clone)]
struct DemoResource {
    id: ResourceId,
    name: String,
    capacity: usize,
    load: usize,
}

impl Resource for DemoResource {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "demo"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.load
    }
}

// Example task implementation
#[derive(Debug, Clone)]
struct DemoTask {
    id: TaskId,
    duration: chrono::Duration,
}

impl Task for DemoTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "demo"
    }

    fn estimated_duration(&self) -> chrono::Duration {
        self.duration
    }

    fn workflow_instance_id(&self) -> Option<sim_core::utils::idents::WorkflowInstanceId> {
        None
    }
}

// Example workflow implementation
#[derive(Debug, Clone)]
struct DemoWorkflow {
    id: sim_core::utils::idents::WorkflowId,
}

impl Workflow for DemoWorkflow {
    fn id(&self) -> sim_core::utils::idents::WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "demo"
    }

    fn steps(&self) -> &[sim_core::workflows::WorkflowStep] {
        &[]
    }
}

#[tokio::main]
async fn main() -> sim_core::Result<()> {
    println!("=== Snapshot Demo ===\n");

    // Create simulation configuration
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<DemoAgent, DemoResource, DemoTask, DemoWorkflow> =
        SimulationRunner::new(config);

    // Initialize snapshot manager with Memory backend
    println!("Initializing snapshot manager with Memory backend...");
    runner.init_snapshot_manager().await?;

    // Setup population
    println!("\nSetting up simulation with 5 agents...");
    let mut population = PopulationHandler::new();
    for i in 0..5 {
        population.add_agent(DemoAgent {
            id: AgentId::new(),
            name: format!("Agent {}", i),
            tick_count: 0,
            status: DemoStatus("Active".to_string()),
        });
    }
    runner.set_population_handler(population);

    // Setup resources
    println!("Adding 3 resources...");
    let mut allocator = ResourceAllocator::new();
    for i in 0..3 {
        let _ = allocator.add_resource(DemoResource {
            id: ResourceId::new(),
            name: format!("Resource {}", i),
            capacity: 10,
            load: 0,
        });
    }
    runner.set_resource_allocator(allocator);

    // Setup workflow engine
    runner.set_workflow_engine(WorkflowEngine::new());

    // Run initial simulation
    println!("\nRunning simulation for 10 steps...");
    runner.run_for_steps(10)?;
    println!("Stats: {} total steps, {} agent ticks",
        runner.stats().total_steps,
        runner.stats().total_agent_ticks
    );

    // Save first snapshot
    println!("\n--- Saving snapshot 'checkpoint_10' ---");
    let metadata = runner.save_snapshot("checkpoint_10").await?;
    println!("Saved snapshot with:");
    println!("  - {} agents", metadata.agent_count);
    println!("  - {} resources", metadata.resource_count);
    println!("  - {} tasks", metadata.task_count);
    println!("  - Timestamp: {}", metadata.timestamp);

    // Continue simulation
    println!("\nContinuing simulation for another 20 steps...");
    runner.run_for_steps(20)?;
    println!("Stats: {} total steps, {} agent ticks",
        runner.stats().total_steps,
        runner.stats().total_agent_ticks
    );

    // Save second snapshot
    println!("\n--- Saving snapshot 'checkpoint_30' ---");
    let metadata = runner.save_snapshot("checkpoint_30").await?;
    println!("Saved snapshot with:");
    println!("  - {} agents", metadata.agent_count);
    println!("  - {} resources", metadata.resource_count);
    println!("  - Timestamp: {}", metadata.timestamp);

    // Demonstrate restore
    println!("\n--- Restoring from 'checkpoint_10' ---");
    let restore_metadata = runner.restore_snapshot("checkpoint_10").await?;
    println!("Restored snapshot:");
    println!("  - {} agents", restore_metadata.agent_count);
    println!("  - {} resources", restore_metadata.resource_count);
    println!("  - State reset to step 10");

    // Verify state was restored
    println!("\nVerifying restored state...");
    println!("  - Population count: {}", runner.population().unwrap().count());
    println!("  - Resource count: {}", runner.resources().unwrap().resource_count());

    // Demonstrate Parquet backend
    println!("\n\n=== Parquet Backend Demo ===\n");
    let temp_dir = tempfile::TempDir::new().unwrap();
    let snapshot_dir = temp_dir.path().to_str().unwrap();

    println!("Switching to Parquet backend at: {}", snapshot_dir);
    runner.set_snapshot_backend(SnapshotBackend::Parquet(snapshot_dir.to_string())).await?;

    // Save to Parquet
    println!("\n--- Saving snapshot 'parquet_demo' to disk ---");
    let metadata = runner.save_snapshot("parquet_demo").await?;
    println!("Saved snapshot to Parquet files:");
    println!("  - {}/parquet_demo/agents.parquet", snapshot_dir);
    println!("  - {}/parquet_demo/resources.parquet", snapshot_dir);
    println!("  - {}/parquet_demo/tasks.parquet", snapshot_dir);
    println!("  - {}/parquet_demo/workflow_instances.parquet", snapshot_dir);
    println!("\nSnapshot metadata:");
    println!("  - Version: {}", metadata.version);
    println!("  - Agent count: {}", metadata.agent_count);
    println!("  - Resource count: {}", metadata.resource_count);

    // Verify files exist
    let snapshot_path = std::path::PathBuf::from(snapshot_dir).join("parquet_demo");
    println!("\nVerifying Parquet files...");
    println!("  - agents.parquet exists: {}", snapshot_path.join("agents.parquet").exists());
    println!("  - resources.parquet exists: {}", snapshot_path.join("resources.parquet").exists());
    println!("  - tasks.parquet exists: {}", snapshot_path.join("tasks.parquet").exists());
    println!("  - workflow_instances.parquet exists: {}",
        snapshot_path.join("workflow_instances.parquet").exists());

    println!("\n=== Demo Complete ===");
    println!("\nKey Takeaways:");
    println!("  1. Snapshots capture complete simulation state");
    println!("  2. Memory backend is fast for development/testing");
    println!("  3. Parquet backend enables persistent storage and analysis");
    println!("  4. Snapshots can be restored to replay or branch simulations");

    Ok(())
}
