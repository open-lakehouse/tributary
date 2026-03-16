//! End-to-end integration tests for the complete snapshot system
//!
//! This test suite validates the snapshot system through SimulationRunner,
//! covering complete workflows including:
//! - Initialize → run → snapshot → restore → verify
//! - Version compatibility
//! - Multi-handler coordination (all 4 handlers)
//! - Snapshot branching (divergent execution paths)
//! - State consistency validation

use geo_types::Point;
use sim_core::{
    SimulationContext,
    context::SimulationConfig,
    population::{Agent, AgentStatus, AgentTickResult, PopulationHandler},
    resources::{LeastLoaded, Resource, ResourceAllocator, Task, TaskFactory},
    runner::SimulationRunner,
    snapshot::{SnapshotBackend, SnapshotMetadata},
    utils::idents::{AgentId, ResourceId, TaskId, WorkflowId, WorkflowInstanceId},
    workflows::{Workflow, WorkflowEngine, WorkflowStep},
};
use std::collections::HashMap;
use std::time::Instant;
use tempfile::TempDir;

// ============================================================================
// Test Agent Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct TestAgent {
    id: AgentId,
    name: String,
    counter: usize,
    status: TestStatus,
    position: Option<Point<f64>>,
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
        self.position
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        self.counter += 1;
        self.status = TestStatus(format!("Active_{}", self.counter));
        Ok(AgentTickResult::default())
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> sim_core::Result<Self> {
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        Ok(TestAgent {
            id,
            name: "Restored".to_string(),
            counter: 0,
            status: TestStatus(row.status.clone()),
            position: if row.position_x.is_some() && row.position_y.is_some() {
                Some(Point::new(row.position_x.unwrap(), row.position_y.unwrap()))
            } else {
                None
            },
        })
    }
}

// ============================================================================
// Test Resource Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct TestResource {
    id: ResourceId,
    name: String,
    capacity: usize,
    load: usize,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "test_resource"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.load
    }
}

// ============================================================================
// Test Task Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct TestTask {
    id: TaskId,
    duration: chrono::Duration,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        "test_resource"
    }

    fn estimated_duration(&self) -> chrono::Duration {
        self.duration
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        None
    }
}

// ============================================================================
// Test Workflow Implementation
// ============================================================================

#[derive(Debug, Clone)]
struct TestWorkflow {
    id: WorkflowId,
    name: String,
    steps: Vec<WorkflowStep>,
}

impl Workflow for TestWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "test_workflow"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

// ============================================================================
// Test Task Factory
// ============================================================================

struct TestTaskFactory;

