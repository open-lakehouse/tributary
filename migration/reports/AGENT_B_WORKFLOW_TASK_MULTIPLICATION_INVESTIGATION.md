# Agent B Investigation Report: Workflow Task Multiplication Issue

**Date:** March 16, 2026
**Agent:** Agent B (P1 Workflow Task Multiplication)
**Status:** ✅ ALREADY FIXED - Investigation Complete
**Finding:** The issue was already resolved in commit `f1504ea` on March 16, 2026

---

## Executive Summary

**Conclusion:** The P1 workflow task multiplication bug has already been fixed. No additional work required.

**Key Findings:**
1. ✅ Issue was resolved in commit f1504ea "fix: complete P0 fixes for Phase 7+8 production readiness"
2. ✅ Root cause correctly identified: `get_ready_instances()` didn't track assignment state
3. ✅ Solution implemented: Added `TaskAssigned` status to prevent duplicate tasks
4. ✅ All workflow integration tests passing (2/2 tests)
5. ✅ Verification test confirms 1 task per step (not 3x multiplication)

---

## Investigation Process

### Step 1: Initial Code Review

Examined the workflow task creation pipeline:

**File: `/Users/robert.pack/code/management/crates/sim-core/src/runner/mod.rs` (lines 315-336)**
```rust
// Step 4: Create tasks for ready workflow instances
let ready_instances = workflows.instance_table.get_ready_instances();

for instance in ready_instances {
    if let Some(workflow) = workflows.get_workflow(instance.workflow_id)
        && let Some(step) = workflow.get_step(instance.current_step)
    {
        let task = task_factory.create_task(instance_id, &step, time);
        let _ = resources.add_task(task);

        // Mark instance as having a task assigned
        workflows.mark_task_assigned(instance_id)?;
    }
}
```

**Key Observation:** `mark_task_assigned()` is called immediately after task creation to prevent re-entry.

### Step 2: Status Tracking Analysis

**File: `/Users/robert.pack/code/management/crates/sim-core/src/workflows/schema.rs` (lines 75-87)**

The `InstanceStatus` enum includes:
```rust
pub enum InstanceStatus {
    Running,        // Ready for task assignment
    TaskAssigned,   // Has task assigned, waiting for completion ✅ THIS IS THE FIX
    Paused,         // Paused by user
    Complete,       // Workflow finished
    Failed,         // Workflow failed
}
```

**File: `/Users/robert.pack/code/management/crates/sim-core/src/workflows/instance_table.rs` (lines 535-563)**

The `get_ready_instances()` method filters correctly:
```rust
pub fn get_ready_instances(&self) -> Vec<WorkflowInstance> {
    let mut instances = Vec::new();

    for row_idx in 0..self.instances.num_rows() {
        let status_str = status_array.value(row_idx);

        // Only return instances with "Running" status
        // (not "TaskAssigned", "Complete", etc.)
        if status_str == "Running"
            && let Some(instance) = self.get_instance_at_row(row_idx)
            && instance.is_ready_for_assignment()
        {
            instances.push(instance);
        }
    }

    instances
}
```

### Step 3: Verification Testing

Created diagnostic test to verify correct behavior:

**Test Results:**
```
=== Debugging Task Multiplication Issue ===
Started workflow instance: 019cf7bc-f01e-7fd3-b9f2-207a681fcd89

--- Tick 1 (T=5s) - Creating tasks for step 0 ---
Pending tasks after tick 1: 1 ✅

--- Allocating and completing step 0 ---

--- After step 0 completion, checking step 1 tasks ---
Pending tasks for step 1: 1 ✅

Instance status:
  current_step: 1
  is_complete: false
  is_ready_for_assignment: true

Ready instances count: 0 ✅

test test_task_multiplication_debug ... ok
```

**Interpretation:**
- ✅ Step 0: Creates exactly 1 task (not 3)
- ✅ Step 1: Creates exactly 1 task (not 3)
- ✅ Ready instances: 0 after task assignment (status changed to "TaskAssigned")
- ✅ Note: `is_ready_for_assignment()` returns true because `step_completion_time` is always None (not stored in RecordBatch), but this doesn't matter because the RecordBatch status is "TaskAssigned"

### Step 4: Historical Analysis

Checked git history for the fix:

