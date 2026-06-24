//! Cross-entity SQL query interface for SimulationRunner
//!
//! This module enables JOIN queries across multiple entity types (agents, resources,
//! tasks, workflow instances) using DataFusion SQL engine.
//!
//! # Available Tables
//!
//! - **agents** - Agent population (if PopulationHandler exists)
//!   - Columns: id (binary), agent_type (string), status (string), position_x (float),
//!     position_y (float), metadata (string)
//!
//! - **resources** - Available resources (if ResourceAllocator exists)
//!   - Columns: id (binary), resource_type (string), capacity (int), available (int),
//!     position_x (float), position_y (float), metadata (string)
//!
//! - **tasks** - Task queue (if ResourceAllocator exists)
//!   - Columns: id (binary), task_type (string), status (string), assigned_resource_id (binary),
//!     workflow_instance_id (binary), priority (int), metadata (string)
//!
//! - **workflow_instances** - Workflow execution state (if WorkflowEngine exists)
//!   - Columns: id (binary), workflow_id (binary), status (string), current_step (int),
//!     total_steps (int), agent_id (binary), started_at (int), completed_at (int),
//!     context (string), metadata (string)
//!
//! # Example Queries
//!
//! ```rust,ignore
//! // Find agents with assigned tasks
//! let result = runner.query("
//!     SELECT a.id, t.task_type, t.status
//!     FROM agents a
//!     JOIN tasks t ON a.id = t.assigned_agent_id
//!     WHERE t.status = 'active'
//! ").await?;
//!
//! // Workflow progress by agent type
//! let result = runner.query("
//!     SELECT a.agent_type, w.status, COUNT(*) as count
//!     FROM workflow_instances w
//!     JOIN agents a ON w.agent_id = a.id
//!     GROUP BY a.agent_type, w.status
//! ").await?;
//!
//! // Resource utilization analysis
//! let result = runner.query("
//!     SELECT
//!         r.resource_type,
//!         r.capacity,
//!         r.available,
//!         (r.capacity - r.available) as in_use,
//!         ROUND(100.0 * (r.capacity - r.available) / r.capacity, 2) as utilization_pct
//!     FROM resources r
//!     ORDER BY utilization_pct DESC
//! ").await?;
//!
//! // Task distribution across resources
//! let result = runner.query("
//!     SELECT
//!         r.resource_type,
//!         t.status,
//!         COUNT(*) as task_count
//!     FROM tasks t
//!     JOIN resources r ON t.assigned_resource_id = r.id
//!     GROUP BY r.resource_type, t.status
//! ").await?;
//! ```

use crate::Result;
use crate::{Agent, Resource, Task, Workflow};
use arrow::record_batch::RecordBatch;
use datafusion::datasource::MemTable;
use datafusion::prelude::*;
use std::sync::Arc;

