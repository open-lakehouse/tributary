//! Package Shipment Workflow
//!
//! This workflow manages the complete package delivery process from warehouse
//! arrival through sorting, loading, transit, and final delivery.
//!
//! # Workflow Steps
//!
//! ```text
//! PackageReceived (0s)
//!      ↓
//! Sorting (300s / 5min) - Sort package at warehouse
//!      ↓
//! Loading (360s / 6min) - Load package onto delivery vehicle
//!      ↓
//! InTransit (1800s / 30min) - Driver delivers to destination
//!      ↓
//! Delivered (60s / 1min) - Confirm delivery
//! ```
//!
//! Total workflow time: ~2520 seconds (42 minutes)

use chrono::Duration;
use geo::Point;
use sim_core::{
    Workflow, WorkflowId,
    resources::{Task, TaskFactory},
    utils::idents::{TaskId, WorkflowInstanceId},
    workflows::WorkflowStep,
};
use std::collections::HashMap;

/// Package Shipment Workflow
///
/// Orchestrates the complete package delivery lifecycle:
/// 1. Package arrival at warehouse
/// 2. Sorting and routing
/// 3. Loading onto delivery vehicle
/// 4. Transit to destination
/// 5. Final delivery confirmation
#[derive(Debug, Clone)]
pub struct PackageShipmentWorkflow {
    /// Unique workflow definition ID
    pub workflow_id: WorkflowId,
    /// Package tracking number
    pub tracking_number: String,
    /// Warehouse where package arrives
    pub warehouse_id: String,
    /// Delivery destination
    pub destination: Point<f64>,
    /// Package weight (kg)
    pub weight_kg: f64,
    /// Workflow steps
    steps: Vec<WorkflowStep>,
}

impl PackageShipmentWorkflow {
    /// Create a new package shipment workflow
    ///
    /// # Arguments
    ///
    /// * `tracking_number` - Package tracking identifier
    /// * `warehouse_id` - Warehouse UUID
    /// * `destination` - Delivery address coordinates
    /// * `weight_kg` - Package weight in kilograms
    pub fn new(
        tracking_number: String,
        warehouse_id: String,
        destination: Point<f64>,
        weight_kg: f64,
    ) -> Self {
        let steps = vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "package_receiver".to_string(),
                expected_duration: Duration::seconds(0), // Instant
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "PackageReceived".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package arrives at warehouse".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "sorting_station".to_string(),
                expected_duration: Duration::seconds(300), // 5 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Sorting".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package sorted and routed for delivery".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "loading_dock".to_string(),
                expected_duration: Duration::seconds(360), // 6 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Loading".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package loaded onto delivery vehicle".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 3,
                required_resource_type: "driver".to_string(),
                expected_duration: Duration::seconds(1800), // 30 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "InTransit".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package in transit to destination".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 4,
                required_resource_type: "delivery_confirmer".to_string(),
                expected_duration: Duration::seconds(60), // 1 minute
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Delivered".to_string());
                    m.insert(
                        "description".to_string(),
                        "Package delivered and confirmed".to_string(),
                    );
                    m
                },
            },
        ];

        Self {
            workflow_id: WorkflowId::new(),
            tracking_number,
            warehouse_id,
            destination,
            weight_kg,
            steps,
        }
    }

    /// Get the tracking number
    pub fn tracking_number(&self) -> &str {
        &self.tracking_number
    }

    /// Get the warehouse ID
    pub fn warehouse_id(&self) -> &str {
        &self.warehouse_id
    }

    /// Get the destination
    pub fn destination(&self) -> Point<f64> {
        self.destination
    }

    /// Get the package weight
    pub fn weight_kg(&self) -> f64 {
        self.weight_kg
    }
}

impl Workflow for PackageShipmentWorkflow {
    fn id(&self) -> WorkflowId {
        self.workflow_id
    }

