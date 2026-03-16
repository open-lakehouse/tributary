//! Supply Chain Workflow - Inventory ordering and delivery
//!
//! This workflow manages the process of ordering inventory from distribution sites
//! and delivering it to kitchens. It represents the complete supply chain pipeline
//! from order placement to final delivery.
//!
//! # Workflow Steps
//!
//! ```text
//! OrderPlaced (0s)
//!      ↓
//! DriverAssigned (300s / 5min) - Find and assign delivery driver
//!      ↓
//! PickingUp (600s / 10min) - Driver travels to distribution site, loads inventory
//!      ↓
//! InTransit (1800s / 30min) - Driver transports inventory to kitchen
//!      ↓
//! Delivered (300s / 5min) - Unload and verify inventory
//! ```
//!
//! Total workflow time: 3000 seconds (50 minutes)
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use sim_kitchen::workflows::SupplyChainWorkflow;
//! use sim_core::Workflow;
//!
//! let workflow = SupplyChainWorkflow::new(
//!     "flour".to_string(),
//!     100,
//!     source_site_id,
//!     destination_kitchen_id,
//! );
//!
//! assert_eq!(workflow.workflow_type(), "supply_chain");
//! assert_eq!(workflow.step_count(), 5);
//! ```

use chrono::Duration;
use sim_core::{
    Workflow, WorkflowId,
    resources::{Task, TaskFactory},
    utils::idents::{TaskId, WorkflowInstanceId},
    workflows::WorkflowStep,
};
use std::collections::HashMap;
use uuid::Uuid;

/// Supply Chain Workflow for inventory ordering and delivery
///
/// This workflow orchestrates the complete supply chain process:
/// 1. Order placement for low inventory items
/// 2. Driver assignment for delivery
/// 3. Pickup of inventory from distribution site
/// 4. Transportation to destination kitchen
/// 5. Delivery and inventory update
///
/// # Metadata
///
/// - `item_type`: Type of inventory item (e.g., "flour", "tomatoes")
/// - `quantity`: Amount to order (units)
/// - `source_site_id`: Distribution site ID
/// - `destination_kitchen_id`: Target kitchen ID
#[derive(Debug, Clone)]
pub struct SupplyChainWorkflow {
    /// Unique workflow definition ID
    pub workflow_id: WorkflowId,
    /// Type of inventory item being ordered
    pub item_type: String,
    /// Quantity to order
    pub quantity: u32,
    /// Source distribution site
    pub source_site_id: Uuid,
    /// Destination kitchen
    pub destination_kitchen_id: Uuid,
    /// Workflow steps
    steps: Vec<WorkflowStep>,
}

