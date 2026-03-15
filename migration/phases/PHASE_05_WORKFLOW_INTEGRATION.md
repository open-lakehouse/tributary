# Phase 5: Workflow Integration

**Status**: NOT STARTED
**Priority**: 🔴 CRITICAL PATH
**Estimated Effort**: 2 weeks
**Dependencies**: None (can start immediately)
**Blocks**: Phase 9 (Multi-Workflow), Phase 11 (Logistics Domain)

---

## Objectives

1. Integrate WorkflowEngine into SimulationRunner tick loop
2. Connect resource completion to workflow step advancement
3. Create bi-directional workflow ↔ resource coordination
4. Update ghost kitchen to use integrated workflow system
5. Achieve end-to-end workflow execution during simulation

---

## Prerequisites Checklist

- [x] WorkflowEngine exists with core API (`crates/sim-core/src/workflows/engine.rs`)
- [x] ResourceAllocator exists and tracks tasks (`crates/sim-core/src/resources/allocator.rs`)
- [x] SimulationRunner orchestrates tick loop (`crates/sim-core/src/runner/mod.rs`)
- [x] Workflow trait defined (`crates/sim-core/src/workflows/traits.rs`)
- [x] Task trait defined (`crates/sim-core/src/resources/traits.rs`)

---

## Current State Analysis

### What Exists
```rust
// crates/sim-core/src/runner/mod.rs:242
// TODO: Process workflows if engine exists
if let Some(workflows) = &mut self.workflows {
    // Workflow processing will be implemented when WorkflowEngine has a tick method
}
```

**Problem**: WorkflowEngine exists but isn't called during simulation.

### What's Missing
1. WorkflowEngine lacks `tick()` method for step progression
2. No mechanism to create tasks from workflow steps
3. ResourceAllocator doesn't notify anyone when tasks complete
4. No mapping between Task → WorkflowInstance
5. Ghost kitchen bypasses workflow system entirely

---

## Tasks

### Task 5.1: Add Workflow Instance → Task Mapping

**File**: `crates/sim-core/src/resources/traits.rs:7-23`

**Current Trait**:
```rust
pub trait Task: Send + Sync {
    fn id(&self) -> TaskId;
    fn required_resource_type(&self) -> &str;
    fn estimated_duration(&self) -> Duration;
}
```

**Required Change**:
```rust
pub trait Task: Send + Sync {
    fn id(&self) -> TaskId;
    fn required_resource_type(&self) -> &str;
    fn estimated_duration(&self) -> Duration;

    /// Optional workflow instance this task belongs to
    fn workflow_instance_id(&self) -> Option<WorkflowInstanceId> {
        None
    }
}
```

**Acceptance Criteria**:
- [ ] Task trait has `workflow_instance_id()` method
- [ ] Default implementation returns None (backward compatible)
- [ ] All tests pass

---

### Task 5.2: Track Completed Tasks in ResourceAllocator

**File**: `crates/sim-core/src/resources/allocator.rs`

**Add Method**:
```rust
impl<R: Resource, T: Task> ResourceAllocator<R, T> {
    /// Get tasks that completed since last tick
    pub fn get_completed_tasks(&mut self) -> Vec<(TaskId, WorkflowInstanceId)> {
        let mut completed = Vec::new();

        // Iterate allocated tasks
        for (resource_id, task_id) in &self.allocated_tasks {
            if let Some(task) = self.tasks.get(task_id) {
                // Check if task duration elapsed
                if self.is_task_complete(resource_id, task_id) {
                    if let Some(instance_id) = task.workflow_instance_id() {
                        completed.push((*task_id, instance_id));
                    }
                }
            }
        }

        // Remove completed tasks
        for (task_id, _) in &completed {
            self.remove_task(*task_id);
        }

        completed
    }

    fn is_task_complete(&self, resource_id: &ResourceId, task_id: &TaskId) -> bool {
        // Check if allocated_at + estimated_duration <= current_time
        // Implementation depends on how you track allocation time
        // For now, return based on your existing completion logic
        todo!("Implement based on your time tracking")
    }
}
```

**Acceptance Criteria**:
- [ ] `get_completed_tasks()` method added
- [ ] Returns tasks with workflow_instance_id
- [ ] Removes completed tasks from allocator
- [ ] Unit test for task completion tracking
- [ ] All existing tests pass

---

### Task 5.3: Add Tick Method to WorkflowEngine

**File**: `crates/sim-core/src/workflows/engine.rs`