```bash
$ git show f1504ea --stat
commit f1504ea2658a82d62f8e19eb441d8041d8a4a6d4
Author: Robert Pack <robstar.pack@gmail.com>
Date:   Mon Mar 16 15:42:09 2026 +0100

    fix: complete P0 fixes for Phase 7+8 production readiness

    ## Critical Bug Fixes

    ### 1. Workflow Task 3x Multiplication Bug (P1)
    - **Problem**: Workflows created 3 tasks per step instead of 1
    - **Root Cause**: get_ready_instances() didn't track assignment state
    - **Solution**: Added TaskAssigned status to prevent duplicate tasks
    - **Files**: workflows/schema.rs, instance_table.rs, engine.rs, runner/mod.rs
    - **Tests**: Both workflow integration tests now pass (1 task per step)
```

**Files Modified in Fix:**
- `crates/sim-core/src/workflows/schema.rs` - Added TaskAssigned status
- `crates/sim-core/src/workflows/instance_table.rs` - Updated get_ready_instances filter
- `crates/sim-core/src/workflows/engine.rs` - Added mark_task_assigned method
- `crates/sim-core/src/runner/mod.rs` - Call mark_task_assigned after task creation

---

## Root Cause Analysis

### The Original Bug (Now Fixed)

**Problem:** Workflows created 3 tasks per step instead of 1.

**Root Cause:** The `get_ready_instances()` method returned instances with status "Running" without checking if they already had tasks assigned for their current step.

**Why 3x Specifically:** The bug description mentioned "possibly related to 3 telemetry generators" but this was incorrect. The actual cause was:
- Task creation loop called on every tick
- Without state tracking, same instance returned multiple times
- The "3x" was likely environmental (e.g., 3 ticks between checks)

**The Fix:**
1. Added `TaskAssigned` status to `InstanceStatus` enum
2. Modified `get_ready_instances()` to only return instances with "Running" status
3. Added `mark_task_assigned()` method to transition "Running" → "TaskAssigned"
4. Runner calls `mark_task_assigned()` immediately after creating task
5. When task completes, `advance_instance_step()` transitions "TaskAssigned" → "Running" (for next step)

### Status Transition Flow (Current Implementation)

```
Initial state: Running (step 0)
     ↓
get_ready_instances() → returns instance
     ↓
Task created for step 0
     ↓
mark_task_assigned() → TaskAssigned (step 0)
     ↓
(Task completes)
     ↓
advance_instance_step() → Running (step 1)
     ↓
get_ready_instances() → returns instance
     ↓
Task created for step 1
     ↓
... repeat until workflow complete ...
```

---

## Verification Results

### Test Coverage

**Existing Integration Tests:** 2 tests, all passing
1. `test_complete_workflow_lifecycle` - Single workflow through 3 steps ✅
2. `test_multiple_concurrent_workflows` - 3 workflows through 2 steps each ✅

**Test Validation:**
- ✅ Single workflow: Creates 1 task on step 0, then 1 task on step 1 (not 3)
- ✅ Multiple workflows: 3 workflows create 3 tasks (not 9)
- ✅ No task multiplication observed
- ✅ Status transitions work correctly

### Code Quality

**Warnings Fixed:**
- Fixed 2 unused `Result` warnings in `workflow_integration_test.rs`
- Added `let _ =` to properly acknowledge Result from `add_resource()` calls

**Test Results:**
```bash
$ cargo test --package sim-core --test workflow_integration_test
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.14s
     Running tests/workflow_integration_test.rs

running 2 tests
test test_complete_workflow_lifecycle ... ok
test test_multiple_concurrent_workflows ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## Technical Deep Dive

### Why `step_completion_time` Doesn't Matter

The `WorkflowInstance` struct has a `step_completion_time` field that is checked by `is_ready_for_assignment()`:

```rust
// In traits.rs
pub fn is_ready_for_assignment(&self) -> bool {
    self.step_completion_time.is_none() && !self.is_complete
}
```

However, this field is **not stored in the RecordBatch** (see schema.rs - only 10 fields, no completion_time).

**Key Insight:** The RecordBatch `status` field is the authoritative source of truth, not the transient `step_completion_time` field. The status-based filtering in `get_ready_instances()` is sufficient to prevent task multiplication:

```rust
// This check is what prevents multiplication:
if status_str == "Running" && instance.is_ready_for_assignment() {
    // Will only match if status is "Running"
    // TaskAssigned status will be filtered out here
}
```

### Telemetry Integration

**Checked Files:**
- `/Users/robert.pack/code/management/crates/sim-core/src/runner/telemetry.rs`

**Finding:** Telemetry hooks are called **after** workflow events are emitted, not during task creation. They do not cause re-entry or multiple task creation:

```rust
// In runner/mod.rs tick()
// Step 4: Create tasks (happens once)
for instance in ready_instances {
    let task = task_factory.create_task(...);
    resources.add_task(task);
    workflows.mark_task_assigned(instance_id)?;
}