impl TaskFactory<TestTask> for TestTaskFactory {
    fn create_task(
        &self,
        _instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: chrono::DateTime<chrono::Utc>,
    ) -> TestTask {
        TestTask {
            id: TaskId::new(),
            duration: step.expected_duration,
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_runner() -> SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> {
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);

    runner.set_population_handler(PopulationHandler::new());
    runner.set_resource_allocator(ResourceAllocator::new());
    runner.set_workflow_engine(WorkflowEngine::new());
    runner.set_task_factory(Box::new(TestTaskFactory));

    runner
}

fn populate_runner(
    runner: &mut SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow>,
    agent_count: usize,
    resource_count: usize,
    workflow_count: usize,
) {
    // Add agents
    if let Some(population) = runner.population_mut() {
        for i in 0..agent_count {
            population.add_agent(TestAgent {
                id: AgentId::new(),
                name: format!("Agent_{}", i),
                counter: 0,
                status: TestStatus("Active".to_string()),
                position: Some(Point::new(i as f64, i as f64)),
            });
        }
    }

    // Add resources
    if let Some(allocator) = runner.resources_mut() {
        for i in 0..resource_count {
            let _ = allocator.add_resource(TestResource {
                id: ResourceId::new(),
                name: format!("Resource_{}", i),
                capacity: 10,
                load: 0,
            });
        }
    }

    // Add workflows
    if let Some(workflow_engine) = runner.workflows_mut() {
        for i in 0..workflow_count {
            let workflow = TestWorkflow {
                id: WorkflowId::new(),
                name: format!("Workflow_{}", i),
                steps: vec![
                    WorkflowStep {
                        step_index: 0,
                        required_resource_type: "test_resource".to_string(),
                        expected_duration: chrono::Duration::seconds(5),
                        metadata: HashMap::new(),
                    },
                    WorkflowStep {
                        step_index: 1,
                        required_resource_type: "test_resource".to_string(),
                        expected_duration: chrono::Duration::seconds(3),
                        metadata: HashMap::new(),
                    },
                ],
            };
            let workflow_id = workflow.id();
            workflow_engine.register_workflow(workflow);
            // Start 2 instances per workflow
            workflow_engine.start_instance(workflow_id).unwrap();
            workflow_engine.start_instance(workflow_id).unwrap();
        }
    }
}

// ============================================================================
// Test E2E-1: Complete SimulationRunner Workflow
// ============================================================================

#[tokio::test]
async fn test_e2e_complete_simulation_workflow() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    // Initialize: 50 agents, 20 resources, 10 workflow instances (5 workflows * 2)
    populate_runner(&mut runner, 50, 20, 5);

    // Verify initial state
    assert_eq!(runner.population().unwrap().count(), 50);
    assert_eq!(runner.resources().unwrap().resource_count(), 20);
    assert_eq!(
        runner.workflows().unwrap().instance_table.instance_count(),
        10
    );

    // Run for 100 steps
    runner.run_for_steps(100).unwrap();
    assert_eq!(runner.stats().total_steps, 100);

    // Save snapshot "checkpoint_100"
    let save_metadata = runner.save_snapshot("checkpoint_100").await.unwrap();
    assert_eq!(save_metadata.agent_count, 50);
    assert_eq!(save_metadata.resource_count, 20);
    assert_eq!(save_metadata.workflow_instance_count, 10);
    assert_eq!(save_metadata.version, "1.0");

    // Run for 50 more steps (now at step 150)
    runner.run_for_steps(50).unwrap();
    assert_eq!(runner.stats().total_steps, 150);

    // Restore snapshot "checkpoint_100"
    let restore_metadata = runner.restore_snapshot("checkpoint_100").await.unwrap();
    assert_eq!(restore_metadata.agent_count, 50);
    assert_eq!(restore_metadata.resource_count, 20);
    assert_eq!(restore_metadata.workflow_instance_count, 10);

    // Verify state is back to checkpoint (entity counts)
    assert_eq!(runner.population().unwrap().count(), 50);
    assert_eq!(runner.resources().unwrap().resource_count(), 20);
    assert_eq!(
        runner.workflows().unwrap().instance_table.instance_count(),
        10
    );

    println!("E2E Test 1 PASSED: Complete workflow");
    println!("  - Initialized with 50 agents, 20 resources, 5 workflows");
    println!("  - Ran for 100 steps, saved checkpoint");
    println!("  - Ran 50 more steps to step 150");
    println!("  - Restored to checkpoint 100 successfully");
}

// ============================================================================
// Test E2E-2: Version Compatibility
// ============================================================================

#[tokio::test]
async fn test_e2e_version_compatibility() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    populate_runner(&mut runner, 25, 10, 3);
    runner.run_for_steps(20).unwrap();

    // Save with version "1.0"
    let save_metadata = runner.save_snapshot("version_test").await.unwrap();
    assert_eq!(save_metadata.version, "1.0");

    // Verify metadata serialization
    let json = save_metadata.to_json().unwrap();
    assert!(json.contains("\"version\": \"1.0\""));

    // Restore and verify
    let restore_metadata = runner.restore_snapshot("version_test").await.unwrap();
    assert_eq!(restore_metadata.version, "1.0");

    // Verify JSON deserialization
    let deserialized = SnapshotMetadata::from_json(&json).unwrap();
    assert_eq!(deserialized.version, "1.0");
    assert_eq!(deserialized.agent_count, 25);

