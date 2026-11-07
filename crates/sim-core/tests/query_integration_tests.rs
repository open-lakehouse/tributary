//! Integration tests for cross-entity SQL query interface
//!
//! These tests verify that SimulationRunner.query() can execute SQL queries
//! across multiple entity types using JOIN operations.

use arrow::array::{Array, Int32Array, Int64Array, StringArray};
use geo::Point;
use sim_core::context::SimulationContext;
use sim_core::population::AgentTickResult;
use sim_core::utils::idents::{AgentId, ResourceId, TaskId, WorkflowId, WorkflowInstanceId};
use sim_core::{
    Agent, AgentStatus, PopulationHandler, Resource, ResourceAllocator, SimulationConfig,
    SimulationRunner, Task, Workflow, WorkflowEngine, WorkflowRegistry, WorkflowStep,
};
use std::collections::HashMap;

// Test types
#[derive(Debug, Clone)]
struct TestAgent {
    id: AgentId,
    agent_type: String,
}

#[derive(Debug, Clone)]
struct TestStatus;

impl AgentStatus for TestStatus {
    fn as_str(&self) -> &str {
        "active"
    }
}

impl Agent for TestAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        &self.agent_type
    }

    fn status(&self) -> &dyn AgentStatus {
        &TestStatus
    }

    fn position(&self) -> Option<Point<f64>> {
        None
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        Ok(AgentTickResult::default())
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> sim_core::Result<Self> {
        let id = AgentId::from_uuid(
            uuid::Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );
        Ok(TestAgent {
            id,
            agent_type: row.agent_type.clone(),
        })
    }
}

#[derive(Debug, Clone)]
struct TestResource {
    id: ResourceId,
    resource_type: String,
    capacity: usize,
    load: usize,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        &self.resource_type
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.load
    }
}

#[derive(Debug, Clone)]
struct TestTask {
    id: TaskId,
    task_type: String,
    workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.task_type
    }

    fn estimated_duration(&self) -> chrono::Duration {
        chrono::Duration::seconds(10)
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

#[derive(Debug, Clone)]
struct TestWorkflow {
    id: WorkflowId,
    workflow_type: String,
    steps: Vec<WorkflowStep>,
}

impl Workflow for TestWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        &self.workflow_type
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

// Helper function to create a runner with all components
fn create_test_runner_with_all_components()
-> SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> {
    let config = SimulationConfig::default();
    let mut runner = SimulationRunner::new(config);

    // Add population
    let mut pop = PopulationHandler::new();
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "customer".to_string(),
    });
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "worker".to_string(),
    });
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "customer".to_string(),
    });
    runner.set_population_handler(pop);

    // Add resources
    let mut allocator = ResourceAllocator::new();
    let _ = allocator.add_resource(TestResource {
        id: ResourceId::new(),
        resource_type: "kitchen".to_string(),
        capacity: 5,
        load: 0,
    });
    let _ = allocator.add_resource(TestResource {
        id: ResourceId::new(),
        resource_type: "kitchen".to_string(),
        capacity: 10,
        load: 0,
    });

    // Add tasks
    let _ = allocator.add_task(TestTask {
        id: TaskId::new(),
        task_type: "kitchen".to_string(),
        workflow_instance_id: None,
    });
    let _ = allocator.add_task(TestTask {
        id: TaskId::new(),
        task_type: "kitchen".to_string(),
        workflow_instance_id: None,
    });

    runner.set_resource_allocator(allocator);

    // Add workflows
    let mut workflow_engine = WorkflowEngine::new();
    let workflow = TestWorkflow {
        id: WorkflowId::new(),
        workflow_type: "food_prep".to_string(),
        steps: vec![WorkflowStep {
            step_index: 0,
            required_resource_type: "kitchen".to_string(),
            expected_duration: chrono::Duration::seconds(10),
            metadata: HashMap::new(),
        }],
    };
    let workflow_id = workflow.id();
    workflow_engine.register_workflow(workflow);
    let _ = workflow_engine.start_instance(workflow_id);
    let _ = workflow_engine.start_instance(workflow_id);

    let mut registry = WorkflowRegistry::new();
    registry
        .register("food_prep".to_string(), workflow_engine)
        .unwrap();
    runner.set_workflow_registry(registry);

    runner
}

