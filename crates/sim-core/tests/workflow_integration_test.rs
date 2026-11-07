//! Comprehensive end-to-end workflow integration test
//!
//! This test demonstrates the complete workflow lifecycle:
//! 1. Setup runner with workflows, resources, and task factory
//! 2. Start a workflow instance
//! 3. Tick: the runner creates the step's task and allocates it
//! 4. Keep ticking until the step's duration elapses; the same tick advances
//!    the instance and allocates the next step's task
//! 5. Repeat until the workflow completes
//! 6. Verify workflow marked complete and events emitted

use chrono::{DateTime, Duration, Utc};
use sim_core::{
    SimulationConfig, SimulationContext, SimulationHooks, SimulationRunner,
    population::{Agent, AgentStatus, AgentTickResult},
    resources::{Resource, ResourceAllocator, Task, TaskFactory},
    utils::idents::{AgentId, ResourceId, TaskId, WorkflowId, WorkflowInstanceId},
    workflows::{Workflow, WorkflowEngine, WorkflowRegistry, WorkflowStep, WorkflowStepEvent},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Test agent status
#[derive(Debug, Clone)]
enum TestStatus {
    Idle,
}

impl AgentStatus for TestStatus {
    fn as_str(&self) -> &str {
        "idle"
    }
}

// Test agent
#[derive(Debug, Clone)]
struct TestAgent {
    id: AgentId,
}

impl Agent for TestAgent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "test"
    }

    fn status(&self) -> &dyn AgentStatus {
        &TestStatus::Idle
    }

    fn position(&self) -> Option<geo::Point> {
        None
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> sim_core::Result<AgentTickResult> {
        Ok(AgentTickResult::default())
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> sim_core::Result<Self> {
        use uuid::Uuid;

        let id = AgentId::from_uuid(
            Uuid::from_slice(&row.id)
                .map_err(|e| sim_core::Error::Agent(format!("Invalid UUID: {}", e)))?,
        );

        Ok(TestAgent { id })
    }
}

// Test resource
#[derive(Debug)]
struct TestResource {
    id: ResourceId,
    capacity: usize,
    load: usize,
}

impl Resource for TestResource {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "test-resource"
    }

    fn capacity(&self) -> usize {
        self.capacity
    }

    fn current_load(&self) -> usize {
        self.load
    }
}

// Test task
#[derive(Debug)]
struct TestTask {
    id: TaskId,
    workflow_instance_id: Option<WorkflowInstanceId>,
    duration: Duration,
    resource_type: String,
}

impl Task for TestTask {
    fn id(&self) -> TaskId {
        self.id
    }

    fn required_resource_type(&self) -> &str {
        &self.resource_type
    }

    fn estimated_duration(&self) -> Duration {
        self.duration
    }

    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        self.workflow_instance_id
    }
}

// Test workflow
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
        "test-workflow"
    }

    fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }
}

// Test task factory
struct TestTaskFactory;

impl TaskFactory<TestTask> for TestTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        _current_time: DateTime<Utc>,
    ) -> TestTask {
        TestTask {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            duration: step.expected_duration,
            resource_type: step.required_resource_type.clone(),
        }
    }
}

// Test hooks that capture events
#[derive(Default)]
struct TestHooks {
    workflow_events: Arc<Mutex<Vec<WorkflowStepEvent>>>,
}

impl SimulationHooks for TestHooks {
    fn on_workflow_step_completed(
        &mut self,
        _ctx: &SimulationContext,
        event: &WorkflowStepEvent,
    ) -> sim_core::Result<()> {
        self.workflow_events.lock().unwrap().push(event.clone());
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

type TestRunner = SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow>;

#[test]
fn test_complete_workflow_lifecycle() {
    // 1. Setup runner with workflows, resources, and task factory
    let config = SimulationConfig {
        time_step: Duration::seconds(5), // 5 second steps
        verbose: false,
        ..Default::default()
    };

    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    // Create a workflow with 3 steps
    let workflow = TestWorkflow {
        id: WorkflowId::new(),
        steps: vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "test-resource".to_string(),
                expected_duration: Duration::seconds(10),
                metadata: HashMap::new(),
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "test-resource".to_string(),
                expected_duration: Duration::seconds(15),
                metadata: HashMap::new(),
            },
            WorkflowStep {
                step_index: 2,
                required_resource_type: "test-resource".to_string(),
                expected_duration: Duration::seconds(20),
                metadata: HashMap::new(),
            },
        ],
    };