impl SupplyChainWorkflow {
    /// Create a new supply chain workflow
    ///
    /// # Arguments
    ///
    /// * `item_type` - Type of inventory item (e.g., "flour", "vegetables")
    /// * `quantity` - Amount to order
    /// * `source_site_id` - Distribution site UUID
    /// * `destination_kitchen_id` - Kitchen UUID
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let workflow = SupplyChainWorkflow::new(
    ///     "flour".to_string(),
    ///     100,
    ///     source_id,
    ///     kitchen_id,
    /// );
    /// ```
    pub fn new(
        item_type: String,
        quantity: u32,
        source_site_id: Uuid,
        destination_kitchen_id: Uuid,
    ) -> Self {
        let steps = vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "order_processor".to_string(),
                expected_duration: Duration::seconds(0), // Instant
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "OrderPlaced".to_string());
                    m.insert(
                        "description".to_string(),
                        "Order placed for inventory replenishment".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "delivery_driver".to_string(),
                expected_duration: Duration::seconds(300), // 5 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "DriverAssigned".to_string());
                    m.insert(
                        "description".to_string(),
                        "Delivery driver assigned to pickup".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "delivery_driver".to_string(),
                expected_duration: Duration::seconds(600), // 10 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "PickingUp".to_string());
                    m.insert(
                        "description".to_string(),
                        "Driver traveling to distribution site and loading inventory".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 3,
                required_resource_type: "delivery_driver".to_string(),
                expected_duration: Duration::seconds(1800), // 30 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "InTransit".to_string());
                    m.insert(
                        "description".to_string(),
                        "Driver transporting inventory to kitchen".to_string(),
                    );
                    m
                },
            },
            WorkflowStep {
                step_index: 4,
                required_resource_type: "receiving_station".to_string(),
                expected_duration: Duration::seconds(300), // 5 minutes
                metadata: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "Delivered".to_string());
                    m.insert(
                        "description".to_string(),
                        "Inventory delivered and verified".to_string(),
                    );
                    m
                },
            },
        ];

        Self {
            workflow_id: WorkflowId::new(),
            item_type,
            quantity,
            source_site_id,
            destination_kitchen_id,
            steps,
        }
    }

    /// Get the item type for this workflow
    pub fn item_type(&self) -> &str {
        &self.item_type
    }

    /// Get the quantity being ordered
    pub fn quantity(&self) -> u32 {
        self.quantity
    }

    /// Get the source distribution site ID
    pub fn source_site_id(&self) -> Uuid {
        self.source_site_id
    }

    /// Get the destination kitchen ID
    pub fn destination_kitchen_id(&self) -> Uuid {
        self.destination_kitchen_id
    }
}

impl Workflow for SupplyChainWorkflow {
    fn id(&self) -> WorkflowId {
        self.workflow_id
    }