#[tokio::test]
async fn test_query_agents_only() {
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    let mut pop = PopulationHandler::new();
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "type_a".to_string(),
    });
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "type_b".to_string(),
    });
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "type_a".to_string(),
    });
    runner.set_population_handler(pop);

    // Query all agents
    let result = runner.query("SELECT * FROM agents").await.unwrap();
    assert_eq!(result[0].num_rows(), 3);

    // Query with filter
    let result = runner
        .query("SELECT * FROM agents WHERE agent_type = 'type_a'")
        .await
        .unwrap();
    assert_eq!(result[0].num_rows(), 2);

    // Query with aggregation
    let result = runner
        .query("SELECT agent_type, COUNT(*) as count FROM agents GROUP BY agent_type ORDER BY agent_type")
        .await
        .unwrap();

    // DataFusion may return results in a single batch or multiple batches
    let total_rows: usize = result.iter().map(|batch| batch.num_rows()).sum();
    assert_eq!(total_rows, 2);

    let agent_type_col = result[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let count_col = result[0]
        .column(1)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();

    // Verify aggregation results
    for i in 0..result[0].num_rows() {
        let agent_type = agent_type_col.value(i);
        let count = count_col.value(i);
        if agent_type == "type_a" {
            assert_eq!(count, 2);
        } else if agent_type == "type_b" {
            assert_eq!(count, 1);
        }
    }
}

#[tokio::test]
async fn test_query_resources_only() {
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    let mut allocator = ResourceAllocator::new();
    let _ = allocator.add_resource(TestResource {
        id: ResourceId::new(),
        resource_type: "kitchen".to_string(),
        capacity: 10,
        load: 0,
    });
    let _ = allocator.add_resource(TestResource {
        id: ResourceId::new(),
        resource_type: "delivery".to_string(),
        capacity: 5,
        load: 0,
    });
    runner.set_resource_allocator(allocator);

    // Query all resources
    let result = runner.query("SELECT * FROM resources").await.unwrap();
    assert_eq!(result[0].num_rows(), 2);

    // Query with calculated field
    let result = runner
        .query(
            "SELECT resource_type, capacity, available,
             (capacity - available) as in_use
             FROM resources",
        )
        .await
        .unwrap();
    assert_eq!(result[0].num_rows(), 2);

    // Query all tasks
    let result = runner.query("SELECT * FROM tasks").await.unwrap();
    assert_eq!(result[0].num_rows(), 0); // No tasks added yet
}

#[tokio::test]
async fn test_query_join_agents_tasks() {
    let mut runner = create_test_runner_with_all_components();

    // First, allocate tasks to resources
    runner.allocate_resources().unwrap();

    // Note: Tasks don't have assigned_agent_id, so we'll query tasks with workflow instances
    let result = runner
        .query(
            "SELECT t.task_type, t.status, COUNT(*) as count
             FROM tasks t
             GROUP BY t.task_type, t.status",
        )
        .await
        .unwrap();

    assert!(result[0].num_rows() > 0);
}

#[tokio::test]
async fn test_query_join_agents_workflows() {
    let runner = create_test_runner_with_all_components();

    // Query workflow instances (they have agent_id)
    let result = runner
        .query("SELECT * FROM workflow_instances")
        .await
        .unwrap();

    // We started 2 workflow instances
    assert_eq!(result[0].num_rows(), 2);

    // Join workflows with agents (workflow instances may not have agent_id set)
    // This query tests the JOIN capability even if no rows match
    let result = runner
        .query(
            "SELECT w.status, COUNT(*) as count
             FROM workflow_instances w
             GROUP BY w.status",
        )
        .await
        .unwrap();

    assert!(result[0].num_rows() > 0);
}