    let workflow_id = workflow.id();

    // Setup workflow engine
    let mut engine = WorkflowEngine::new();
    engine.register_workflow(workflow);

    // 2. Start a workflow instance
    let instance = engine.start_instance(workflow_id).unwrap();
    let instance_id = instance.id;

    let mut registry = WorkflowRegistry::new();
    registry.register("test".to_string(), engine).unwrap();
    runner.set_workflow_registry(registry);

    // Setup resources
    let mut allocator = ResourceAllocator::new();
    let _ = allocator.add_resource(TestResource {
        id: ResourceId::new(),
        capacity: 10,
        load: 0,
    });
    runner.set_resource_allocator(allocator);

    // Setup task factory
    runner.set_task_factory(Box::new(TestTaskFactory));

    // Setup hooks to capture events
    let hooks = TestHooks::default();
    let events_ref = hooks.workflow_events.clone();
    runner.set_hooks(Box::new(hooks));

    let current_step = |runner: &TestRunner| {
        runner
            .workflow_registry()
            .unwrap()
            .get("test")
            .unwrap()
            .instance_table
            .get_instance(instance_id)
            .unwrap()
            .current_step
    };

    // 3. First tick (T=0): the step-0 task is created and allocated in the same tick.
    runner.tick().unwrap();
    assert_eq!(runner.resources().unwrap().pending_task_count(), 0);
    assert_eq!(runner.resources().unwrap().assignments().len(), 1);

    // 4. Step 0 takes 10s: it completes in the tick that starts at T=10,
    //    which also creates and allocates the step-1 task.
    runner.tick().unwrap(); // T=5
    assert_eq!(current_step(&runner), 0);
    runner.tick().unwrap(); // T=10
    assert_eq!(current_step(&runner), 1);
    assert_eq!(runner.resources().unwrap().assignments().len(), 1);

    // 5. Step 1 takes 15s: completes at T=25.
    runner.tick().unwrap(); // T=15
    runner.tick().unwrap(); // T=20
    assert_eq!(current_step(&runner), 1);
    runner.tick().unwrap(); // T=25
    assert_eq!(current_step(&runner), 2);

    // 6. Step 2 takes 20s: completes at T=45.
    for _ in 0..3 {
        runner.tick().unwrap(); // T=30, 35, 40
    }
    assert_eq!(current_step(&runner), 2);
    runner.tick().unwrap(); // T=45

    // 11. Verify workflow marked complete
    let engine = runner.workflow_registry().unwrap().get("test").unwrap();
    let instance = engine.instance_table.get_instance(instance_id).unwrap();
    assert_eq!(instance.current_step, 3);
    assert!(instance.is_complete);

    // 12. Verify events emitted
    let events = events_ref.lock().unwrap();
    assert_eq!(events.len(), 3); // 2 StepCompleted + 1 WorkflowCompleted

    // Verify first event (step 0 completed)
    match &events[0] {
        WorkflowStepEvent::StepCompleted {
            instance_id: id,
            step_index,
            ..
        } => {
            assert_eq!(*id, instance_id);
            assert_eq!(*step_index, 0);
        }
        _ => panic!("Expected StepCompleted event"),
    }

    // Verify second event (step 1 completed)
    match &events[1] {
        WorkflowStepEvent::StepCompleted {
            instance_id: id,
            step_index,
            ..
        } => {
            assert_eq!(*id, instance_id);
            assert_eq!(*step_index, 1);
        }
        _ => panic!("Expected StepCompleted event"),
    }

    // Verify third event (workflow completed)
    match &events[2] {
        WorkflowStepEvent::WorkflowCompleted {
            instance_id: id,
            workflow_id: wf_id,
            ..
        } => {
            assert_eq!(*id, instance_id);
            assert_eq!(*wf_id, workflow_id);
        }
        _ => panic!("Expected WorkflowCompleted event"),
    }