**Add Methods**:
```rust
impl<W: Workflow> WorkflowEngine<W> {
    /// Process workflow instances for current tick
    pub fn tick(&mut self, current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        let mut events = Vec::new();

        // Update internal time tracking
        self.current_time = current_time;

        // Check for timed progressions (if any workflows have time-based steps)
        events.extend(self.process_timed_steps(current_time)?);

        Ok(events)
    }

    /// Get instances ready for next step (waiting for resource allocation)
    pub fn get_ready_instances(&self) -> Vec<&WorkflowInstance> {
        self.instances
            .values()
            .filter(|instance| {
                instance.status == WorkflowInstanceStatus::WaitingForResource
            })
            .collect()
    }

    /// Advance instance to next step after task completion
    pub fn advance_instance_step(
        &mut self,
        instance_id: WorkflowInstanceId,
        completion_time: DateTime<Utc>
    ) -> Result<Option<WorkflowStepEvent>> {
        let instance = self.instances
            .get_mut(&instance_id)
            .ok_or_else(|| anyhow!("Instance not found: {:?}", instance_id))?;

        // Mark current step complete
        instance.mark_step_complete(completion_time);

        // Move to next step
        instance.current_step += 1;

        // Check if workflow complete
        if instance.current_step >= self.workflows.get(&instance.workflow_id)
            .ok_or_else(|| anyhow!("Workflow not found"))?.steps().len()
        {
            instance.status = WorkflowInstanceStatus::Completed;
            instance.end_time = Some(completion_time);

            return Ok(Some(WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                workflow_id: instance.workflow_id,
                completion_time,
            }));
        }

        // Set status to waiting for resource
        instance.status = WorkflowInstanceStatus::WaitingForResource;

        Ok(Some(WorkflowStepEvent::StepCompleted {
            instance_id,
            step_index: instance.current_step - 1,
            completion_time,
        }))
    }

    fn process_timed_steps(&mut self, current_time: DateTime<Utc>) -> Result<Vec<WorkflowStepEvent>> {
        // For workflows with time-based progression (e.g., "wait 5 minutes")
        // Most workflows are resource-based, so this might be empty
        Ok(Vec::new())
    }
}

/// Events emitted by workflow progression
pub enum WorkflowStepEvent {
    StepCompleted {
        instance_id: WorkflowInstanceId,
        step_index: usize,
        completion_time: DateTime<Utc>,
    },
    WorkflowCompleted {
        instance_id: WorkflowInstanceId,
        workflow_id: WorkflowId,
        completion_time: DateTime<Utc>,
    },
}
```

**Acceptance Criteria**:
- [ ] `tick()` method added
- [ ] `get_ready_instances()` returns instances waiting for resources
- [ ] `advance_instance_step()` progresses workflow
- [ ] WorkflowStepEvent enum defined
- [ ] Unit tests for step advancement
- [ ] All existing tests pass

---

### Task 5.4: Integrate Workflow Processing into SimulationRunner

**File**: `crates/sim-core/src/runner/mod.rs:242`

**Replace TODO with Implementation**:
```rust
// Inside SimulationRunner::tick() method, around line 242

// Process workflows if engine exists
if let Some(workflows) = &mut self.workflows {
    // Step 1: Tick workflows (process timed steps)
    let workflow_events = workflows.tick(ctx.current_time())?;

    // Step 2: Get completed tasks from resources
    if let Some(resources) = &mut self.resources {
        let completed_tasks = resources.get_completed_tasks();

        // Step 3: Advance workflow instances for completed tasks
        for (_task_id, instance_id) in completed_tasks {
            if let Some(event) = workflows.advance_instance_step(instance_id, ctx.current_time())? {
                workflow_events.push(event);
            }
        }

        // Step 4: Get instances ready for next step
        let ready_instances = workflows.get_ready_instances();

        // Step 5: Create tasks for ready instances and allocate
        for instance in ready_instances {
            let workflow = workflows.get_workflow(instance.workflow_id)?;
            let step = workflow.get_step(instance.current_step)?;

            // Create task from workflow step
            let task = create_task_from_workflow_step(
                instance.instance_id,
                step,
                ctx.current_time()
            );

            // Add task to resource allocator
            resources.add_task(task);
        }
    }

    // Emit workflow events through hooks
    if let Some(hooks) = &mut self.hooks {
        for event in workflow_events {
            hooks.on_workflow_step_completed(&event, ctx)?;
        }
    }
}

// Helper function (add to impl block)
fn create_task_from_workflow_step<T: Task>(
    instance_id: WorkflowInstanceId,
    step: &WorkflowStep,
    current_time: DateTime<Utc>
) -> T {
    // This needs to be generic or you need a TaskFactory trait
    // For now, assume domains provide their own task creation
    todo!("Domain-specific task creation - see Task 5.5")
}
```

