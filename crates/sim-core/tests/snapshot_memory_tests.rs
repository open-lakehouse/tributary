//! Comprehensive tests for Memory backend snapshot operations
//!
//! These tests verify that the snapshot system correctly saves and restores
//! complete simulation state using Memory backend.

use geo::Point;
use sim_core::{
    Error, Result,
    context::SimulationConfig,
    population::{Agent, AgentStatus, AgentTickResult, PopulationHandler},
    resources::{Resource, ResourceAllocator, Task},
    runner::SimulationRunner,
    snapshot::{SnapshotBackend, SnapshotData, SnapshotManager},
    utils::idents::{AgentId, ResourceId, TaskId, WorkflowId, WorkflowInstanceId},
    workflows::{Workflow, WorkflowEngine, WorkflowRegistry, WorkflowStep},
};

// =============================================================================
// Test Agent Implementation
// =============================================================================

#[derive(Debug, Clone)]
struct TestAgent {
    id: AgentId,
    status: TestStatus,
}

#[derive(Debug, Clone)]
struct TestStatus(String);

impl AgentStatus for TestStatus {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Agent for TestAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "test_agent"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.status
    }

    fn position(&self) -> Option<Point<f64>> {
        None
    }

    fn tick(&mut self, _ctx: &sim_core::context::SimulationContext) -> Result<AgentTickResult> {
        Ok(AgentTickResult::default())
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> Result<Self> {
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| Error::Agent(format!("Invalid UUID: {}", e)))?,
        );
        Ok(TestAgent {
            id,
            status: TestStatus("active".to_string()),
        })
    }
}

// =============================================================================
// Test Resource Implementation
// =============================================================================

#[derive(Debug, Clone)]
struct TestResource {
    id: ResourceId,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "test_resource"
    }

    fn capacity(&self) -> usize {
        10
    }

    fn current_load(&self) -> usize {
        0
    }
}

// =============================================================================
// Test Task Implementation
// =============================================================================

#[derive(Debug, Clone)]
struct TestTask {
    id: TaskId,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "test_resource"
    }

    fn estimated_duration(&self) -> chrono::Duration {
        chrono::Duration::minutes(5)
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        None
    }
}

// =============================================================================
// Test Workflow Implementation
// =============================================================================

#[derive(Debug, Clone)]
struct TestWorkflow {
    id: WorkflowId,
}

impl Workflow for TestWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "test_workflow"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &[]
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

fn create_test_agents(count: usize) -> Vec<TestAgent> {
    (0..count)
        .map(|_| TestAgent {
            id: AgentId::new(),
            status: TestStatus("active".to_string()),
        })
        .collect()
}

fn setup_simulation() -> SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> {
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);

    runner.set_population_handler(PopulationHandler::new());
    runner.set_resource_allocator(ResourceAllocator::new());
    runner.set_workflow_registry(WorkflowRegistry::new());

    runner
}

// =============================================================================
// Tests
// =============================================================================

#[tokio::test]
async fn test_small_simulation_round_trip() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();

    // Capture agent IDs before snapshot
    let agents = create_test_agents(100);
    let agent_ids: Vec<AgentId> = agents.iter().take(10).map(|a| a.id).collect();

    // Add agents
    runner.population_mut().unwrap().add_agents(agents).unwrap();
    let original_count = runner.population().unwrap().count();

    // Save and restore
    let meta = runner.save_snapshot("test_100").await.unwrap();
    assert_eq!(meta.agent_count, 100);

    runner.restore_snapshot("test_100").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), original_count);

    // Verify agents can be retrieved by ID (proves index rebuilt)
    for id in agent_ids {
        assert!(runner.population().unwrap().get_agent(id).is_some());
    }
}

#[tokio::test]
async fn test_index_rebuild_verification() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();

    // Add agents and capture all IDs
    let agents = create_test_agents(50);
    let all_ids: Vec<AgentId> = agents.iter().map(|a| a.id).collect();
    runner.population_mut().unwrap().add_agents(agents).unwrap();

    // Save and restore
    runner.save_snapshot("index_test").await.unwrap();
    runner.restore_snapshot("index_test").await.unwrap();

    // Verify EVERY agent can be looked up (proves index correct)
    for id in all_ids {
        let agent = runner.population().unwrap().get_agent(id);
        assert!(agent.is_some(), "Agent {:?} not found after restore", id);
        assert_eq!(agent.unwrap().id(), id);
    }
}