    println!("E2E Test 2 PASSED: Version compatibility");
}

// ============================================================================
// Test E2E-3: Multi-Handler Coordination
// ============================================================================

#[tokio::test]
async fn test_e2e_multi_handler_coordination() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    // Populate all 4 handler types
    populate_runner(&mut runner, 50, 20, 5);

    // Run with resource allocation
    for _ in 0..10 {
        runner.tick().unwrap();
        runner.allocate_resources(&LeastLoaded).unwrap();
    }

    // Save snapshot
    let save_metadata = runner.save_snapshot("multi_handler").await.unwrap();
    let expected_agents = save_metadata.agent_count;
    let expected_resources = save_metadata.resource_count;
    let expected_tasks = save_metadata.task_count;
    let expected_workflows = save_metadata.workflow_instance_count;

    // Modify ALL handlers
    if let Some(population) = runner.population_mut() {
        for i in 0..10 {
            population.add_agent(TestAgent {
                id: AgentId::new(),
                name: format!("Extra_{}", i),
                counter: 0,
                status: TestStatus("Extra".to_string()),
                position: None,
            });
        }
    }

    if let Some(allocator) = runner.resources_mut() {
        for i in 0..5 {
            let _ = allocator.add_resource(TestResource {
                id: ResourceId::new(),
                name: format!("ExtraRes_{}", i),
                capacity: 5,
                load: 0,
            });
        }
    }

    // Verify state changed
    assert_eq!(runner.population().unwrap().count(), expected_agents + 10);
    assert_eq!(
        runner.resources().unwrap().resource_count(),
        expected_resources + 5
    );

    // Restore - ALL handlers should restore atomically
    let restore_metadata = runner.restore_snapshot("multi_handler").await.unwrap();
    assert_eq!(restore_metadata.agent_count, expected_agents);
    assert_eq!(restore_metadata.resource_count, expected_resources);
    assert_eq!(restore_metadata.task_count, expected_tasks);
    assert_eq!(restore_metadata.workflow_instance_count, expected_workflows);

    // Verify actual state
    assert_eq!(runner.population().unwrap().count(), expected_agents);
    assert_eq!(
        runner.resources().unwrap().resource_count(),
        expected_resources
    );
    assert_eq!(
        runner.workflows().unwrap().instance_table.instance_count(),
        expected_workflows
    );

    println!("E2E Test 3 PASSED: Multi-handler coordination (all 4 handlers restored atomically)");
}

// ============================================================================
// Test E2E-4: Snapshot Branching
// ============================================================================

#[tokio::test]
async fn test_e2e_snapshot_branching() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    populate_runner(&mut runner, 50, 20, 10);

    // Run to step 100 and save branch point
    runner.run_for_steps(100).unwrap();
    runner.save_snapshot("branch_point").await.unwrap();

    // Path A: Run 50 more steps
    runner.run_for_steps(50).unwrap();
    let path_a_metadata = runner.save_snapshot("path_a").await.unwrap();

    // Restore to branch point
    runner.restore_snapshot("branch_point").await.unwrap();

    // Path B: Run 30 different steps
    runner.run_for_steps(30).unwrap();
    let path_b_metadata = runner.save_snapshot("path_b").await.unwrap();

    // Both paths have same entity counts
    assert_eq!(path_a_metadata.agent_count, path_b_metadata.agent_count);
    assert_eq!(
        path_a_metadata.resource_count,
        path_b_metadata.resource_count
    );

    // Verify independence
    runner.restore_snapshot("path_a").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 50);

    runner.restore_snapshot("path_b").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 50);

    // Modify path_b
    if let Some(population) = runner.population_mut() {
        population.add_agent(TestAgent {
            id: AgentId::new(),
            name: "PathB_Only".to_string(),
            counter: 0,
            status: TestStatus("PathB".to_string()),
            position: None,
        });
    }
    assert_eq!(runner.population().unwrap().count(), 51);

    // Restore path_a - should NOT have extra agent
    runner.restore_snapshot("path_a").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 50);

    println!("E2E Test 4 PASSED: Snapshot branching (independent divergent paths)");
}