/// Execute SQL query across multiple simulation entity types
///
/// Registers available handlers as DataFusion tables and executes SQL queries
/// that can JOIN across entity types.
///
/// # Arguments
///
/// * `runner` - Reference to the SimulationRunner
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
/// let result = query_simulation(runner, "
///     SELECT a.agent_type, COUNT(*) as count
///     FROM agents a
///     JOIN tasks t ON a.id = t.assigned_agent_id
///     WHERE t.status = 'active'
///     GROUP BY a.agent_type
/// ").await?;
///
/// // Complex three-way join
/// let result = query_simulation(runner, "
///     SELECT
///         a.agent_type,
///         w.status as workflow_status,
///         t.status as task_status,
///         r.resource_type
///     FROM workflow_instances w
///     JOIN agents a ON w.agent_id = a.id
///     JOIN tasks t ON w.id = t.workflow_instance_id
///     JOIN resources r ON t.assigned_resource_id = r.id
///     WHERE w.status = 'active'
/// ").await?;
/// ```
pub async fn query_simulation<A, R, T, W>(
    runner: &crate::runner::SimulationRunner<A, R, T, W>,
    sql: &str,
) -> Result<Vec<RecordBatch>>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    let ctx = SessionContext::new();

    // Register agents table (if PopulationHandler exists)
    if let Some(ref pop) = runner.population {
        let batch = pop.export_batch();
        let schema = batch.schema();

        // Create MemTable from RecordBatch
        let mem_table = MemTable::try_new(schema, vec![vec![batch]])
            .map_err(|e| crate::Error::Query(format!("Failed to create agents table: {}", e)))?;

        ctx.register_table("agents", Arc::new(mem_table))
            .map_err(|e| crate::Error::Query(format!("Failed to register agents table: {}", e)))?;
    }

    // Register resources and tasks tables (if ResourceAllocator exists)
    if let Some(ref res) = runner.resources {
        let (resources_batch, tasks_batch) = res.export_batch();

        // Register resources table
        let resources_schema = resources_batch.schema();
        let resources_mem_table = MemTable::try_new(resources_schema, vec![vec![resources_batch]])
            .map_err(|e| crate::Error::Query(format!("Failed to create resources table: {}", e)))?;

        ctx.register_table("resources", Arc::new(resources_mem_table))
            .map_err(|e| {
                crate::Error::Query(format!("Failed to register resources table: {}", e))
            })?;

        // Register tasks table
        let tasks_schema = tasks_batch.schema();
        let tasks_mem_table = MemTable::try_new(tasks_schema, vec![vec![tasks_batch]])
            .map_err(|e| crate::Error::Query(format!("Failed to create tasks table: {}", e)))?;

        ctx.register_table("tasks", Arc::new(tasks_mem_table))
            .map_err(|e| crate::Error::Query(format!("Failed to register tasks table: {}", e)))?;
    }

    // Register workflow_instances table (if WorkflowRegistry exists)
    if let Some(ref registry) = runner.workflow_registry {
        // Collect and concatenate instances from all engines
        let batches: Vec<arrow::record_batch::RecordBatch> = registry
            .engines()
            .map(|engine| engine.instance_table.export_batch())
            .filter(|b| b.num_rows() > 0)
            .collect();

        let instances_batch = if batches.is_empty() {
            arrow::record_batch::RecordBatch::new_empty(crate::workflows::workflow_instance_schema())
        } else if batches.len() == 1 {
            batches.into_iter().next().unwrap()
        } else {
            let schema = batches[0].schema();
            arrow::compute::concat_batches(&schema, &batches).map_err(|e| {
                crate::Error::Query(format!(
                    "Failed to concatenate workflow instance batches: {}",
                    e
                ))
            })?
        };

        let instances_schema = instances_batch.schema();
        let instances_mem_table = MemTable::try_new(instances_schema, vec![vec![instances_batch]])
            .map_err(|e| {
                crate::Error::Query(format!("Failed to create workflow_instances table: {}", e))
            })?;

        ctx.register_table("workflow_instances", Arc::new(instances_mem_table))
            .map_err(|e| {
                crate::Error::Query(format!(
                    "Failed to register workflow_instances table: {}",
                    e
                ))
            })?;
    }

    // Execute SQL query
    let df = ctx
        .sql(sql)
        .await
        .map_err(|e| crate::Error::Query(format!("SQL execution failed: {}", e)))?;

    // Collect results
    let batches = df
        .collect()
        .await
        .map_err(|e| crate::Error::Query(format!("Failed to collect query results: {}", e)))?;

    Ok(batches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use crate::population::{Agent, AgentStatus, AgentTickResult, PopulationHandler};
    use crate::resources::{Resource, Task};
    use crate::runner::SimulationRunner;
    use crate::utils::idents::{AgentId, ResourceId, TaskId, WorkflowId, WorkflowInstanceId};
    use crate::workflows::{Workflow, WorkflowStep};

    use geo::Point;

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
            "test"
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

        fn tick(&mut self, _ctx: &crate::context::SimulationContext) -> Result<AgentTickResult> {
            Ok(AgentTickResult::default())
        }

        fn from_arrow_row(row: &crate::population::ArrowAgentRow) -> Result<Self> {
            let id = AgentId::from_uuid(
                uuid::Uuid::from_slice(&row.id)
                    .map_err(|e| crate::Error::Agent(format!("Invalid UUID: {}", e)))?,
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

    #[tokio::test]
    async fn test_query_empty_simulation() {
        let config = SimulationConfig::default();
        let runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
            SimulationRunner::new(config);

        // Query with no tables should fail gracefully
        let result = query_simulation(&runner, "SELECT * FROM agents").await;
        assert!(result.is_err());
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
        runner.set_population_handler(pop);

        let result = query_simulation(&runner, "SELECT * FROM agents")
            .await
            .unwrap();
        assert_eq!(result[0].num_rows(), 2);
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
        let result = query_simulation(&runner, "SELECT * FROM nonexistent").await;
        assert!(result.is_err());
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

        // Invalid SQL should fail
        let result = query_simulation(&runner, "INVALID SQL SYNTAX").await;
        assert!(result.is_err());
    }
}