**Acceptance Criteria**:
- [ ] TODO comment replaced with working implementation
- [ ] Workflows tick each simulation step
- [ ] Completed tasks trigger workflow advancement
- [ ] Ready instances get new tasks allocated
- [ ] Workflow events emitted through hooks
- [ ] Integration test passes

---

### Task 5.5: Add TaskFactory Trait for Domain-Specific Task Creation

**File**: `crates/sim-core/src/resources/factory.rs` (new file)

**Create Trait**:
```rust
use crate::workflows::{WorkflowInstanceId, WorkflowStep};
use crate::resources::Task;
use chrono::{DateTime, Utc};

/// Factory for creating domain-specific tasks from workflow steps
pub trait TaskFactory<T: Task>: Send + Sync {
    /// Create a task from a workflow step
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>
    ) -> T;
}
```

**Update SimulationRunner**:
```rust
pub struct SimulationRunner<A, R, T, W>
where
    A: Agent,
    R: Resource,
    T: Task,
    W: Workflow,
{
    // ... existing fields ...
    task_factory: Option<Box<dyn TaskFactory<T>>>,
}
```

**Acceptance Criteria**:
- [ ] TaskFactory trait defined
- [ ] SimulationRunner accepts optional task_factory
- [ ] Builder pattern updated to set task_factory
- [ ] Documentation with example

---

### Task 5.6: Update Ghost Kitchen to Use Integrated Workflows

**File**: `crates/sim-kitchen/src/lib.rs`

**Changes Needed**:

1. **Create TaskFactory Implementation**:
```rust
// crates/sim-kitchen/src/resources/order_task_factory.rs (new file)

use sim_core::resources::TaskFactory;
use sim_core::workflows::{WorkflowInstanceId, WorkflowStep};
use crate::resources::Order;

pub struct OrderTaskFactory;

impl TaskFactory<Order> for OrderTaskFactory {
    fn create_task(
        &self,
        instance_id: WorkflowInstanceId,
        step: &WorkflowStep,
        current_time: DateTime<Utc>
    ) -> Order {
        Order {
            id: TaskId::new(),
            workflow_instance_id: Some(instance_id),
            required_resource_type: step.required_resource_type.clone(),
            estimated_duration: step.expected_duration,
            created_at: current_time,
            // ... other fields from step metadata
        }
    }
}
```

2. **Update Order to Track Workflow**:
```rust
// crates/sim-kitchen/src/resources/mod.rs

impl Task for Order {
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
        self.workflow_instance_id  // ← Add this field
    }
}
```

3. **Remove Manual Order Processing**:
```rust
// crates/sim-kitchen/src/lib.rs

// DELETE the process_demands() function
// Let the workflow system handle order creation → assignment → completion
```

4. **Update KitchenSimulationHooks**:
```rust
impl SimulationHooks for KitchenSimulationHooks {
    fn on_workflow_step_completed(
        &mut self,
        event: &WorkflowStepEvent,
        ctx: &SimulationContext
    ) -> Result<()> {
        match event {
            WorkflowStepEvent::StepCompleted { instance_id, step_index, completion_time } => {
                // Emit appropriate CloudEvents based on step
                match step_index {
                    0 => self.emit_order_assigned_event(*instance_id, *completion_time),
                    1 => self.emit_order_completed_event(*instance_id, *completion_time),
                    _ => Ok(()),
                }
            },
            WorkflowStepEvent::WorkflowCompleted { instance_id, workflow_id, completion_time } => {
                self.emit_order_delivered_event(*instance_id, *completion_time)
            }
        }
    }
}
```

**Acceptance Criteria**:
- [ ] OrderTaskFactory implemented
- [ ] Order tracks workflow_instance_id
- [ ] process_demands() removed
- [ ] Workflow events trigger CloudEvents emission
- [ ] End-to-end test: customer order → kitchen prep → delivery
- [ ] All existing tests updated and passing

---

### Task 5.7: Create Integration Test

**File**: `crates/sim-core/tests/workflow_integration_test.rs` (new file)

