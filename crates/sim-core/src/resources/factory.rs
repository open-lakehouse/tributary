//! Task factory for creating domain-specific tasks from workflow steps

use super::Task;
use crate::utils::idents::WorkflowInstanceId;
use crate::workflows::WorkflowStep;
use chrono::{DateTime, Utc};

/// Factory for creating domain-specific tasks from workflow steps
///
/// This trait allows domains to convert generic workflow steps into
/// their specific task types. The simulation runner uses this factory
/// to create tasks when a workflow instance is ready for its next step.
///
/// # Example
///
/// ```rust,ignore
/// use sim_core::resources::TaskFactory;
/// use sim_core::workflows::{WorkflowInstanceId, WorkflowStep};
///
/// struct OrderTaskFactory;
///
/// impl TaskFactory<Order> for OrderTaskFactory {
///     fn create_task(
///         &self,
///         instance_id: WorkflowInstanceId,
///         step: &WorkflowStep,
///         current_time: DateTime<Utc>
///     ) -> Order {
///         Order {
///             id: TaskId::new(),
///             workflow_instance_id: Some(instance_id),
///             required_resource_type: step.required_resource_type.clone(),
///             estimated_duration: step.expected_duration,
///             created_at: current_time,
///         }
///     }
/// }
/// ```
pub trait TaskFactory<T: Task>: Send + Sync {
    /// Create a task from a workflow step
    ///
    /// # Arguments
    ///
    /// * `instance_id` - The workflow instance this task belongs to
    /// * `step` - The workflow step definition
    /// * `current_time` - Current simulation time
    ///
    /// # Returns
    ///
    /// A new task instance configured from the workflow step
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>,
    ) -> T;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::idents::TaskId;
    use chrono::Duration;
    use std::collections::HashMap;

    #[derive(Debug)]
    struct TestTask {
        id: TaskId,
        workflow_instance_id: Option<WorkflowInstanceId>,
        required_resource_type: String,
        estimated_duration: Duration,
        created_at: DateTime<Utc>,
    }

    impl Task for TestTask {
        fn id(&self) -> TaskId {
            self.id
        }

        fn required_resource_type(&self) -> &str {
            &self.required_resource_type
        }

        fn estimated_duration(&self) -> Duration {
            self.estimated_duration
        }

        fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
            self.workflow_instance_id
        }
    }

    struct TestTaskFactory;

    impl TaskFactory<TestTask> for TestTaskFactory {
        fn create_task(
            &self,
            instance_id: WorkflowInstanceId,
            step: &WorkflowStep,
            current_time: DateTime<Utc>,
        ) -> TestTask {
            TestTask {
                id: TaskId::new(),
                workflow_instance_id: Some(instance_id),
                required_resource_type: step.required_resource_type.clone(),
                estimated_duration: step.expected_duration,
                created_at: current_time,
            }
        }
    }

    #[test]
    fn test_task_factory_creates_task_with_workflow_id() {
        let factory = TestTaskFactory;
        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "prep".to_string(),
            expected_duration: Duration::seconds(30),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, current_time);

        assert_eq!(task.workflow_instance_id(), Some(instance_id));
        assert_eq!(task.created_at, current_time);
    }

    #[test]
    fn test_task_factory_uses_step_duration() {
        let factory = TestTaskFactory;
        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "cook".to_string(),
            expected_duration: Duration::seconds(120),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, current_time);

        assert_eq!(task.estimated_duration(), Duration::seconds(120));
    }

    #[test]
    fn test_task_factory_uses_step_resource_type() {
        let factory = TestTaskFactory;
        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let step = WorkflowStep {
            step_index: 1,
            required_resource_type: "delivery".to_string(),
            expected_duration: Duration::seconds(60),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, current_time);

        assert_eq!(task.required_resource_type(), "delivery");
    }

    #[test]
    fn test_task_factory_creates_unique_task_ids() {
        let factory = TestTaskFactory;
        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "test".to_string(),
            expected_duration: Duration::seconds(10),
            metadata: HashMap::new(),
        };

        let task1 = factory.create_task(instance_id, &step, current_time);
        let task2 = factory.create_task(instance_id, &step, current_time);

        assert_ne!(task1.id(), task2.id());
    }
}