    // Verify no more pending tasks
    assert_eq!(runner.resources().unwrap().pending_task_count(), 0);
}

#[test]
fn test_multiple_concurrent_workflows() {
    // Setup runner
    let config = SimulationConfig {
        time_step: Duration::seconds(5),
        verbose: false,
        ..Default::default()
    };

    let mut runner: SimulationRunner<TestAgent, TestResource, TestTask, TestWorkflow> =
        SimulationRunner::new(config);

    // Create a simple 2-step workflow
    let workflow = TestWorkflow {
        id: WorkflowId::new(),
        steps: vec![
            WorkflowStep {
                step_index: 0,
                required_resource_type: "test-resource".to_string(),
                expected_duration: Duration::seconds(10),
                metadata: HashMap::new(),
            },
            WorkflowStep {
                step_index: 1,
                required_resource_type: "test-resource".to_string(),
                expected_duration: Duration::seconds(10),
                metadata: HashMap::new(),
            },
        ],
    };

    let workflow_id = workflow.id();

    // Setup workflow engine with 3 concurrent instances
    let mut engine = WorkflowEngine::new();
    engine.register_workflow(workflow);

    let instance1 = engine.start_instance(workflow_id).unwrap();
    let instance2 = engine.start_instance(workflow_id).unwrap();
    let instance3 = engine.start_instance(workflow_id).unwrap();

    let instance_ids = vec![instance1.id, instance2.id, instance3.id];

    let mut registry = WorkflowRegistry::new();
    registry.register("test".to_string(), engine).unwrap();
    runner.set_workflow_registry(registry);

    // Setup resources (enough capacity for all tasks)
    let mut allocator = ResourceAllocator::new();
    for _ in 0..3 {
        let _ = allocator.add_resource(TestResource {
            id: ResourceId::new(),
            capacity: 10,
            load: 0,
        });
    }
    runner.set_resource_allocator(allocator);

    // Setup task factory
    runner.set_task_factory(Box::new(TestTaskFactory));

    // Setup hooks
    let hooks = TestHooks::default();
    let events_ref = hooks.workflow_events.clone();
    runner.set_hooks(Box::new(hooks));

    // Tick at T=0 creates and allocates one task per instance.
    runner.tick().unwrap();
    assert_eq!(runner.resources().unwrap().pending_task_count(), 0);
    assert_eq!(runner.resources().unwrap().assignments().len(), 3);

    // Step 0 (10s) completes in the tick starting at T=10.
    runner.tick().unwrap(); // T=5
    runner.tick().unwrap(); // T=10

    let engine = runner.workflow_registry().unwrap().get("test").unwrap();
    for instance_id in &instance_ids {
        let instance = engine.instance_table.get_instance(*instance_id).unwrap();
        assert_eq!(instance.current_step, 1);
        assert!(!instance.is_complete);
    }
    // Step-1 tasks were created and allocated in the same tick.
    assert_eq!(runner.resources().unwrap().assignments().len(), 3);

    // Step 1 (10s) completes at T=20.
    runner.tick().unwrap(); // T=15
    runner.tick().unwrap(); // T=20

    let engine = runner.workflow_registry().unwrap().get("test").unwrap();
    for instance_id in &instance_ids {
        let instance = engine.instance_table.get_instance(*instance_id).unwrap();
        assert_eq!(instance.current_step, 2);
        assert!(instance.is_complete);
    }

    // Verify events: 3 StepCompleted + 3 WorkflowCompleted = 6 events
    let events = events_ref.lock().unwrap();
    assert_eq!(events.len(), 6);

    // Count event types
    let step_completed = events
        .iter()
        .filter(|e| matches!(e, WorkflowStepEvent::StepCompleted { .. }))
        .count();
    let workflow_completed = events
        .iter()
        .filter(|e| matches!(e, WorkflowStepEvent::WorkflowCompleted { .. }))
        .count();

    assert_eq!(step_completed, 3);
    assert_eq!(workflow_completed, 3);
}