**Test Scenario**:
```rust
#[test]
fn test_workflow_resource_integration() {
    // Setup: Create simulation with workflows and resources
    let runner = SimulationRunner::builder()
        .context(create_test_context())
        .resources(create_test_resources())  // 3 kitchens
        .workflows(create_test_workflow_engine())  // Cooking workflow
        .task_factory(Box::new(OrderTaskFactory))
        .build()
        .unwrap();

    // Create workflow instance
    let instance_id = runner.workflows().start_instance(cooking_workflow_id)?;

    // Tick until workflow completes
    let mut ticks = 0;
    loop {
        runner.tick()?;
        ticks += 1;

        let instance = runner.workflows().get_instance(instance_id)?;
        if instance.status == WorkflowInstanceStatus::Completed {
            break;
        }

        assert!(ticks < 100, "Workflow should complete within 100 ticks");
    }

    // Verify workflow progressed through all steps
    let instance = runner.workflows().get_instance(instance_id)?;
    assert_eq!(instance.current_step, 3);  // Assuming 3-step workflow
    assert!(instance.end_time.is_some());
}
```

**Acceptance Criteria**:
- [ ] Integration test file created
- [ ] Test creates runner with all components
- [ ] Test verifies workflow progression
- [ ] Test verifies task allocation
- [ ] Test verifies step completion
- [ ] Test passes reliably

---

## Deliverables

### Code Changes
- [ ] `Task` trait has `workflow_instance_id()` method
- [ ] `ResourceAllocator` has `get_completed_tasks()` method
- [ ] `WorkflowEngine` has `tick()`, `get_ready_instances()`, `advance_instance_step()` methods
- [ ] `SimulationRunner` integrates workflow processing in tick loop
- [ ] `TaskFactory` trait created for domain-specific task creation
- [ ] Ghost kitchen uses integrated workflow system
- [ ] `process_demands()` manual logic removed

### Tests
- [ ] Unit tests for workflow advancement (5+ tests)
- [ ] Unit tests for task completion tracking (3+ tests)
- [ ] Integration test for full workflow lifecycle (1 comprehensive test)
- [ ] Ghost kitchen end-to-end test updated (1 test)
- [ ] All existing tests pass (79 → ~90 tests)

### Documentation
- [ ] Code comments explaining workflow-resource coordination
- [ ] Example in TaskFactory trait docstring
- [ ] Updated README in sim-core explaining workflow integration

---

## Testing Strategy

### Unit Tests
```bash
# Test workflow engine
cd crates/sim-core
cargo test workflows::engine::tests --lib

# Test resource allocator
cargo test resources::allocator::tests::test_get_completed_tasks --lib
```

### Integration Tests
```bash
# Test full integration
cd crates/sim-core
cargo test --test workflow_integration_test

# Test ghost kitchen
cd crates/sim-kitchen
cargo test --lib
```

### Manual Verification
```bash
# Run ghost kitchen example
cd crates/sim-kitchen
cargo run --example basic

# Should see:
# - Orders created
# - Orders assigned to kitchens (via workflow)
# - Orders completed (via workflow)
# - No process_demands() logs
```

---

## Acceptance Criteria (Phase Complete)

- [ ] **Workflow Engine Integrated**: WorkflowEngine.tick() called every simulation step
- [ ] **Resource Coordination**: Task completion triggers workflow advancement
- [ ] **Task Creation**: Ready workflow instances generate tasks automatically
- [ ] **Ghost Kitchen Updated**: Uses workflow system, manual logic removed
- [ ] **Events Emitted**: Workflow step events trigger CloudEvents
- [ ] **Tests Pass**: All 90+ tests passing
- [ ] **Integration Test**: End-to-end workflow lifecycle test passes
- [ ] **No Regressions**: Existing functionality still works

---

## Next Steps After Completion

1. **Mark Phase Complete**: Create `PHASE_05_COMPLETE.md` summary
2. **Update Main Plan**: Change status to COMPLETE in `FRAMEWORK_COMPLETION_PLAN.md`
3. **Unblock Phase 9**: Multi-workflow support now possible
4. **Unblock Phase 11**: Logistics domain can use workflows

---

## Troubleshooting

### Issue: Tasks not completing
**Check**:
- ResourceAllocator tracking allocation time
- `is_task_complete()` logic correct
- Time advancing in simulation context

### Issue: Workflows not advancing
**Check**:
- `get_completed_tasks()` returning correct instances
- `advance_instance_step()` being called
- WorkflowInstance status transitions

### Issue: Tests failing after changes
**Check**:
- Task trait changes backward compatible (default impl for workflow_instance_id)
- Existing tests don't assume old behavior
- Mock tasks updated if needed

---

## Parallel Work Opportunities

This phase is sequential (tasks depend on each other), but Phase 7 (Road Network) can run in parallel.

**Coordination Point**: Both phases modify different files, minimal merge conflicts expected.

---

**Estimated Effort**: 2 weeks (1 developer)
**Can Start**: Immediately (no blockers)
**Blocks**: Phase 9, Phase 11
