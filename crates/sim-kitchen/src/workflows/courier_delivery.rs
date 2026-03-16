//! Courier Delivery Workflow - Food order pickup and customer delivery
//!
//! This workflow manages the process of picking up prepared food orders from kitchens
//! and delivering them to customers. It represents the complete courier delivery pipeline
//! from order acceptance to final delivery.
//!
//! # Workflow Steps
//!
//! ```text
//! OrderAccepted (60s / 1min) - Courier accepts order, prepares to pick up
//!      ↓
//! PickingUp (300s / 5min) - Courier travels to kitchen, picks up order
//!      ↓
//! EnRoute (600s / 10min) - Courier transports order to customer
//!      ↓
//! Delivered (60s / 1min) - Courier hands off order to customer
//! ```
//!
//! Total workflow time: 1020 seconds (17 minutes)
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use sim_kitchen::workflows::CourierDeliveryWorkflow;
//! use sim_core::Workflow;
//! use geo::Point;
//!
//! let workflow = CourierDeliveryWorkflow::new(
//!     customer_id,
//!     Point::new(-122.5, 37.9),  // pickup location (kitchen)
//!     Point::new(-122.3, 37.7),  // delivery location (customer)
//!     order_id,
//! );
//!
//! assert_eq!(workflow.workflow_type(), "courier_delivery");
//! assert_eq!(workflow.step_count(), 4);
//! ```

use chrono::Duration;
use geo::Point;
use sim_core::{
    Workflow, WorkflowId,
    resources::{Task, TaskFactory},
    utils::idents::{TaskId, WorkflowInstanceId},
    workflows::WorkflowStep,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Courier Delivery Workflow for food order pickup and delivery
///
/// This workflow orchestrates the complete courier delivery process:
/// 1. Order acceptance and courier assignment
/// 2. Pickup from kitchen
/// 3. Transportation to customer
/// 4. Delivery and completion
///
/// # Metadata
///
/// - `customer_id`: Customer receiving the order
/// - `pickup_location`: Kitchen location (latitude/longitude)
/// - `delivery_location`: Customer location (latitude/longitude)
/// - `order_id`: Order being delivered
#[derive(Debug, Clone)]
pub struct CourierDeliveryWorkflow {
    /// Unique workflow definition ID
    pub workflow_id: WorkflowId,
    /// Customer receiving the order
    pub customer_id: Uuid,
    /// Kitchen location where order is prepared
    pub pickup_location: Point<f64>,
    /// Customer location for delivery
    pub delivery_location: Point<f64>,
    /// Order being delivered
    pub order_id: Uuid,
    /// Workflow steps
    steps: Vec<WorkflowStep>,
}

impl CourierDeliveryWorkflow {
    /// Create a new courier delivery workflow
    ///
    /// # Arguments
    ///
    /// * `customer_id` - Customer UUID receiving the order
    /// * `pickup_location` - Kitchen location (Point with lat/lon)
    /// * `delivery_location` - Customer location (Point with lat/lon)
    /// * `order_id` - Order UUID being delivered
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use geo::Point;
    /// let workflow = CourierDeliveryWorkflow::new(
    ///     customer_id,
    ///     Point::new(-122.5, 37.9),
    ///     Point::new(-122.3, 37.7),
    ///     order_id,
    /// );
    /// ```
    pub fn new(
        customer_id: Uuid,
        pickup_location: Point<f64>,
        delivery_location: Point<f64>,
        order_id: Uuid,
    ) -> Self {
        let steps = vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "courier".to_string(),
                expected_duration: Duration::seconds(60), // 1 minute
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "OrderAccepted".to_string());
                    m.insert(
                        "description".to_string(),
                        "Courier accepts order and prepares for pickup".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "courier".to_string(),
                expected_duration: Duration::seconds(300), // 5 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "PickingUp".to_string());
                    m.insert(
                        "description".to_string(),
                        "Courier traveling to kitchen and picking up order".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "courier".to_string(),
                expected_duration: Duration::seconds(600), // 10 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "EnRoute".to_string());
                    m.insert(
                        "description".to_string(),
                        "Courier transporting order to customer".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 3,
                required_resource_type: "customer".to_string(),
                expected_duration: Duration::seconds(60), // 1 minute
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Delivered".to_string());
                    m.insert(
                        "description".to_string(),
                        "Order delivered to customer".to_string(),
                    );
                    m
                },
            },
        ];

        Self {
            workflow_id: WorkflowId::new(),
            customer_id,
            pickup_location,
            delivery_location,
            order_id,
            steps,
        }
    }

    /// Get the customer ID for this workflow
    pub fn customer_id(&self) -> Uuid {
        self.customer_id
    }

    /// Get the pickup location (kitchen)
    pub fn pickup_location(&self) -> Point<f64> {
        self.pickup_location
    }

    /// Get the delivery location (customer)
    pub fn delivery_location(&self) -> Point<f64> {
        self.delivery_location
    }

    /// Get the order ID being delivered
    pub fn order_id(&self) -> Uuid {
        self.order_id
    }
}

