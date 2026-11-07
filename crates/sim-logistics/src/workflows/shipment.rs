//! Package shipment workflow

use crate::resources::StationKind;
use chrono::{DateTime, Duration, Utc};
use sim_core::{
    Workflow, WorkflowId,
    resources::{Task, TaskFactory},
    utils::idents::{TaskId, WorkflowInstanceId},
    workflows::WorkflowStep,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Shipping one package: receive → sort → load → transit → confirm
///
/// | Step | Station | Duration |
/// |---|---|---|
/// | 0 receive | `package_receiver` | instant |
/// | 1 sort | `sorting_station` | 5 min |
/// | 2 load | `loading_dock` | 6 min |
/// | 3 transit | `driver` | 30 min |
/// | 4 confirm | `delivery_confirmer` | 1 min |
///
/// There is one definition; each package is an instance of it.
#[derive(Debug, Clone)]
pub struct PackageShipmentWorkflow {
    id: WorkflowId,
    steps: Vec<WorkflowStep>,
}

impl PackageShipmentWorkflow {
    /// Step names and durations (seconds), in order
    pub const STEPS: [(&'static str, i64); 5] = [
        ("receive", 0),
        ("sort", 300),
        ("load", 360),
        ("transit", 1800),
        ("confirm", 60),
    ];

    /// Index of the transit step (served by a driver)
    pub const TRANSIT_STEP: usize = 3;

    pub fn new() -> Self {
        let steps = Self::STEPS
            .iter()
            .zip(StationKind::PIPELINE)
            .enumerate()
            .map(|(step_index, ((name, secs), kind))| WorkflowStep {
                step_index,
                required_resource_type: kind.resource_type().to_string(),
                expected_duration: Duration::seconds(*secs),
                metadata: HashMap::from([("name".to_string(), (*name).to_string())]),
            })
            .collect();
        Self {
            // Stable across processes so snapshots restore anywhere.
            id: WorkflowId::from_uuid(Uuid::from_u128(0x7c1d_9a02_5e3b_4f6a_b8d4_2c71_e0a9_3f15)),
            steps,
        }
    }

    /// Duration of a step
    pub fn step_duration(step_index: usize) -> Duration {
        Duration::seconds(Self::STEPS[step_index].1)
    }
}

impl Default for PackageShipmentWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

impl Workflow for PackageShipmentWorkflow {
    fn id(&self) -> WorkflowId {
        self.id
    }

    fn workflow_type(&self) -> &str {
        "package_shipment"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

/// One step of a package's shipment, handled by a [`Station`](crate::Station)
#[derive(Debug, Clone)]
pub struct ShipmentTask {
    pub id: TaskId,
    pub instance_id: WorkflowInstanceId,
    pub step_index: usize,
    pub priority: i32,
    resource_type: String,
    duration: Duration,
}

impl Task for ShipmentTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.resource_type
    }

    fn estimated_duration(&self) -> Duration {
        self.duration
    }

    fn priority(&self) -> i32 {
        self.priority
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        Some(self.instance_id)
    }
}

/// Creates [`ShipmentTask`]s, carrying each package's priority
///
/// The simulation registers a package's priority under its workflow instance
/// when it starts the shipment; tasks for unknown instances get priority 0.
#[derive(Debug, Clone, Default)]
pub struct ShipmentTaskFactory {
    priorities: Arc<Mutex<HashMap<WorkflowInstanceId, i32>>>,
}

impl ShipmentTaskFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the task priority for a shipment's steps
    pub fn set_priority(&self, instance_id: WorkflowInstanceId, priority: i32) {
        self.lock().insert(instance_id, priority);
    }

    /// Forget a finished shipment
    pub fn forget(&self, instance_id: WorkflowInstanceId) {
        self.lock().remove(&instance_id);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<WorkflowInstanceId, i32>> {
        self.priorities.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl TaskFactory<ShipmentTask> for ShipmentTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>,
    ) -> ShipmentTask {
        ShipmentTask {
            id: TaskId::new(),
            instance_id,
            step_index: step.step_index,
            priority: self.lock().get(&instance_id).copied().unwrap_or(0),
            resource_type: step.required_resource_type.clone(),
            duration: step.expected_duration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_map_onto_station_kinds() {
        let wf = PackageShipmentWorkflow::new();
        let types: Vec<&str> = wf
            .steps()
            .iter()
            .map(|s| s.required_resource_type.as_str())
            .collect();
        assert_eq!(
            types,
            [
                "package_receiver",
                "sorting_station",
                "loading_dock",
                "driver",
                "delivery_confirmer"
            ]
        );
        assert_eq!(wf.steps()[3].expected_duration, Duration::minutes(30));
        assert_eq!(wf.id(), PackageShipmentWorkflow::new().id());
    }

    #[test]
    fn factory_applies_registered_priority() {
        let factory = ShipmentTaskFactory::new();
        let wf = PackageShipmentWorkflow::new();
        let (a, b) = (WorkflowInstanceId::new(), WorkflowInstanceId::new());
        factory.set_priority(a, 2);
        let now = Utc::now();
        assert_eq!(factory.create_task(a, &wf.steps()[1], now).priority(), 2);
        assert_eq!(factory.create_task(b, &wf.steps()[1], now).priority(), 0);
        factory.forget(a);
        assert_eq!(factory.create_task(a, &wf.steps()[1], now).priority(), 0);
    }
}