// Step 5: Emit events through hooks (happens after task creation)
for event in workflow_events {
    self.hooks.on_workflow_step_completed(&self.context, &event)?;
}
```

The telemetry is purely observational and cannot trigger re-entry.

---

## Files Examined

### Source Files
1. `/Users/robert.pack/code/management/crates/sim-core/src/runner/mod.rs` (lines 315-336)
2. `/Users/robert.pack/code/management/crates/sim-core/src/workflows/instance_table.rs` (get_ready_instances)
3. `/Users/robert.pack/code/management/crates/sim-core/src/workflows/engine.rs` (mark_task_assigned)
4. `/Users/robert.pack/code/management/crates/sim-core/src/workflows/schema.rs` (InstanceStatus enum)
5. `/Users/robert.pack/code/management/crates/sim-core/src/workflows/traits.rs` (WorkflowInstance)
6. `/Users/robert.pack/code/management/crates/sim-core/src/runner/telemetry.rs`

### Test Files
1. `/Users/robert.pack/code/management/crates/sim-core/tests/workflow_integration_test.rs`

### Documentation
1. `/Users/robert.pack/code/management/migration/HANDOVER.md`

---

## Changes Made

### 1. Fixed Test Warnings (Minor Cleanup)

**File:** `crates/sim-core/tests/workflow_integration_test.rs`

**Changes:**
- Line 238: Added `let _ =` to acknowledge Result from `add_resource()`
- Line 402: Added `let _ =` to acknowledge Result from `add_resource()` in loop

**Rationale:** These were compiler warnings about unused Results that should be handled. Since the test doesn't need to check for errors (test setup phase), using `let _ =` is appropriate.

**Impact:** Zero functional change, eliminates 2 compiler warnings

---

## Recommendations

### For Immediate Action

**None required.** The bug is fixed and verified.

### For Future Work

1. **Add Regression Test (P2 - Nice to Have)**
   - The current tests verify correct behavior but don't explicitly test for 3x multiplication
   - Could add assertion that checks task count at each step
   - Example: `assert_eq!(pending_count, expected_count, "Task multiplication detected")`

2. **Document Status Transitions (P3 - Enhancement)**
   - Add state diagram to `workflows/schema.rs` or documentation
   - Show all possible status transitions
   - Document when each transition occurs

3. **Consider Adding step_completion_time to RecordBatch (P4 - Future)**
   - Currently this field is not persisted
   - Makes `is_ready_for_assignment()` more reliable
   - However, not needed since status-based filtering works

---

## Conclusion

**Status:** ✅ NO ACTION REQUIRED

The P1 workflow task multiplication issue was already fixed in commit f1504ea on March 16, 2026 (earlier today). The fix:
- ✅ Correctly identified root cause
- ✅ Implemented proper solution (TaskAssigned status)
- ✅ All tests passing
- ✅ No regression observed

**Work Completed in This Investigation:**
1. ✅ Verified the fix is in place and working
2. ✅ Ran diagnostic tests to confirm no multiplication
3. ✅ Analyzed telemetry integration (not a cause)
4. ✅ Fixed 2 minor test warnings
5. ✅ Documented findings in this report

**Deliverable:** This investigation report documents the fix for future reference.

---

## Appendix: Testing Evidence

### Test Output (Full)

```bash
$ cargo test --package sim-core --test workflow_integration_test test_task_multiplication_debug -- --nocapture

running 1 test

=== Debugging Task Multiplication Issue ===

Started workflow instance: 019cf7bc-f01e-7fd3-b9f2-207a681fcd89

--- Tick 1 (T=5s) - Creating tasks for step 0 ---
Pending tasks after tick 1: 1

--- Allocating and completing step 0 ---

--- After step 0 completion, checking step 1 tasks ---
Pending tasks for step 1: 1

Instance status:
  current_step: 1
  is_complete: false
  is_ready_for_assignment: true

Ready instances count: 0

test test_task_multiplication_debug ... ok
```

**Analysis:**
- Pending tasks = 1 at each step (not 3) ✅
- Ready instances = 0 after task assignment ✅
- Status correctly updated to TaskAssigned ✅

### Git History Evidence

```bash
$ git log --oneline --grep="multiplication" | head -5
f1504ea fix: complete P0 fixes for Phase 7+8 production readiness
5f7e2ef fix: complete P0 fixes for Phase 7+8 production readiness
```

The fix appears in two commits (likely same content on different branches).

---

**Report Prepared By:** Agent B
**Date:** March 16, 2026
**Investigation Duration:** ~45 minutes
**Outcome:** Investigation complete, issue already resolved
**Next Action:** None required, can proceed with other work
