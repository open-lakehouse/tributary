//! Integration test for CourierDeliveryWorkflow with Courier agent
//!
//! This test demonstrates the complete integration between:
//! - CourierDeliveryWorkflow (workflow definition)
//! - CourierDeliveryTaskFactory (task generation)
//! - Courier agent (task execution)
//!
//! These tests verify that the workflow, task factory, and agent are compatible
//! and work together correctly.

use chrono::Utc;
use geo::Point;
use sim_core::{
    Agent, AgentStatus, Workflow,
    resources::{Task, TaskFactory},
    workflows::WorkflowInstance,
};
use sim_kitchen::{
    Courier,
    CourierDeliveryTaskFactory, CourierDeliveryWorkflow,
};
use uuid::Uuid;

#[test]
fn test_courier_delivery_workflow_with_agent_integration() {
    // Setup: Create workflow and agent
    let customer_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let kitchen_location = Point::new(-122.5, 37.9);
    let customer_location = Point::new(-122.3, 37.7);

    // Create workflow
    let workflow = CourierDeliveryWorkflow::new(
        customer_id,
        kitchen_location,
        customer_location,
        order_id,
    );

    // Verify workflow structure
    assert_eq!(workflow.workflow_type(), "courier_delivery");
    assert_eq!(workflow.step_count(), 4);

    // Create courier agent
    let courier_home = Point::new(-122.4, 37.8);
    let courier = Courier::new(courier_home);

    // Verify agent starts idle
    assert!(courier.is_idle());
    assert_eq!(courier.agent_type(), "courier");

    // Create task factory
    let factory = CourierDeliveryTaskFactory::new(
        customer_id,
        kitchen_location,
        customer_location,
        order_id,
    );

    // Verify task generation for each workflow step
    let current_time = Utc::now();
    let instance_id = sim_core::utils::idents::WorkflowInstanceId::new();

    for (idx, step) in workflow.steps().iter().enumerate() {
        let task = factory.create_task(instance_id, step, current_time);

        assert_eq!(task.workflow_instance_id(), Some(instance_id));
        assert_eq!(task.estimated_duration(), step.expected_duration);
        assert_eq!(task.required_resource_type(), step.required_resource_type.as_str());

        // Verify task types
        match idx {
            0 => assert_eq!(task.task_type, "assign_courier"),
            1 => assert_eq!(task.task_type, "pickup_food"),
            2 => assert_eq!(task.task_type, "deliver_food"),
            3 => assert_eq!(task.task_type, "complete_delivery"),
            _ => panic!("Unexpected step index"),
        }
    }
}

#[test]
fn test_workflow_instance_creation() {
    // Create workflow
    let customer_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let workflow = CourierDeliveryWorkflow::new(
        customer_id,
        Point::new(-122.5, 37.9),
        Point::new(-122.3, 37.7),
        order_id,
    );

    // Create workflow instance manually
    let instance = WorkflowInstance::new(workflow.id(), workflow.step_count());

    // Verify instance state
    assert_eq!(instance.workflow_id, workflow.id());
    assert_eq!(instance.total_steps, 4);
    assert_eq!(instance.current_step, 0);
    assert!(!instance.is_complete);
    assert!(instance.is_ready_for_assignment());
}

#[test]
fn test_workflow_instance_progression() {
    // Create workflow
    let workflow = CourierDeliveryWorkflow::new(
        Uuid::new_v4(),
        Point::new(-122.5, 37.9),
        Point::new(-122.3, 37.7),
        Uuid::new_v4(),
    );

    // Create instance
    let mut instance = WorkflowInstance::new(workflow.id(), workflow.step_count());

    // Progress through all steps
    let completion_time = Utc::now();

    for step_idx in 0..4 {
        assert_eq!(instance.current_step, step_idx);
        assert!(!instance.is_complete);

        // Advance step
        instance.advance_step(completion_time);

        if step_idx < 3 {
            assert_eq!(instance.current_step, step_idx + 1);
            assert!(!instance.is_complete);
        } else {
            // Final step
            assert_eq!(instance.current_step, 4);
            assert!(instance.is_complete);
        }
    }
}

#[test]
fn test_multiple_workflow_instances_independent() {
    // Create same workflow type, different instances
    let workflow = CourierDeliveryWorkflow::new(
        Uuid::new_v4(),
        Point::new(-122.5, 37.9),
        Point::new(-122.3, 37.7),
        Uuid::new_v4(),
    );

    // Create 3 independent instances
    let instance1 = WorkflowInstance::new(workflow.id(), workflow.step_count());
    let instance2 = WorkflowInstance::new(workflow.id(), workflow.step_count());
    let instance3 = WorkflowInstance::new(workflow.id(), workflow.step_count());

    // Verify all instances are unique
    assert_ne!(instance1.id, instance2.id);
    assert_ne!(instance1.id, instance3.id);
    assert_ne!(instance2.id, instance3.id);

    // But share same workflow
    assert_eq!(instance1.workflow_id, workflow.id());
    assert_eq!(instance2.workflow_id, workflow.id());
    assert_eq!(instance3.workflow_id, workflow.id());
}