#[tokio::test]
async fn test_error_invalid_snapshot_id() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();
    runner
        .population_mut()
        .unwrap()
        .add_agents(create_test_agents(10))
        .unwrap();

    let result = runner.restore_snapshot("nonexistent").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_error_missing_snapshot() {
    let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();
    let result = manager.restore_snapshot("missing").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_empty_simulation_handling() {
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    runner.set_population_handler(PopulationHandler::new());
    runner.set_resource_allocator(ResourceAllocator::new());
    runner.set_workflow_registry(WorkflowRegistry::new());
    runner.init_snapshot_manager().await.unwrap();

    let meta = runner.save_snapshot("empty").await.unwrap();
    assert_eq!(meta.agent_count, 0);

    runner.restore_snapshot("empty").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 0);
}

#[tokio::test]
async fn test_multi_entity_snapshot() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();

    let agents = create_test_agents(25);
    let sample_id = agents[0].id;
    runner.population_mut().unwrap().add_agents(agents).unwrap();

    let meta = runner.save_snapshot("multi").await.unwrap();
    assert_eq!(meta.agent_count, 25);

    runner.restore_snapshot("multi").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 25);
    assert!(runner.population().unwrap().get_agent(sample_id).is_some());
}

#[tokio::test]
async fn test_data_integrity_after_restore() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();

    let agents = create_test_agents(20);
    let agent_details: Vec<(AgentId, String)> = agents
        .iter()
        .map(|a| (a.id, a.agent_type().to_string()))
        .collect();

    runner.population_mut().unwrap().add_agents(agents).unwrap();

    runner.save_snapshot("integrity").await.unwrap();
    runner.restore_snapshot("integrity").await.unwrap();

    for (id, agent_type) in agent_details {
        let agent = runner.population().unwrap().get_agent(id).unwrap();
        assert_eq!(agent.id(), id);
        assert_eq!(agent.agent_type(), agent_type);
    }
}

#[tokio::test]
async fn test_multiple_snapshots_isolation() {
    let mut runner1 = setup_simulation();
    let mut runner2 = setup_simulation();

    runner1.init_snapshot_manager().await.unwrap();
    runner2.init_snapshot_manager().await.unwrap();

    runner1
        .population_mut()
        .unwrap()
        .add_agents(create_test_agents(10))
        .unwrap();
    runner2
        .population_mut()
        .unwrap()
        .add_agents(create_test_agents(20))
        .unwrap();

    let meta1 = runner1.save_snapshot("snap_10").await.unwrap();
    let meta2 = runner2.save_snapshot("snap_20").await.unwrap();

    assert_eq!(meta1.agent_count, 10);
    assert_eq!(meta2.agent_count, 20);

    runner1.restore_snapshot("snap_10").await.unwrap();
    runner2.restore_snapshot("snap_20").await.unwrap();

    assert_eq!(runner1.population().unwrap().count(), 10);
    assert_eq!(runner2.population().unwrap().count(), 20);
}

#[tokio::test]
async fn test_snapshot_manager_direct_usage() {
    let manager = SnapshotManager::new(SnapshotBackend::Memory).await.unwrap();

    let mut population = PopulationHandler::new();
    population.add_agents(create_test_agents(15)).unwrap();

    let agents_batch = population.export_batch();
    let allocator: ResourceAllocator<TestResource, TestTask> = ResourceAllocator::new();
    let (resources_batch, tasks_batch) = allocator.export_batch();
    let engine: WorkflowEngine<TestWorkflow> = WorkflowEngine::new();
    let workflow_batch = engine.instance_table.export_batch();

    let snapshot_data = SnapshotData {
        agents: agents_batch,
        resources: resources_batch,
        tasks: tasks_batch,
        workflow_instances: workflow_batch,
        locations: None,
    };

    manager
        .save_snapshot("direct", snapshot_data)
        .await
        .unwrap();
    let restored = manager.restore_snapshot("direct").await.unwrap();

    assert_eq!(restored.agents.num_rows(), 15);

    let mut new_pop: PopulationHandler<TestAgent> = PopulationHandler::new();
    new_pop.import_batch(restored.agents).unwrap();
    assert_eq!(new_pop.count(), 15);
}

#[tokio::test]
async fn test_large_batch_performance() {
    let mut runner = setup_simulation();
    runner.init_snapshot_manager().await.unwrap();

    runner
        .population_mut()
        .unwrap()
        .add_agents(create_test_agents(500))
        .unwrap();

    let save_start = std::time::Instant::now();
    runner.save_snapshot("large").await.unwrap();
    let save_duration = save_start.elapsed();

    let restore_start = std::time::Instant::now();
    runner.restore_snapshot("large").await.unwrap();
    let restore_duration = restore_start.elapsed();

    assert_eq!(runner.population().unwrap().count(), 500);

    assert!(save_duration.as_millis() < 1000);
    assert!(restore_duration.as_millis() < 1000);

    println!(
        "Performance: save={}ms, restore={}ms",
        save_duration.as_millis(),
        restore_duration.as_millis()
    );
}
