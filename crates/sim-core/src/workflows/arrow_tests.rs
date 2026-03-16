//! Comprehensive tests for Arrow RecordBatch workflow storage
//!
//! This module tests the WorkflowEngine's Apache Arrow RecordBatch integration,
//! including DataFusion queries, RecordBatch operations, and performance characteristics.

#[cfg(test)]
mod tests {
    use crate::utils::idents::{AgentId, WorkflowId};
    use crate::workflows::{Workflow, WorkflowEngine, WorkflowStep};
    use chrono::Duration;
    use std::collections::HashMap;

    #[derive(Debug)]
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

    fn create_workflow(step_count: usize) -> TestWorkflow {
        let steps = (0..step_count)
            .map(|i| WorkflowStep {
                step_index: i,
                required_resource_type: format!("step{}", i),
                expected_duration: Duration::seconds(10),
                metadata: HashMap::new(),
            })
            .collect();

        TestWorkflow {
            id: WorkflowId::new(),
            steps,
        }
    }

    /// Test 1: Start 100 workflow instances and verify RecordBatch structure
    #[test]
    fn test_start_100_instances_verify_structure() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(3);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Start 100 instances
        for _ in 0..100 {
            engine.start_instance(workflow_id).unwrap();
        }

        // Verify RecordBatch has correct number of rows
        assert_eq!(engine.instance_table.instance_count(), 100);