// ============================================================================
// Test E2E-5: State Consistency Validation
// ============================================================================

#[tokio::test]
async fn test_e2e_state_consistency_validation() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    // Create complex state
    populate_runner(&mut runner, 100, 50, 10);

    for _ in 0..20 {
        runner.tick().unwrap();
        runner.allocate_resources(&LeastLoaded).unwrap();
    }

    // Save snapshot
    let save_metadata = runner.save_snapshot("consistency_test").await.unwrap();
    let expected_agents = save_metadata.agent_count;
    let expected_resources = save_metadata.resource_count;
    let expected_tasks = save_metadata.task_count;
    let expected_workflows = save_metadata.workflow_instance_count;

    // Clear ALL handlers
    runner.set_population_handler(PopulationHandler::new());
    runner.set_resource_allocator(ResourceAllocator::new());
    runner.set_workflow_engine(WorkflowEngine::new());

    assert_eq!(runner.population().unwrap().count(), 0);
    assert_eq!(runner.resources().unwrap().resource_count(), 0);

    // Restore - atomic restore of all handlers
    let restore_metadata = runner.restore_snapshot("consistency_test").await.unwrap();
    assert_eq!(restore_metadata.agent_count, expected_agents);
    assert_eq!(restore_metadata.resource_count, expected_resources);
    assert_eq!(restore_metadata.task_count, expected_tasks);
    assert_eq!(restore_metadata.workflow_instance_count, expected_workflows);

    // Verify actual state
    assert_eq!(runner.population().unwrap().count(), expected_agents);
    assert_eq!(
        runner.resources().unwrap().resource_count(),
        expected_resources
    );
    assert_eq!(
        runner.workflows().unwrap().instance_table.instance_count(),
        expected_workflows
    );

    // Verify handlers are functional
    runner.tick().unwrap();
    runner.allocate_resources(&LeastLoaded).unwrap();

    println!("E2E Test 5 PASSED: State consistency validation (atomic restore verified)");
}

// ============================================================================
// Test E2E-6: Large Scale Integration
// ============================================================================

#[tokio::test]
async fn test_e2e_large_scale_simulation() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    // Large scale: 500 agents, 100 resources, 20 workflows
    populate_runner(&mut runner, 500, 100, 20);

    for _ in 0..50 {
        runner.tick().unwrap();
        runner.allocate_resources(&LeastLoaded).unwrap();
    }

    // Benchmark save
    let save_start = Instant::now();
    let save_metadata = runner.save_snapshot("large_scale").await.unwrap();
    let save_duration = save_start.elapsed();

    assert_eq!(save_metadata.agent_count, 500);
    assert_eq!(save_metadata.resource_count, 100);
    assert_eq!(save_metadata.workflow_instance_count, 40);

    // Clear state
    runner.set_population_handler(PopulationHandler::new());
    runner.set_resource_allocator(ResourceAllocator::new());
    runner.set_workflow_engine(WorkflowEngine::new());

    // Benchmark restore
    let restore_start = Instant::now();
    let restore_metadata = runner.restore_snapshot("large_scale").await.unwrap();
    let restore_duration = restore_start.elapsed();

    assert_eq!(restore_metadata.agent_count, 500);
    assert_eq!(runner.population().unwrap().count(), 500);
    assert_eq!(runner.resources().unwrap().resource_count(), 100);
    assert_eq!(
        runner.workflows().unwrap().instance_table.instance_count(),
        40
    );

    println!("E2E Test 6 PASSED: Large scale simulation");
    println!("  - Saved 500 agents in {:?}", save_duration);
    println!("  - Restored 500 agents in {:?}", restore_duration);
}

// ============================================================================
// Test E2E-7: Multiple Snapshots
// ============================================================================