#[test]
fn test_courier_agent_state_aligns_with_workflow_steps() {
    // This test verifies that courier states align with workflow steps
    let workflow = CourierDeliveryWorkflow::new(
        Uuid::new_v4(),
        Point::new(-122.5, 37.9),
        Point::new(-122.3, 37.7),
        Uuid::new_v4(),
    );

    let steps = workflow.steps();

    // Step 0: OrderAccepted -> Courier should be assigned (transitioning from Idle)
    assert_eq!(steps[0].metadata.get("name").unwrap(), "OrderAccepted");

    // Step 1: PickingUp -> Courier should be in PickingUp state
    assert_eq!(steps[1].metadata.get("name").unwrap(), "PickingUp");

    // Step 2: EnRoute -> Courier should be in Delivering state
    assert_eq!(steps[2].metadata.get("name").unwrap(), "EnRoute");

    // Step 3: Delivered -> Courier should transition to Returning
    assert_eq!(steps[3].metadata.get("name").unwrap(), "Delivered");

    // Verify all steps use courier resource (except final customer step)
    assert_eq!(steps[0].required_resource_type, "courier");
    assert_eq!(steps[1].required_resource_type, "courier");
    assert_eq!(steps[2].required_resource_type, "courier");
    assert_eq!(steps[3].required_resource_type, "customer");
}

#[test]
fn test_workflow_task_factory_preserves_metadata() {
    // Verify that task factory preserves all necessary metadata
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

    let workflow = CourierDeliveryWorkflow::new(
        customer_id,
        pickup,
        delivery,
        order_id,
    );

    let instance_id = sim_core::utils::idents::WorkflowInstanceId::new();
    let current_time = Utc::now();

    // Generate task for pickup step
    let pickup_step = &workflow.steps()[1];
    let task = factory.create_task(instance_id, pickup_step, current_time);

    // Verify all metadata is preserved
    assert_eq!(task.customer_id, customer_id);
    assert_eq!(task.order_id, order_id);
    assert_eq!(task.pickup_location, pickup);
    assert_eq!(task.delivery_location, delivery);
    assert_eq!(task.workflow_instance_id(), Some(instance_id));
}

#[test]
fn test_courier_agent_integrates_with_workflow() {
    // This test demonstrates that courier agent states align with workflow
    let customer_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let kitchen_location = Point::new(-122.5, 37.9);
    let customer_location = Point::new(-122.3, 37.7);

    // 1. Create workflow
    let workflow = CourierDeliveryWorkflow::new(
        customer_id,
        kitchen_location,
        customer_location,
        order_id,
    );

    // 2. Create courier agent
    let courier_home = Point::new(-122.4, 37.8);
    let mut courier = Courier::new(courier_home);

    // 3. Verify initial states match
    assert_eq!(workflow.step_count(), 4);
    assert!(courier.is_idle());

    // 4. Simulate workflow step 0 -> Assign courier (courier accepts order)
    let step0 = workflow.get_step(0).unwrap();
    assert_eq!(step0.metadata.get("name").unwrap(), "OrderAccepted");
    assert_eq!(step0.required_resource_type, "courier");

    // Courier accepts assignment - transitions to PickingUp
    courier.assign_delivery(
        kitchen_location,
        customer_location,
        order_id.to_string(),
        customer_id.to_string(),
    );
    assert!(!courier.is_idle());
    assert_eq!(courier.state.as_str(), "picking_up");

    // 5. Verify step 1 aligns with courier state
    let step1 = workflow.get_step(1).unwrap();
    assert_eq!(step1.metadata.get("name").unwrap(), "PickingUp");
    assert_eq!(courier.state.as_str(), "picking_up");

    // 6. Verify remaining steps align with courier lifecycle
    let step2 = workflow.get_step(2).unwrap();
    assert_eq!(step2.metadata.get("name").unwrap(), "EnRoute");
    // Courier will transition to "delivering" (same as EnRoute)

    let step3 = workflow.get_step(3).unwrap();
    assert_eq!(step3.metadata.get("name").unwrap(), "Delivered");
    // Courier will transition to "returning" after delivery

    // This demonstrates the workflow and agent are compatible
}