    fn workflow_type(&self) -> &str {
        "supply_chain"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

/// Supply chain delivery task
///
/// Tasks created from supply chain workflow steps. Each task represents
/// a specific action in the delivery pipeline.
#[derive(Debug, Clone)]
pub struct SupplyDeliveryTask {
    /// Task ID
    pub id: TaskId,
    /// Task type (assign_driver, pickup_inventory, deliver_inventory)
    pub task_type: String,
    /// Item type being delivered
    pub item_type: String,
    /// Quantity
    pub quantity: u32,
    /// Source distribution site
    pub source_site_id: Uuid,
    /// Destination kitchen
    pub destination_kitchen_id: Uuid,
    /// Required resource type
    pub required_resource_type: String,
    /// Estimated duration
    pub estimated_duration: Duration,
    /// Workflow instance this task belongs to
    pub workflow_instance_id: Option<WorkflowInstanceId>,
}

impl Task for SupplyDeliveryTask {
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

/// Task factory for supply chain workflows
///
/// Converts workflow steps into concrete supply delivery tasks.
#[derive(Debug, Clone)]
pub struct SupplyChainTaskFactory {
    /// Item type for this factory
    pub item_type: String,
    /// Quantity to deliver
    pub quantity: u32,
    /// Source site
    pub source_site_id: Uuid,
    /// Destination kitchen
    pub destination_kitchen_id: Uuid,
}

impl SupplyChainTaskFactory {
    /// Create a new supply chain task factory
    pub fn new(
        item_type: String,
        quantity: u32,
        source_site_id: Uuid,
        destination_kitchen_id: Uuid,
    ) -> Self {
        Self {
            item_type,
            quantity,
            source_site_id,
            destination_kitchen_id,
        }
    }
}

impl TaskFactory<SupplyDeliveryTask> for SupplyChainTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: chrono::DateTime<chrono::Utc>,
    ) -> SupplyDeliveryTask {
        // Determine task type based on step index
        let task_type = match step.step_index {
            0 => "place_order",
            1 => "assign_driver",
            2 => "pickup_inventory",
            3 => "transport_inventory",
            4 => "deliver_inventory",
            _ => "unknown",
        };

        SupplyDeliveryTask {
            id: TaskId::new(),
            task_type: task_type.to_string(),
            item_type: self.item_type.clone(),
            quantity: self.quantity,
            source_site_id: self.source_site_id,
            destination_kitchen_id: self.destination_kitchen_id,
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
    fn test_supply_chain_workflow_creation() {
        let source_id = Uuid::new_v4();
        let kitchen_id = Uuid::new_v4();

        let workflow = SupplyChainWorkflow::new(
            "flour".to_string(),
            100,
            source_id,
            kitchen_id,
        );

        assert_eq!(workflow.workflow_type(), "supply_chain");
        assert_eq!(workflow.item_type(), "flour");
        assert_eq!(workflow.quantity(), 100);
        assert_eq!(workflow.source_site_id(), source_id);
        assert_eq!(workflow.destination_kitchen_id(), kitchen_id);
    }

    #[test]
    fn test_workflow_steps_in_correct_order() {
        let workflow = SupplyChainWorkflow::new(
            "vegetables".to_string(),
            50,
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        let steps = workflow.steps();
        assert_eq!(steps.len(), 5);

        // Verify step order
        assert_eq!(steps[0].step_index, 0);
        assert_eq!(steps[0].metadata.get("name").unwrap(), "OrderPlaced");
        assert_eq!(steps[0].required_resource_type, "order_processor");

        assert_eq!(steps[1].step_index, 1);
        assert_eq!(steps[1].metadata.get("name").unwrap(), "DriverAssigned");
        assert_eq!(steps[1].required_resource_type, "delivery_driver");

        assert_eq!(steps[2].step_index, 2);
        assert_eq!(steps[2].metadata.get("name").unwrap(), "PickingUp");
        assert_eq!(steps[2].required_resource_type, "delivery_driver");

        assert_eq!(steps[3].step_index, 3);
        assert_eq!(steps[3].metadata.get("name").unwrap(), "InTransit");
        assert_eq!(steps[3].required_resource_type, "delivery_driver");

        assert_eq!(steps[4].step_index, 4);
        assert_eq!(steps[4].metadata.get("name").unwrap(), "Delivered");
        assert_eq!(steps[4].required_resource_type, "receiving_station");
    }

    #[test]
    fn test_task_factory_generates_correct_task_types() {
        let source_id = Uuid::new_v4();
        let kitchen_id = Uuid::new_v4();
        let instance_id = WorkflowInstanceId::new();

        let factory = SupplyChainTaskFactory::new(
            "meat".to_string(),
            75,
            source_id,
            kitchen_id,
        );

        let workflow = SupplyChainWorkflow::new(
            "meat".to_string(),
            75,
            source_id,
            kitchen_id,
        );

        let steps = workflow.steps();
        let current_time = chrono::Utc::now();

        // Test each step's task generation
        let task0 = factory.create_task(instance_id, &steps[0], current_time);
        assert_eq!(task0.task_type, "place_order");
        assert_eq!(task0.required_resource_type(), "order_processor");

        let task1 = factory.create_task(instance_id, &steps[1], current_time);
        assert_eq!(task1.task_type, "assign_driver");
        assert_eq!(task1.required_resource_type(), "delivery_driver");

        let task2 = factory.create_task(instance_id, &steps[2], current_time);
        assert_eq!(task2.task_type, "pickup_inventory");
        assert_eq!(task2.required_resource_type(), "delivery_driver");

        let task3 = factory.create_task(instance_id, &steps[3], current_time);
        assert_eq!(task3.task_type, "transport_inventory");
        assert_eq!(task3.required_resource_type(), "delivery_driver");

        let task4 = factory.create_task(instance_id, &steps[4], current_time);
        assert_eq!(task4.task_type, "deliver_inventory");
        assert_eq!(task4.required_resource_type(), "receiving_station");
    }

    #[test]
    fn test_workflow_progression() {
        let workflow = SupplyChainWorkflow::new(
            "dairy".to_string(),
            200,
            Uuid::new_v4(),
            Uuid::new_v4(),
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
        assert_eq!(steps[2].expected_duration, Duration::seconds(600)); // 10 min
        assert_eq!(steps[3].expected_duration, Duration::seconds(1800)); // 30 min
        assert_eq!(steps[4].expected_duration, Duration::seconds(300)); // 5 min
    }

    #[test]
    fn test_multiple_instances_running_concurrently() {
        let source_id = Uuid::new_v4();
        let kitchen_id = Uuid::new_v4();

        // Create multiple workflow instances
        let workflow1 = SupplyChainWorkflow::new(
            "flour".to_string(),
            100,
            source_id,
            kitchen_id,
        );

        let workflow2 = SupplyChainWorkflow::new(
            "sugar".to_string(),
            50,
            source_id,
            kitchen_id,
        );

        let workflow3 = SupplyChainWorkflow::new(
            "salt".to_string(),
            25,
            source_id,
            kitchen_id,
        );

        // Each should have unique workflow IDs
        assert_ne!(workflow1.id(), workflow2.id());
        assert_ne!(workflow1.id(), workflow3.id());
        assert_ne!(workflow2.id(), workflow3.id());

        // But same structure
        assert_eq!(workflow1.step_count(), 5);
        assert_eq!(workflow2.step_count(), 5);
        assert_eq!(workflow3.step_count(), 5);

        // And same workflow type
        assert_eq!(workflow1.workflow_type(), "supply_chain");
        assert_eq!(workflow2.workflow_type(), "supply_chain");
        assert_eq!(workflow3.workflow_type(), "supply_chain");
    }

    #[test]
    fn test_completion_triggers_inventory_update() {
        // This test verifies the workflow structure supports inventory updates
        let source_id = Uuid::new_v4();
        let kitchen_id = Uuid::new_v4();

        let workflow = SupplyChainWorkflow::new(
            "eggs".to_string(),
            144,
            source_id,
            kitchen_id,
        );

        // Final step should be "Delivered"
        let final_step = workflow.get_step(4).unwrap();
        assert_eq!(final_step.metadata.get("name").unwrap(), "Delivered");
        assert_eq!(final_step.required_resource_type, "receiving_station");

        // Verify we have the metadata needed for inventory update
        assert_eq!(workflow.item_type(), "eggs");
        assert_eq!(workflow.quantity(), 144);
        assert_eq!(workflow.destination_kitchen_id(), kitchen_id);
    }

    #[test]
    fn test_task_has_workflow_instance_id() {
        let source_id = Uuid::new_v4();
        let kitchen_id = Uuid::new_v4();
        let instance_id = WorkflowInstanceId::new();

        let factory = SupplyChainTaskFactory::new(
            "bread".to_string(),
            20,
            source_id,
            kitchen_id,
        );

        let step = WorkflowStep {
            step_index: 1,
            required_resource_type: "delivery_driver".to_string(),
            expected_duration: Duration::seconds(300),
            metadata: HashMap::new(),
        };

        let task = factory.create_task(instance_id, &step, chrono::Utc::now());

        assert_eq!(task.workflow_instance_id(), Some(instance_id));
        assert_eq!(task.item_type, "bread");
        assert_eq!(task.quantity, 20);
    }

    #[test]
    fn test_task_factory_creates_unique_task_ids() {
        let factory = SupplyChainTaskFactory::new(
            "milk".to_string(),
            10,
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        let step = WorkflowStep {
            step_index: 0,
            required_resource_type: "order_processor".to_string(),
            expected_duration: Duration::seconds(0),
            metadata: HashMap::new(),
        };

        let instance_id = WorkflowInstanceId::new();
        let current_time = chrono::Utc::now();

        let task1 = factory.create_task(instance_id, &step, current_time);
        let task2 = factory.create_task(instance_id, &step, current_time);

        assert_ne!(task1.id(), task2.id());
    }
}
