//! Kitchens and the preparation tasks they process

use chrono::{DateTime, Duration, Utc};
use sim_core::resources::TaskFactory;
use sim_core::utils::idents::WorkflowInstanceId;
use sim_core::workflows::WorkflowStep;
use sim_core::{Resource, ResourceId, Task, TaskId};
use uuid::Uuid;

/// Namespace for deterministic kitchen-domain IDs (UUID v5)
pub(crate) const KITCHEN_NAMESPACE: Uuid =
    Uuid::from_u128(0x5f1c_7e0a_2b4d_4c11_9a3e_6d2f_8b70_c4e1);

/// A kitchen: a capacity-constrained resource that prepares orders
///
/// `capacity` is the number of orders the kitchen works on at once. Each
/// kitchen has its own resource type (`kitchen:<name>`), so an order's
/// preparation steps all run in the kitchen it was routed to. The
/// [`ResourceAllocator`](sim_core::ResourceAllocator) tracks load.
///
/// # Example
///
/// ```rust
/// use sim_kitchen::Kitchen;
/// use sim_core::Resource;
///
/// let kitchen = Kitchen::new("Kitchen-1", 5);
/// assert_eq!(kitchen.resource_type(), "kitchen:Kitchen-1");
/// assert_eq!(kitchen.capacity(), 5);
/// // IDs are derived from the name, so they are stable across runs.
/// assert_eq!(kitchen.id, Kitchen::new("Kitchen-1", 5).id);
/// ```
#[derive(Debug, Clone)]
pub struct Kitchen {
    pub id: ResourceId,
    pub name: String,
    pub capacity: usize,
    resource_type: String,
}

impl Kitchen {
    /// Create a kitchen that works on up to `capacity` orders at once
    pub fn new(name: impl Into<String>, capacity: usize) -> Self {
        let name = name.into();
        Self {
            id: ResourceId::from_name(KITCHEN_NAMESPACE, &format!("kitchen/{name}")),
            resource_type: format!("kitchen:{name}"),
            name,
            capacity,
        }
    }
}

impl Resource for Kitchen {
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
        // The allocator tracks load; a kitchen starts empty.
        0
    }
}

/// One step of preparing an order (prep, cook or package) in a kitchen
#[derive(Debug, Clone)]
pub struct PrepTask {
    pub id: TaskId,
    pub instance_id: WorkflowInstanceId,
    pub step_index: usize,
    resource_type: String,
    duration: Duration,
}

impl Task for PrepTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.resource_type
    }

    fn estimated_duration(&self) -> Duration {
        self.duration
    }

    /// Later steps outrank earlier ones, so an order already in progress
    /// takes the slot it just freed before a new order can start. This keeps
    /// the number of orders in progress per kitchen at or below its capacity.
    fn priority(&self) -> i32 {
        i32::try_from(self.step_index).unwrap_or(i32::MAX)
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        Some(self.instance_id)
    }
}

/// Creates a [`PrepTask`] for each food-preparation workflow step
#[derive(Debug, Default, Clone, Copy)]
pub struct PrepTaskFactory;

impl TaskFactory<PrepTask> for PrepTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>,
    ) -> PrepTask {
        PrepTask {
            id: TaskId::new(),
            instance_id,
            step_index: step.step_index,
            resource_type: step.required_resource_type.clone(),
            duration: step.expected_duration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn kitchen_has_its_own_resource_type() {
        let a = Kitchen::new("A", 5);
        let b = Kitchen::new("B", 5);
        assert_ne!(a.resource_type(), b.resource_type());
        assert_ne!(a.id, b.id);
        assert_eq!(a.available_capacity(), 5);
    }

    #[test]
    fn factory_copies_step_and_ranks_later_steps_higher() {
        let step = |i: usize| WorkflowStep {
            step_index: i,
            required_resource_type: "kitchen:A".to_string(),
            expected_duration: Duration::seconds(60),
            metadata: HashMap::new(),
        };
        let instance = WorkflowInstanceId::new();
        let first = PrepTaskFactory.create_task(instance, &step(0), Utc::now());
        let last = PrepTaskFactory.create_task(instance, &step(2), Utc::now());
        assert_eq!(first.required_resource_type(), "kitchen:A");
        assert_eq!(first.estimated_duration(), Duration::seconds(60));
        assert_eq!(first.workflow_instance_id(), Some(instance));
        assert!(last.priority() > first.priority());
    }
}