        // Verify all instances are retrievable
        let running = engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 100);

        // Verify each instance has correct initial state
        for instance_id in running {
            let instance = engine.instance_table.get_instance(instance_id).unwrap();
            assert_eq!(instance.workflow_id, workflow_id);
            assert_eq!(instance.current_step, 0);
            assert_eq!(instance.total_steps, 3);
            assert!(!instance.is_complete);
        }
    }

    /// Test 2: Advance instance through all steps (update current_step_index)
    #[test]
    fn test_advance_through_all_steps() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(5);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        let completion_time = chrono::Utc::now();

        // Advance through all 5 steps
        for step in 0..5 {
            let instance = engine.instance_table.get_instance(instance_id).unwrap();
            assert_eq!(instance.current_step, step);
            assert!(!instance.is_complete);

            let event = engine
                .advance_instance_step(instance_id, completion_time)
                .unwrap();

            if step < 4 {
                // Not complete yet
                assert!(matches!(
                    event,
                    Some(crate::workflows::WorkflowStepEvent::StepCompleted { .. })
                ));
            } else {
                // Final step - workflow complete
                assert!(matches!(
                    event,
                    Some(crate::workflows::WorkflowStepEvent::WorkflowCompleted { .. })
                ));
            }
        }

        // Verify final state
        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 5);
        assert!(instance.is_complete);
    }

    /// Test 3: Complete instance, verify status and completed_at set
    #[test]
    fn test_complete_instance_sets_completed_at() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(2);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Initially not complete
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Running);

        // Complete the workflow
        engine.complete_instance(instance_id).unwrap();

        // Verify status changed
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Complete);

        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert!(instance.is_complete);
    }

    /// Test 4: DataFusion query finds running instances
    #[test]
    fn test_datafusion_query_running_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(3);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create mix of running and completed instances
        let mut running_ids = Vec::new();
        let mut completed_ids = Vec::new();

        for i in 0..10 {
            let instance = engine.start_instance(workflow_id).unwrap();
            if i % 2 == 0 {
                running_ids.push(instance.id);
            } else {
                completed_ids.push(instance.id);
                engine.complete_instance(instance.id).unwrap();
            }
        }

        // Query running instances
        let running = engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 5);

        for id in &running_ids {
            assert!(running.contains(id));
        }

        for id in &completed_ids {
            assert!(!running.contains(id));
        }

        // Query completed instances
        let completed = engine.get_completed_instances().unwrap();
        assert_eq!(completed.len(), 5);

        for id in &completed_ids {
            assert!(completed.contains(id));
        }
    }

    /// Test 5: Query instances by agent_id (currently returns empty as we don't set agents)
    #[test]
    fn test_query_by_agent_id() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(2);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instances
        engine.start_instance(workflow_id).unwrap();
        engine.start_instance(workflow_id).unwrap();

        // Query by agent (should be empty as no agents assigned)
        let agent_id = AgentId::new();
        let agent_workflows = engine.instance_table.get_agent_workflows(agent_id).unwrap();
        assert_eq!(agent_workflows.len(), 0);

        // Note: In a full implementation, we would test assigning agents
        // and verifying the query returns the correct instances
    }

    /// Test 6: Pause and resume instances
    #[test]
    fn test_pause_and_resume_workflow() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(3);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Initially running
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Running);

        // Pause
        engine.pause_instance(instance_id).unwrap();
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Paused);

        // Resume
        engine.resume_instance(instance_id).unwrap();
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Running);

        // Pause again
        engine.pause_instance(instance_id).unwrap();
        let status = engine
            .instance_table
            .get_instance_status(instance_id)
            .unwrap();
        assert_eq!(status, crate::workflows::InstanceStatus::Paused);
    }

    /// Test 7: Performance benchmark - advance 1000 instances in single tick
    #[test]
    fn test_performance_advance_1000_instances() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(10);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create 1000 instances
        let mut instance_ids = Vec::new();
        for _ in 0..1000 {
            let instance = engine.start_instance(workflow_id).unwrap();
            instance_ids.push(instance.id);
        }

        let start = std::time::Instant::now();

        // Advance all instances by one step
        let completion_time = chrono::Utc::now();
        for instance_id in &instance_ids {
            engine
                .advance_instance_step(*instance_id, completion_time)
                .unwrap();
        }

        let duration = start.elapsed();

        // Verify all advanced
        for instance_id in &instance_ids {
            let instance = engine.instance_table.get_instance(*instance_id).unwrap();
            assert_eq!(instance.current_step, 1);
        }

        // Performance expectation: should complete in reasonable time
        // This is mainly to ensure no O(n^2) or worse behavior
        println!("Advancing 1000 instances took {:?}", duration);
        assert!(duration.as_secs() < 5, "Performance degradation detected");
    }

    /// Test 8: Edge case - instance with 0 steps
    #[test]
    fn test_zero_step_workflow() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(0);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        assert_eq!(instance.total_steps, 0);
        assert_eq!(instance.current_step, 0);

        // Advancing a 0-step workflow should complete it immediately
        let completion_time = chrono::Utc::now();
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        assert!(matches!(
            event,
            Some(crate::workflows::WorkflowStepEvent::WorkflowCompleted { .. })
        ));

        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert!(instance.is_complete);
    }

    /// Test 9: Edge case - already completed instance
    #[test]
    fn test_already_completed_instance() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(1);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Complete the workflow
        let completion_time = chrono::Utc::now();
        engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert!(instance.is_complete);

        // Try to advance again - should fail
        let result = engine.advance_instance_step(instance_id, completion_time);
        assert!(result.is_err());

        // Try to pause completed instance - should succeed (implementation choice)
        let result = engine.pause_instance(instance_id);
        assert!(result.is_ok());
    }

    /// Test 10: Integration test - Start instance → create tasks → complete workflow
    #[test]
    fn test_full_workflow_lifecycle() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(3);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Start instance
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        assert_eq!(engine.instance_table.instance_count(), 1);
        assert_eq!(instance.current_step, 0);

        let completion_time = chrono::Utc::now();

        // Simulate task completion for step 0
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();
        assert!(matches!(
            event,
            Some(crate::workflows::WorkflowStepEvent::StepCompleted { step_index: 0, .. })
        ));

        // Step 1
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();
        assert!(matches!(
            event,
            Some(crate::workflows::WorkflowStepEvent::StepCompleted { step_index: 1, .. })
        ));

        // Step 2 (final)
        let event = engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();
        assert!(matches!(
            event,
            Some(crate::workflows::WorkflowStepEvent::WorkflowCompleted { .. })
        ));

        // Verify final state
        let instance = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(instance.current_step, 3);
        assert!(instance.is_complete);

        // Verify in completed instances
        let completed = engine.get_completed_instances().unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0], instance_id);
    }

    /// Test 11: Round-trip serialization test
    #[test]
    fn test_roundtrip_instance_storage() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(4);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instance with metadata
        let instance = engine.start_instance(workflow_id).unwrap();
        let instance_id = instance.id;

        // Retrieve and verify
        let retrieved = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(retrieved.id, instance_id);
        assert_eq!(retrieved.workflow_id, workflow_id);
        assert_eq!(retrieved.current_step, 0);
        assert_eq!(retrieved.total_steps, 4);
        assert!(!retrieved.is_complete);

        // Advance and verify persistence
        let completion_time = chrono::Utc::now();
        engine
            .advance_instance_step(instance_id, completion_time)
            .unwrap();

        let retrieved = engine.instance_table.get_instance(instance_id).unwrap();
        assert_eq!(retrieved.current_step, 1);
    }

    /// Test 12: Multiple workflows with different step counts
    #[test]
    fn test_multiple_workflow_types() {
        let mut engine = WorkflowEngine::new();

        let workflow1 = create_workflow(2);
        let workflow2 = create_workflow(5);
        let workflow3 = create_workflow(3);

        let id1 = workflow1.id();
        let id2 = workflow2.id();
        let id3 = workflow3.id();

        engine.register_workflow(workflow1);
        engine.register_workflow(workflow2);
        engine.register_workflow(workflow3);

        // Start instances of each type
        let inst1 = engine.start_instance(id1).unwrap();
        let inst2 = engine.start_instance(id2).unwrap();
        let inst3 = engine.start_instance(id3).unwrap();

        // Verify correct step counts
        assert_eq!(inst1.total_steps, 2);
        assert_eq!(inst2.total_steps, 5);
        assert_eq!(inst3.total_steps, 3);

        // Verify all in running state
        let running = engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 3);
        assert!(running.contains(&inst1.id));
        assert!(running.contains(&inst2.id));
        assert!(running.contains(&inst3.id));
    }

    /// Test 13: Verify RecordBatch column types
    #[test]
    fn test_recordbatch_schema_validation() {
        let engine = WorkflowEngine::<TestWorkflow>::new();

        // Verify schema has correct fields
        let schema = crate::workflows::workflow_instance_schema();

        assert_eq!(schema.fields().len(), 10);
        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(1).name(), "workflow_id");
        assert_eq!(schema.field(2).name(), "status");
        assert_eq!(schema.field(3).name(), "current_step_index");
        assert_eq!(schema.field(4).name(), "total_steps");
        assert_eq!(schema.field(5).name(), "agent_id");
        assert_eq!(schema.field(6).name(), "started_at");
        assert_eq!(schema.field(7).name(), "completed_at");
        assert_eq!(schema.field(8).name(), "context");
        assert_eq!(schema.field(9).name(), "metadata");

        // Verify initial RecordBatch is empty
        assert_eq!(engine.instance_table.instance_count(), 0);
    }

    /// Test 14: Concurrent instance operations
    #[test]
    fn test_concurrent_instance_creation() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(2);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create multiple instances in sequence (simulating concurrent creation)
        let mut ids = Vec::new();
        for _ in 0..50 {
            let instance = engine.start_instance(workflow_id).unwrap();
            ids.push(instance.id);
        }

        // Verify all instances exist and are unique
        assert_eq!(engine.instance_table.instance_count(), 50);

        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 50);

        // Verify all are retrievable
        for id in ids {
            let instance = engine.instance_table.get_instance(id);
            assert!(instance.is_some());
        }
    }

    /// Test 15: DataFusion query with mixed statuses
    #[test]
    fn test_datafusion_mixed_status_queries() {
        let mut engine = WorkflowEngine::new();
        let workflow = create_workflow(3);
        let workflow_id = workflow.id();

        engine.register_workflow(workflow);

        // Create instances with different statuses
        let running1 = engine.start_instance(workflow_id).unwrap();
        let running2 = engine.start_instance(workflow_id).unwrap();

        let paused1 = engine.start_instance(workflow_id).unwrap();
        engine.pause_instance(paused1.id).unwrap();

        let completed1 = engine.start_instance(workflow_id).unwrap();
        engine.complete_instance(completed1.id).unwrap();

        let completed2 = engine.start_instance(workflow_id).unwrap();
        engine.complete_instance(completed2.id).unwrap();

        // Query each status
        let running = engine.get_running_instances().unwrap();
        assert_eq!(running.len(), 2);
        assert!(running.contains(&running1.id));
        assert!(running.contains(&running2.id));

        let completed = engine.get_completed_instances().unwrap();
        assert_eq!(completed.len(), 2);
        assert!(completed.contains(&completed1.id));
        assert!(completed.contains(&completed2.id));

        // Paused instances are queried via get_instances_by_status
        let paused = engine
            .instance_table
            .get_instances_by_status(crate::workflows::InstanceStatus::Paused)
            .unwrap();
        assert_eq!(paused.len(), 1);
        assert!(paused.contains(&paused1.id));
    }
}