    fn workflow_type(&self) -> &str {
        "package_shipment"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

/// Package shipment task
///
/// Tasks created from shipment workflow steps.
#[derive(Debug, Clone)]
pub struct ShipmentTask {
    /// Task ID
    pub id: TaskId,
    /// Task type (receive, sort, load, transit, deliver)
    pub task_type: String,
    /// Package tracking number
    pub tracking_number: String,
    /// Package weight
    pub weight_kg: f64,
    /// Warehouse ID
    pub warehouse_id: String,
    /// Destination
    pub destination: Point<f64>,
    /// Required resource type
    pub required_resource_type: String,
    /// Estimated duration
    pub estimated_duration: Duration,
    /// Workflow instance this task belongs to
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for ShipmentTask {
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

/// Task factory for package shipment workflows
#[derive(Debug, Clone)]
pub struct ShipmentTaskFactory {
    /// Tracking number
    pub tracking_number: String,
    /// Weight
    pub weight_kg: f64,
    /// Warehouse ID
    pub warehouse_id: String,
    /// Destination
    pub destination: Point<f64>,
}

impl ShipmentTaskFactory {
    /// Create a new shipment task factory
    pub fn new(
        tracking_number: String,
        weight_kg: f64,
        warehouse_id: String,
        destination: Point<f64>,
    ) -> Self {
        Self {
            tracking_number,
            weight_kg,
            warehouse_id,
            destination,
        }
    }
}

impl TaskFactory<ShipmentTask> for ShipmentTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: chrono::DateTime<chrono::Utc>,
    ) -> ShipmentTask {
        // Determine task type based on step index
        let task_type = match step.step_index {
            0 => "receive_package",
            1 => "sort_package",
            2 => "load_package",
            3 => "transit_package",
            4 => "deliver_package",
            _ => "unknown",
        };

        ShipmentTask {
            id: TaskId::new(),
            task_type: task_type.to_string(),
            tracking_number: self.tracking_number.clone(),
            weight_kg: self.weight_kg,
            warehouse_id: self.warehouse_id.clone(),
            destination: self.destination,
            required_resource_type: step.required_resource_type.clone(),
            estimated_duration: step.expected_duration,
            workflow_instance_id: Some(instance_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shipment_workflow_creation() {
        let warehouse_id = "warehouse-1".to_string();
        let destination = Point::new(-122.4, 37.8);

        let workflow = PackageShipmentWorkflow::new(
            "PKG-123".to_string(),
            warehouse_id.clone(),
            destination,
            2.5,
        );

        assert_eq!(workflow.workflow_type(), "package_shipment");
        assert_eq!(workflow.tracking_number(), "PKG-123");
        assert_eq!(workflow.warehouse_id(), &warehouse_id);
        assert_eq!(workflow.weight_kg(), 2.5);
        assert_eq!(workflow.destination(), destination);
    }

    #[test]
    fn test_workflow_steps_in_correct_order() {
        let workflow = PackageShipmentWorkflow::new(
            "PKG-456".to_string(),
            "w1".to_string(),
            Point::new(0.0, 0.0),
            1.0,
        );

        let steps = workflow.steps();
        assert_eq!(steps.len(), 5);

        // Verify step order
        assert_eq!(steps[0].step_index, 0);
        assert_eq!(steps[0].metadata.get("name").unwrap(), "PackageReceived");
        assert_eq!(steps[0].required_resource_type, "package_receiver");

        assert_eq!(steps[1].step_index, 1);
        assert_eq!(steps[1].metadata.get("name").unwrap(), "Sorting");
        assert_eq!(steps[1].required_resource_type, "sorting_station");

        assert_eq!(steps[2].step_index, 2);
        assert_eq!(steps[2].metadata.get("name").unwrap(), "Loading");
        assert_eq!(steps[2].required_resource_type, "loading_dock");

        assert_eq!(steps[3].step_index, 3);
        assert_eq!(steps[3].metadata.get("name").unwrap(), "InTransit");
        assert_eq!(steps[3].required_resource_type, "driver");

        assert_eq!(steps[4].step_index, 4);
        assert_eq!(steps[4].metadata.get("name").unwrap(), "Delivered");
        assert_eq!(steps[4].required_resource_type, "delivery_confirmer");
    }

    #[test]
    fn test_task_factory_generates_correct_task_types() {
        let instance_id = WorkflowInstanceId::new();
        let destination = Point::new(1.0, 2.0);

        let factory = ShipmentTaskFactory::new(
            "PKG-789".to_string(),
            3.5,
            "warehouse-2".to_string(),
            destination,
        );

        let workflow = PackageShipmentWorkflow::new(
            "PKG-789".to_string(),
            "warehouse-2".to_string(),
            destination,
            3.5,
        );

        let steps = workflow.steps();
        let current_time = chrono::Utc::now();

        // Test each step's task generation
        let task0 = factory.create_task(instance_id, &steps[0], current_time);
        assert_eq!(task0.task_type, "receive_package");
        assert_eq!(task0.required_resource_type(), "package_receiver");

        let task1 = factory.create_task(instance_id, &steps[1], current_time);
        assert_eq!(task1.task_type, "sort_package");
        assert_eq!(task1.required_resource_type(), "sorting_station");

        let task2 = factory.create_task(instance_id, &steps[2], current_time);
        assert_eq!(task2.task_type, "load_package");
        assert_eq!(task2.required_resource_type(), "loading_dock");

        let task3 = factory.create_task(instance_id, &steps[3], current_time);
        assert_eq!(task3.task_type, "transit_package");
        assert_eq!(task3.required_resource_type(), "driver");

        let task4 = factory.create_task(instance_id, &steps[4], current_time);
        assert_eq!(task4.task_type, "deliver_package");
        assert_eq!(task4.required_resource_type(), "delivery_confirmer");
    }

    #[test]
    fn test_workflow_progression() {
        let workflow = PackageShipmentWorkflow::new(
            "PKG-111".to_string(),
            "w1".to_string(),
            Point::new(0.0, 0.0),
            1.5,
        );

        assert_eq!(workflow.step_count(), 5);

        // Verify we can get each step
        assert!(workflow.get_step(0).is_some());
        assert!(workflow.get_step(1).is_some());
        assert!(workflow.get_step(2).is_some());
        assert!(workflow.get_step(3).is_some());
        assert!(workflow.get_step(4).is_some());
        assert!(workflow.get_step(5).is_none());

        // Verify step durations
        let steps = workflow.steps();
        assert_eq!(steps[0].expected_duration, Duration::seconds(0)); // Instant
        assert_eq!(steps[1].expected_duration, Duration::seconds(300)); // 5 min
        assert_eq!(steps[2].expected_duration, Duration::seconds(360)); // 6 min
        assert_eq!(steps[3].expected_duration, Duration::seconds(1800)); // 30 min
        assert_eq!(steps[4].expected_duration, Duration::seconds(60)); // 1 min
    }

    #[test]
    fn test_multiple_instances_running_concurrently() {
        let dest1 = Point::new(1.0, 1.0);
        let dest2 = Point::new(2.0, 2.0);
        let dest3 = Point::new(3.0, 3.0);

        // Create multiple workflow instances
        let workflow1 =
            PackageShipmentWorkflow::new("PKG-001".to_string(), "w1".to_string(), dest1, 1.0);

        let workflow2 =
            PackageShipmentWorkflow::new("PKG-002".to_string(), "w1".to_string(), dest2, 2.0);

        let workflow3 =
            PackageShipmentWorkflow::new("PKG-003".to_string(), "w2".to_string(), dest3, 3.0);

        // Each should have unique workflow IDs
        assert_ne!(workflow1.id(), workflow2.id());
        assert_ne!(workflow1.id(), workflow3.id());
        assert_ne!(workflow2.id(), workflow3.id());

        // But same structure
        assert_eq!(workflow1.step_count(), 5);
        assert_eq!(workflow2.step_count(), 5);
        assert_eq!(workflow3.step_count(), 5);

        // And same workflow type
        assert_eq!(workflow1.workflow_type(), "package_shipment");
        assert_eq!(workflow2.workflow_type(), "package_shipment");
        assert_eq!(workflow3.workflow_type(), "package_shipment");
    }

    #[test]
    fn test_completion_triggers_delivery_confirmation() {
        let destination = Point::new(5.0, 5.0);
        let workflow = PackageShipmentWorkflow::new(
            "PKG-FINAL".to_string(),
            "w1".to_string(),
            destination,
            1.0,
        );

        // Final step should be "Delivered"
        let final_step = workflow.get_step(4).unwrap();
        assert_eq!(final_step.metadata.get("name").unwrap(), "Delivered");
        assert_eq!(final_step.required_resource_type, "delivery_confirmer");

        // Verify we have the metadata needed for delivery confirmation
        assert_eq!(workflow.tracking_number(), "PKG-FINAL");
        assert_eq!(workflow.destination(), destination);
    }

    #[test]
    fn test_task_has_workflow_instance_id() {
        let instance_id = WorkflowInstanceId::new();
        let destination = Point::new(1.0, 1.0);

        let factory =
            ShipmentTaskFactory::new("PKG-TEST".to_string(), 2.0, "w1".to_string(), destination);

        let step = WorkflowStep {
            step_index: 1,
            required_resource_type: "sorting_station".to_string(),
            expected_duration: Duration::seconds(300),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, chrono::Utc::now());

        assert_eq!(task.workflow_instance_id(), Some(instance_id));
        assert_eq!(task.tracking_number, "PKG-TEST");
        assert_eq!(task.weight_kg, 2.0);
    }

    #[test]
    fn test_task_factory_creates_unique_task_ids() {
        let destination = Point::new(0.0, 0.0);
        let factory =
            ShipmentTaskFactory::new("PKG-UNIQUE".to_string(), 1.0, "w1".to_string(), destination);

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "package_receiver".to_string(),
            expected_duration: Duration::seconds(0),
            metadata: HashMap::new(),
        };

        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let task1 = factory.create_task(instance_id, &step, current_time);
        let task2 = factory.create_task(instance_id, &step, current_time);

        assert_ne!(task1.id(), task2.id());
    }

    #[test]
    fn test_total_workflow_duration() {
        let workflow = PackageShipmentWorkflow::new(
            "PKG-TIMING".to_string(),
            "w1".to_string(),
            Point::new(0.0, 0.0),
            1.0,
        );

        // Calculate total expected duration
        let total_seconds: i64 = workflow
            .steps()
            .iter()
            .map(|step| step.expected_duration.num_seconds())
            .sum();

        // 0 + 300 + 360 + 1800 + 60 = 2520 seconds (42 minutes)
        assert_eq!(total_seconds, 2520);
    }

    #[test]
    fn test_workflow_ids_are_unique() {
        let workflows: Vec<PackageShipmentWorkflow> = (0..10)
            .map(|i| {
                PackageShipmentWorkflow::new(
                    format!("PKG-{}", i),
                    "w1".to_string(),
                    Point::new(0.0, 0.0),
                    1.0,
                )
            })
            .collect();

        // Check all IDs are unique
        for i in 0..workflows.len() {
            for j in (i + 1)..workflows.len() {
                assert_ne!(workflows[i].id(), workflows[j].id());
            }
        }
    }

    #[test]
    fn test_task_preserves_package_metadata() {
        let destination = Point::new(10.0, 20.0);
        let factory = ShipmentTaskFactory::new(
            "PKG-META".to_string(),
            5.5,
            "warehouse-main".to_string(),
            destination,
        );

        let step = WorkflowStep {
            step_index: 2,
            required_resource_type: "loading_dock".to_string(),
            expected_duration: Duration::seconds(360),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(WorkflowInstanceId::new(), &step, chrono::Utc::now());

        assert_eq!(task.tracking_number, "PKG-META");
        assert_eq!(task.weight_kg, 5.5);
        assert_eq!(task.warehouse_id, "warehouse-main");
        assert_eq!(task.destination, destination);
    }
}