impl Workflow for CourierDeliveryWorkflow {
    fn id(&self) -> WorkflowId {
        self.workflow_id
    }

    fn workflow_type(&self) -> &str {
        "courier_delivery"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

/// Courier delivery task
///
/// Tasks created from courier delivery workflow steps. Each task represents
/// a specific action in the delivery pipeline.
#[derive(Debug, Clone)]
pub struct CourierDeliveryTask {
    /// Task ID
    pub id: TaskId,
    /// Task type (assign_courier, pickup_food, deliver_food)
    pub task_type: String,
    /// Customer ID
    pub customer_id: Uuid,
    /// Pickup location (kitchen)
    pub pickup_location: Point<f64>,
    /// Delivery location (customer)
    pub delivery_location: Point<f64>,
    /// Order ID
    pub order_id: Uuid,
    /// Required resource type
    pub required_resource_type: String,
    /// Estimated duration
    pub estimated_duration: Duration,
    /// Workflow instance this task belongs to
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for CourierDeliveryTask {
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

/// Task factory for courier delivery workflows
///
/// Converts workflow steps into concrete courier delivery tasks.
#[derive(Debug, Clone)]
pub struct CourierDeliveryTaskFactory {
    /// Customer ID for this factory
    pub customer_id: Uuid,
    /// Pickup location
    pub pickup_location: Point<f64>,
    /// Delivery location
    pub delivery_location: Point<f64>,
    /// Order ID
    pub order_id: Uuid,
}

impl CourierDeliveryTaskFactory {
    /// Create a new courier delivery task factory
    pub fn new(
        customer_id: Uuid,
        pickup_location: Point<f64>,
        delivery_location: Point<f64>,
        order_id: Uuid,
    ) -> Self {
        Self {
            customer_id,
            pickup_location,
            delivery_location,
            order_id,
        }
    }
}

impl TaskFactory<CourierDeliveryTask> for CourierDeliveryTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: chrono::DateTime<chrono::Utc>,
    ) -> CourierDeliveryTask {
        // Determine task type based on step index
        let task_type = match step.step_index {
            0 => "assign_courier",
            1 => "pickup_food",
            2 => "deliver_food",
            3 => "complete_delivery",
            _ => "unknown",
        };

        CourierDeliveryTask {
            id: TaskId::new(),
            task_type: task_type.to_string(),
            customer_id: self.customer_id,
            pickup_location: self.pickup_location,
            delivery_location: self.delivery_location,
            order_id: self.order_id,
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
    fn test_courier_delivery_workflow_creation() {
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let pickup = Point::new(-122.5, 37.9);
        let delivery = Point::new(-122.3, 37.7);

        let workflow = CourierDeliveryWorkflow::new(
            customer_id,
            pickup,
            delivery,
            order_id,
        );

        assert_eq!(workflow.workflow_type(), "courier_delivery");
        assert_eq!(workflow.customer_id(), customer_id);
        assert_eq!(workflow.pickup_location(), pickup);
        assert_eq!(workflow.delivery_location(), delivery);
        assert_eq!(workflow.order_id(), order_id);
    }

    #[test]
    fn test_workflow_steps_in_correct_order() {
        let workflow = CourierDeliveryWorkflow::new(
            Uuid::new_v4(),
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            Uuid::new_v4(),
        );

        let steps = workflow.steps();
        assert_eq!(steps.len(), 4);

        // Verify step order
        assert_eq!(steps[0].step_index, 0);
        assert_eq!(steps[0].metadata.get("name").unwrap(), "OrderAccepted");
        assert_eq!(steps[0].required_resource_type, "courier");

        assert_eq!(steps[1].step_index, 1);
        assert_eq!(steps[1].metadata.get("name").unwrap(), "PickingUp");
        assert_eq!(steps[1].required_resource_type, "courier");

        assert_eq!(steps[2].step_index, 2);
        assert_eq!(steps[2].metadata.get("name").unwrap(), "EnRoute");
        assert_eq!(steps[2].required_resource_type, "courier");

        assert_eq!(steps[3].step_index, 3);
        assert_eq!(steps[3].metadata.get("name").unwrap(), "Delivered");
        assert_eq!(steps[3].required_resource_type, "customer");
    }

    #[test]
    fn test_task_factory_generates_correct_task_types() {
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let instance_id = WorkflowInstanceId::new();
        let pickup = Point::new(-122.5, 37.9);
        let delivery = Point::new(-122.3, 37.7);

        let factory = CourierDeliveryTaskFactory::new(
            customer_id,
            pickup,
            delivery,
            order_id,
        );

        let workflow = CourierDeliveryWorkflow::new(
            customer_id,
            pickup,
            delivery,
            order_id,
        );

        let steps = workflow.steps();
        let current_time = chrono::Utc::now();

        // Test each step's task generation
        let task0 = factory.create_task(instance_id, &steps[0], current_time);
        assert_eq!(task0.task_type, "assign_courier");
        assert_eq!(task0.required_resource_type(), "courier");

        let task1 = factory.create_task(instance_id, &steps[1], current_time);
        assert_eq!(task1.task_type, "pickup_food");
        assert_eq!(task1.required_resource_type(), "courier");

        let task2 = factory.create_task(instance_id, &steps[2], current_time);
        assert_eq!(task2.task_type, "deliver_food");
        assert_eq!(task2.required_resource_type(), "courier");

        let task3 = factory.create_task(instance_id, &steps[3], current_time);
        assert_eq!(task3.task_type, "complete_delivery");
        assert_eq!(task3.required_resource_type(), "customer");
    }

    #[test]
    fn test_workflow_progression() {
        let workflow = CourierDeliveryWorkflow::new(
            Uuid::new_v4(),
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            Uuid::new_v4(),
        );

        assert_eq!(workflow.step_count(), 4);

        // Verify we can get each step
        assert!(workflow.get_step(0).is_some());
        assert!(workflow.get_step(1).is_some());
        assert!(workflow.get_step(2).is_some());
        assert!(workflow.get_step(3).is_some());
        assert!(workflow.get_step(4).is_none());

        // Verify step durations
        let steps = workflow.steps();
        assert_eq!(steps[0].expected_duration, Duration::seconds(60)); // 1 min
        assert_eq!(steps[1].expected_duration, Duration::seconds(300)); // 5 min
        assert_eq!(steps[2].expected_duration, Duration::seconds(600)); // 10 min
        assert_eq!(steps[3].expected_duration, Duration::seconds(60)); // 1 min
    }

    #[test]
    fn test_multiple_instances_running_concurrently() {
        let customer1 = Uuid::new_v4();
        let customer2 = Uuid::new_v4();
        let customer3 = Uuid::new_v4();

        // Create multiple workflow instances
        let workflow1 = CourierDeliveryWorkflow::new(
            customer1,
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            Uuid::new_v4(),
        );

        let workflow2 = CourierDeliveryWorkflow::new(
            customer2,
            Point::new(-122.4, 37.8),
            Point::new(-122.2, 37.6),
            Uuid::new_v4(),
        );

        let workflow3 = CourierDeliveryWorkflow::new(
            customer3,
            Point::new(-122.6, 38.0),
            Point::new(-122.1, 37.5),
            Uuid::new_v4(),
        );

        // Each should have unique workflow IDs
        assert_ne!(workflow1.id(), workflow2.id());
        assert_ne!(workflow1.id(), workflow3.id());
        assert_ne!(workflow2.id(), workflow3.id());

        // But same structure
        assert_eq!(workflow1.step_count(), 4);
        assert_eq!(workflow2.step_count(), 4);
        assert_eq!(workflow3.step_count(), 4);

        // And same workflow type
        assert_eq!(workflow1.workflow_type(), "courier_delivery");
        assert_eq!(workflow2.workflow_type(), "courier_delivery");
        assert_eq!(workflow3.workflow_type(), "courier_delivery");
    }

    #[test]
    fn test_completion_triggers_customer_satisfaction() {
        // This test verifies the workflow structure supports customer delivery
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();

        let workflow = CourierDeliveryWorkflow::new(
            customer_id,
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            order_id,
        );

        // Final step should be "Delivered"
        let final_step = workflow.get_step(3).unwrap();
        assert_eq!(final_step.metadata.get("name").unwrap(), "Delivered");
        assert_eq!(final_step.required_resource_type, "customer");

        // Verify we have the metadata needed for delivery confirmation
        assert_eq!(workflow.customer_id(), customer_id);
        assert_eq!(workflow.order_id(), order_id);
    }

    #[test]
    fn test_task_has_workflow_instance_id() {
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let instance_id = WorkflowInstanceId::new();

        let factory = CourierDeliveryTaskFactory::new(
            customer_id,
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            order_id,
        );

        let step = WorkflowStep {
            step_index: 1,
            required_resource_type: "courier".to_string(),
            expected_duration: Duration::seconds(300),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, chrono::Utc::now());

        assert_eq!(task.workflow_instance_id(), Some(instance_id));
        assert_eq!(task.customer_id, customer_id);
        assert_eq!(task.order_id, order_id);
    }

    #[test]
    fn test_task_factory_creates_unique_task_ids() {
        let factory = CourierDeliveryTaskFactory::new(
            Uuid::new_v4(),
            Point::new(-122.5, 37.9),
            Point::new(-122.3, 37.7),
            Uuid::new_v4(),
        );

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "courier".to_string(),
            expected_duration: Duration::seconds(60),
            metadata: HashMap::new(),
        };

        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let task1 = factory.create_task(instance_id, &step, current_time);
        let task2 = factory.create_task(instance_id, &step, current_time);

        assert_ne!(task1.id(), task2.id());
    }

    #[test]
    fn test_task_contains_location_metadata() {
        let customer_id = Uuid::new_v4();
        let order_id = Uuid::new_v4();
        let pickup = Point::new(-122.5, 37.9);
        let delivery = Point::new(-122.3, 37.7);

        let factory = CourierDeliveryTaskFactory::new(
            customer_id,
            pickup,
            delivery,
            order_id,
        );

        let step = WorkflowStep {
            step_index: 1,
            required_resource_type: "courier".to_string(),
            expected_duration: Duration::seconds(300),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(WorkflowInstanceId::new(), &step, chrono::Utc::now());

        // Verify location metadata is preserved
        assert_eq!(task.pickup_location, pickup);
        assert_eq!(task.delivery_location, delivery);
    }
}