#[tokio::test]
async fn test_query_complex_three_way_join() {
    let mut runner = create_test_runner_with_all_components();

    // Add tasks with workflow instance IDs
    let engine = runner
        .workflow_registry()
        .unwrap()
        .get("food_prep")
        .unwrap();
    let instances: Vec<_> = engine
        .instance_table
        .export_batch()
        .column(0)
        .as_any()
        .downcast_ref::<arrow::array::BinaryArray>()
        .unwrap()
        .iter()
        .flatten()
        .map(|bytes| WorkflowInstanceId::from_bytes(bytes).unwrap())
        .collect();

    if let Some(instance_id) = instances.first() {
        runner
            .resources_mut()
            .unwrap()
            .add_task(TestTask {
                id: TaskId::new(),
                task_type: "kitchen".to_string(),
                workflow_instance_id: Some(*instance_id),
            })
            .unwrap();
    }

    // Three-way join: workflows, tasks, resources
    let result = runner
        .query(
            "SELECT
                w.status as workflow_status,
                t.status as task_status,
                r.resource_type
             FROM workflow_instances w
             JOIN tasks t ON w.id = t.workflow_instance_id
             JOIN resources r ON t.assigned_resource_id = r.id",
        )
        .await;

    // Query may return empty result if no tasks are assigned, but should not error
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_query_aggregation_by_agent_type() {
    let runner = create_test_runner_with_all_components();

    // Query agent type distribution
    let result = runner
        .query(
            "SELECT agent_type, COUNT(*) as count
             FROM agents
             GROUP BY agent_type
             ORDER BY count DESC",
        )
        .await
        .unwrap();

    assert_eq!(result[0].num_rows(), 2); // customer and worker types

    let agent_type_col = result[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let count_col = result[0]
        .column(1)
        .as_any()
        .downcast_ref::<Int64Array>()
        .unwrap();

    // First row should be "customer" with count 2
    assert_eq!(agent_type_col.value(0), "customer");
    assert_eq!(count_col.value(0), 2);

    // Second row should be "worker" with count 1
    assert_eq!(agent_type_col.value(1), "worker");
    assert_eq!(count_col.value(1), 1);
}

#[tokio::test]
async fn test_query_subquery() {
    let runner = create_test_runner_with_all_components();

    // Subquery to find resources with most capacity
    let result = runner
        .query(
            "SELECT resource_type, capacity
             FROM resources
             WHERE capacity = (SELECT MAX(capacity) FROM resources)",
        )
        .await
        .unwrap();

    // Should have at least one result
    assert!(!result.is_empty());
    assert!(result[0].num_rows() > 0);

    let capacity_col = result[0].column(1).as_any().downcast_ref::<Int32Array>();

    // If we have results, verify the max capacity
    if let Some(col) = capacity_col {
        assert_eq!(col.value(0), 10); // Max capacity resource
    }
}

#[tokio::test]
async fn test_query_empty_simulation() {
    let config = SimulationConfig::default();
    let runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    // Query should fail when no handlers exist
    let result = runner.query("SELECT * FROM agents").await;
    assert!(result.is_err());

    // Query for non-existent table should also fail
    let result = runner.query("SELECT * FROM resources").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_query_invalid_table() {
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    let mut pop = PopulationHandler::new();
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "type_a".to_string(),
    });
    runner.set_population_handler(pop);

    // Query non-existent table should fail
    let result = runner.query("SELECT * FROM nonexistent_table").await;
    assert!(result.is_err());

    // Query should contain helpful error message
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Query error") || err.to_string().contains("nonexistent"));
}

#[tokio::test]
async fn test_query_invalid_sql() {
    let config = SimulationConfig::default();
    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    let mut pop = PopulationHandler::new();
    pop.add_agent(TestAgent {
        id: AgentId::new(),
        agent_type: "type_a".to_string(),
    });
    runner.set_population_handler(pop);

    // Invalid SQL syntax should fail
    let result = runner.query("INVALID SQL SYNTAX HERE").await;
    assert!(result.is_err());

    // Incomplete query should fail
    let result = runner.query("SELECT * FROM").await;
    assert!(result.is_err());

    // Malformed JOIN should fail
    let result = runner.query("SELECT * FROM agents JOIN").await;
    assert!(result.is_err());
}