#[tokio::test]
async fn test_e2e_multiple_snapshots_sequence() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    populate_runner(&mut runner, 30, 15, 5);

    // Save every 25 steps
    for i in 1..=4 {
        runner.run_for_steps(25).unwrap();
        let metadata = runner
            .save_snapshot(&format!("checkpoint_{}", i * 25))
            .await
            .unwrap();
        assert_eq!(metadata.agent_count, 30);
    }

    // Restore to any checkpoint
    for i in 1..=4 {
        let metadata = runner
            .restore_snapshot(&format!("checkpoint_{}", i * 25))
            .await
            .unwrap();
        assert_eq!(metadata.agent_count, 30);
        assert_eq!(runner.population().unwrap().count(), 30);
    }

    println!("E2E Test 7 PASSED: Multiple snapshots (4 checkpoints created and verified)");
}

// ============================================================================
// Test E2E-8: Parquet Backend Integration
// ============================================================================

#[tokio::test]
async fn test_e2e_parquet_backend_integration() {
    let temp_dir = TempDir::new().unwrap();
    let snapshot_dir = temp_dir.path().to_str().unwrap().to_string();

    let mut runner = create_runner();
    runner
        .set_snapshot_backend(SnapshotBackend::Parquet(snapshot_dir.clone()))
        .await
        .unwrap();

    populate_runner(&mut runner, 100, 40, 10);
    runner.run_for_steps(30).unwrap();

    // Save to Parquet
    let save_metadata = runner.save_snapshot("parquet_e2e").await.unwrap();

    // Verify files
    let snapshot_path = std::path::PathBuf::from(&snapshot_dir).join("parquet_e2e");
    assert!(snapshot_path.exists());
    assert!(snapshot_path.join("agents.parquet").exists());
    assert!(snapshot_path.join("resources.parquet").exists());
    assert!(snapshot_path.join("tasks.parquet").exists());
    assert!(snapshot_path.join("workflow_instances.parquet").exists());

    assert_eq!(save_metadata.agent_count, 100);
    assert_eq!(save_metadata.resource_count, 40);
    assert_eq!(save_metadata.workflow_instance_count, 20);

    println!("E2E Test 8 PASSED: Parquet backend integration");
}

// ============================================================================
// Test E2E-9: Empty to Populated Transitions
// ============================================================================

#[tokio::test]
async fn test_e2e_empty_to_populated_transitions() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    // Save empty
    let empty_metadata = runner.save_snapshot("empty").await.unwrap();
    assert_eq!(empty_metadata.agent_count, 0);

    // Populate
    populate_runner(&mut runner, 50, 20, 5);
    let populated_metadata = runner.save_snapshot("populated").await.unwrap();
    assert_eq!(populated_metadata.agent_count, 50);

    // Restore to empty
    runner.restore_snapshot("empty").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 0);

    // Restore to populated
    runner.restore_snapshot("populated").await.unwrap();
    assert_eq!(runner.population().unwrap().count(), 50);

    println!("E2E Test 9 PASSED: Empty to populated transitions");
}

// ============================================================================
// Test E2E-10: Metadata Accuracy
// ============================================================================

#[tokio::test]
async fn test_e2e_snapshot_metadata_accuracy() {
    let mut runner = create_runner();
    runner.init_snapshot_manager().await.unwrap();

    populate_runner(&mut runner, 75, 30, 8);

    let save_metadata = runner.save_snapshot("metadata_test").await.unwrap();

    // Verify accuracy
    assert_eq!(save_metadata.agent_count, 75);
    assert_eq!(save_metadata.resource_count, 30);
    assert_eq!(save_metadata.workflow_instance_count, 16); // 8 * 2
    assert_eq!(save_metadata.version, "1.0");

    // Verify timestamp
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(save_metadata.timestamp);
    assert!(diff.num_seconds() < 5);

    // Verify JSON roundtrip
    let json = save_metadata.to_json().unwrap();
    let deserialized = SnapshotMetadata::from_json(&json).unwrap();
    assert_eq!(deserialized.agent_count, 75);
    assert_eq!(deserialized.resource_count, 30);

    println!("E2E Test 10 PASSED: Metadata accuracy verified");
}
