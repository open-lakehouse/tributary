# Forward Plan - Post Code Review

**Created:** March 16, 2026
**Status:** Code review complete, ready for implementation
**Overall Progress:** 38% complete (5.33/14 phases)
**Target:** Production-ready framework with all 14 phases complete

---

## Executive Summary

Based on comprehensive code review of all crates, we have identified **critical bugs and code quality issues** that must be addressed before continuing with new features (Phases 7-14). This plan outlines a phased approach to:

1. **Fix critical bugs** (correctness, safety)
2. **Clean up documentation** (reduce redundancy, improve navigation)
3. **Simplify codebase** (reduce duplication, improve performance)
4. **Continue feature development** (Phases 7-14)

**Timeline:** 10-12 weeks remaining (with or without fixes, but quality differs significantly)

---

## Phase 1: Critical Fixes 🔴

**Priority:** MUST DO FIRST (Blockers)
**Duration:** 1-2 days
**Impact:** Correctness, safety, maintainability

### Goals
- Fix all critical bugs identified in code review
- Ensure all 221+ tests pass
- Achieve zero clippy warnings
- Validate simulation correctness

---

### Track A: sim-core Fixes (4-6 hours)

#### Task 1.1: Remove Duplicate Code in WorkflowEngine (2-3 hours)
**File:** `crates/sim-core/src/workflows/engine.rs`

**Actions:**
- [ ] Delete `update_instance_at_row()` async method (lines 549-778) - saves 230 lines
- [ ] Rename `update_instance_at_row_sync()` to `update_instance_at_row()`
- [ ] Update all callers (lines 821, 833) to use new name
- [ ] Verify all 151 sim-core tests still pass

**Lines saved:** ~230

---

#### Task 1.2: Extract Array Manipulation Helpers (1-2 hours)
**File:** `crates/sim-core/src/workflows/engine.rs`

**Actions:**
- [ ] Create `ColumnArrays` struct to hold typed array references
- [ ] Implement `extract_column_arrays(&self) -> Result<ColumnArrays>`
- [ ] Replace repeated downcasting in:
  - `copy_column_data()` (lines 337-420)
  - `update_instance_at_row()` (lines 872-970)
- [ ] Verify tests pass

**Lines saved:** ~150

**Example:**
```rust
struct ColumnArrays<'a> {
    id: &'a BinaryArray,
    workflow_id: &'a BinaryArray,
    current_step: &'a UInt32Array,
    // ... other columns
}

impl WorkflowEngine {
    fn extract_column_arrays(&self) -> Result<ColumnArrays> {
        Ok(ColumnArrays {
            id: self.instances.column(0).as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| Error::Workflow("...".to_string()))?,
            // ... other columns
        })
    }
}
```

---

#### Task 1.3: Fix Unused SessionContext (30 mins)
**Files:**
- `crates/sim-core/src/workflows/engine.rs:79-83`
- `crates/sim-core/src/resources/allocator.rs:85`

**Actions:**
- [ ] **Option A (Quick):** Remove unused `ctx: SessionContext` fields and update docs
- [ ] **Option B (Future):** File GitHub issue for Phase 6 Week 3 (SQL query interface)

**Recommendation:** Option A (remove) - implement queries later if needed

---

#### Task 1.4: Fix Timestamp Handling (1 hour)
**File:** `crates/sim-core/src/workflows/engine.rs`

**Actions:**
- [ ] Pass `SimulationContext` or current timestamp to workflow methods
- [ ] Replace `TODO` comments with actual timestamp capture:
  - Line 281: `started_at_builder.append_value(ctx.step())`
  - Line 824: `update.completed_at = Some(ctx.step())`
  - Line 1193: `update.completed_at = Some(ctx.step())`
- [ ] Verify timestamps are non-zero in tests

---

#### Task 1.5: Add Step Completion Time to Schema (1 hour)
**File:** `crates/sim-core/src/workflows/engine.rs`

**Actions:**
- [ ] Add `step_completion_time` column to `workflow_instance_schema()`
- [ ] Update `append_instance()` to populate field
- [ ] Update `get_instance()` to read field (remove line 542 comment)
- [ ] Update export/import methods
- [ ] Add test for round-trip with completion time

---

### Track B: sim-geo Fixes (2-3 hours)

#### Task 1.6: Fix Division by Zero (30 mins)
**File:** `crates/sim-geo/src/movement/mod.rs:109`

**Actions:**
- [ ] Add guard for zero-distance journeys:
```rust
let progress_delta = if journey.distance_m > 0.0 {
    distance_traveled / journey.distance_m
} else {
    1.0 // Instant completion for zero-distance
};
```
- [ ] Add test for zero-distance journey
- [ ] Verify no NaN/Infinity values in tests

---

#### Task 1.7: Add Coordinate Validation (1 hour)
**File:** `crates/sim-geo/src/locations/handler.rs`

**Actions:**
- [ ] Add validation in `add_location()` (line 114):
```rust
if lat < -90.0 || lat > 90.0 || lon < -180.0 || lon > 180.0 {
    return Err(Error::Location(
        format!("Invalid coordinates: lat={}, lon={}", lat, lon)
    ));
}
```
- [ ] Add tests for invalid coordinates (lat=200, lon=400, etc.)
- [ ] Update documentation with coordinate bounds

---

#### Task 1.8: Add Radius Validation (30 mins)
**File:** `crates/sim-geo/src/locations/handler.rs:311`

**Actions:**
- [ ] Add validation in `within_radius()`:
```rust
if radius_meters < 0.0 || !radius_meters.is_finite() {
    return Err(Error::Location("Invalid radius".to_string()));
}
```
- [ ] Add tests for negative and infinite radius

---

### Track C: sim-events Fixes (1 hour)

#### Task 1.9: Fix Count Tracking (15 mins)
**File:** `crates/sim-events/src/dispatcher/mod.rs:77-79`

**Actions:**
- [ ] Update `clear()` method:
```rust
pub fn clear(&mut self) {
    self.buffer.clear();
    self.counts.clear();  // Add this line
}
```
- [ ] Verify fix with test

---

#### Task 1.10: Document Count Behavior (15 mins)
**File:** `crates/sim-events/src/dispatcher/mod.rs`

**Actions:**
- [ ] **Decision:** Are counts cumulative across flushes or per-buffer?
- [ ] Document decision in module docs
- [ ] If cumulative: Add doc comment explaining behavior
- [ ] If per-buffer: Also reset counts in `flush_to_parquet()` and `flush_to_jsonl()`

---

#### Task 1.11: Add Count Tracking Test (30 mins)
**File:** `crates/sim-events/tests/`

**Actions:**
- [ ] Add test for `clear()` resetting counts
- [ ] Add test for count behavior across multiple flushes
- [ ] Verify counts match actual events

---

### Track D: sim-kitchen Fixes (3-4 hours)

#### Task 1.12: Fix Order Lifecycle Completion (2 hours)
**File:** `crates/sim-kitchen/src/simulation.rs`

**Actions:**
- [ ] Implement order completion tracking in `KitchenSimulationHooks`
- [ ] Add method to check completed tasks and notify customers:
```rust
fn on_resources_updated(&mut self, ctx, allocator) -> Result<()> {
    // Check for completed tasks
    for task_id in allocator.get_completed_tasks()? {
        if let Some(order_info) = self.orders.get(&task_id.to_string()) {
            // Notify customer to start eating
            let customer = population.get_agent_mut(&order_info.customer_id)?;
            customer.start_eating(ctx.current_time());
        }
    }
    Ok(())
}
```
- [ ] Update `OrderInfo.completed_at` when task completes
- [ ] Verify customers transition: Ordering → Waiting → Eating → Done

---

#### Task 1.13: Handle Result Returns (30 mins)
**File:** `crates/sim-kitchen/src/simulation.rs`

**Actions:**
- [ ] Line 213: Change `allocator.add_resource(kitchen);` to `allocator.add_resource(kitchen)?;`
- [ ] Line 281: Change `allocator.add_task(order);` to `allocator.add_task(order)?;`
- [ ] Verify error propagation works correctly

---

#### Task 1.14: Fix Customer ID Tracking (30 mins)
**File:** `crates/sim-kitchen/src/simulation.rs:129`

**Actions:**
- [ ] Parse actual customer ID from demand metadata
- [ ] Replace `AgentId::new()` with parsed customer ID:
```rust
let customer_id = demand.metadata.get("customer_id")
    .and_then(|v| v.as_str())
    .map(|s| AgentId::from_str(s))
    .ok_or_else(|| Error::Kitchen("Missing customer_id".to_string()))?;
```
- [ ] Verify events have correct customer IDs

---

#### Task 1.15: Add Integration Test (1 hour)
**File:** `crates/sim-kitchen/tests/integration.rs`

**Actions:**
- [ ] Create integration test for complete customer lifecycle:
  - Start with idle customer
  - Customer orders food
  - Order assigned to kitchen
  - Order completed
  - Customer starts eating
  - Customer finishes and transitions to Done
- [ ] Verify all events emitted (Created, Assigned, Completed, Eating, Done)
- [ ] Check kitchen capacity constraints

---

### Phase 1 Success Criteria

- ✅ All 221+ tests passing (sim-core: 151, sim-geo: 43, sim-events: 12, sim-kitchen: 15+)
- ✅ Zero clippy warnings
- ✅ sim-core WorkflowEngine reduced by ~380 lines
- ✅ No division-by-zero or invalid coordinate bugs possible
- ✅ sim-kitchen customers complete full lifecycle
- ✅ sim-events count tracking correct
- ✅ All timestamps non-zero

---

## Phase 2: Documentation Cleanup 📚

**Priority:** ESSENTIAL (High value, low effort)
**Duration:** 0.5 days (3-4 hours)
**Impact:** Developer experience, onboarding speed

### Goals
- Reduce documentation from 8,557 to ~1,400 essential lines (84% reduction)
- Eliminate 40% redundancy
- Create single source of truth (README.md)
- Improve navigation and discoverability

---

### Task 2.1: Create Directory Structure (15 mins)

**Actions:**
- [ ] Create `migration/completed/` directory
- [ ] Create `migration/archive/` directory
- [ ] Create `docs/` directory at repository root (if not exists)

---

### Task 2.2: Move Completion Docs (15 mins)

**Actions:**
- [ ] Move to `migration/completed/`:
  - PHASE_5_COMPLETE.md
  - PHASE_5_REFINEMENT.md
  - PHASE_6_WEEK_1_COMPLETE.md
  - PHASE_6_WEEK_1_REFINEMENT.md
  - PHASE_6_WEEK_2_COMPLETE.md

---

### Task 2.3: Archive Superseded Docs (30 mins)

**Actions:**
- [ ] Extract code review findings from MIGRATION_STATUS.md (lines 39-87)
- [ ] Merge findings into PHASE_6_WEEK_2_COMPLETE.md
- [ ] Move to `migration/archive/`:
  - FRAMEWORK_COMPLETION_PLAN.md (after extracting design decisions)
  - MIGRATION_STATUS.md
  - PHASE_5_REVIEW.md
  - PHASE_6_WEEK_1_VERIFICATION_REPORT.md
  - PHASE_6_WEEK_2_PLAN.md

---

### Task 2.4: Create ARCHITECTURE.md (1 hour)

**File:** `docs/ARCHITECTURE.md`

**Actions:**
- [ ] Extract "Design Decisions" from FRAMEWORK_COMPLETION_PLAN.md
- [ ] Document key patterns:
  - RecordBatch + HashMap dual index
  - TaskFactory pattern
  - SnapshotManager dual backend (Memory/Parquet)
  - Demand-driven workflow allocation
- [ ] Add cross-references to implementation files
- [ ] Explain Arrow/DataFusion integration

---

### Task 2.5: Enhance README.md (1-2 hours)

**File:** `migration/README.md`

**Actions:**
- [ ] Add "Last Updated" timestamp
- [ ] Add "Quick Decision Tree":
  ```
  ┌─ Starting fresh? → Quick Start section
  ├─ Working with others? → PARALLEL_EXECUTION_GUIDE.md
  ├─ Understanding past work? → completed/PHASE_X_COMPLETE.md
  ├─ Planning next phase? → phases/PHASE_XX_NAME.md
  └─ Looking for old docs? → archive/
  ```
- [ ] Consolidate status tables (eliminate duplication)
- [ ] Add links to `completed/` and `archive/` directories
- [ ] Update phase status to reflect code review completion
- [ ] Add note about CURRENT_STATUS.md and CODE_REVIEW_SUMMARY.md

---

### Phase 2 Success Criteria

- ✅ Migration docs organized into 3 clear directories (completed/, phases/, archive/)
- ✅ README.md is single source of truth with clear navigation
- ✅ Onboarding reading reduced from 8,557 to <1,500 lines
- ✅ Zero redundancy across active documentation
- ✅ docs/ARCHITECTURE.md provides design context

---

## Phase 3: Code Simplification 🔧

**Priority:** QUALITY (Medium - can run parallel with Phase 7/8)
**Duration:** 2-3 days
**Impact:** Maintainability, performance, developer experience

### Goals
- Reduce code complexity and duplication
- Improve performance (fix O(N²) issues)
- Enhance sim-kitchen as complete reference implementation
- Add performance benchmarks

---

### Track A: sim-core Refactoring (1 day)

#### Task 3.1: Extract WorkflowInstanceTable Wrapper (3-4 hours)
**New File:** `crates/sim-core/src/workflows/table.rs`

**Actions:**
- [ ] Create `WorkflowInstanceTable` struct to encapsulate RecordBatch operations:
```rust
pub struct WorkflowInstanceTable {
    batch: RecordBatch,
    schema: SchemaRef,
    index: HashMap<WorkflowInstanceId, usize>,
}

impl WorkflowInstanceTable {
    pub fn get(&self, id: WorkflowInstanceId) -> Option<WorkflowInstance>;
    pub fn update(&mut self, id: WorkflowInstanceId, f: impl FnOnce(&mut WorkflowInstance)) -> Result<()>;
    pub fn filter_by_status(&self, status: InstanceStatus) -> Vec<WorkflowInstanceId>;
    pub fn export_batch(&self) -> RecordBatch;
    pub fn import_batch(batch: RecordBatch) -> Result<Self>;
}
```
- [ ] Refactor WorkflowEngine to use table wrapper
- [ ] Reduce engine.rs from 1,100 to ~700 lines
- [ ] Verify all tests pass

---

#### Task 3.2: Implement Batch Update API (2-3 hours)
**File:** `crates/sim-core/src/workflows/engine.rs`

**Actions:**
- [ ] Create `update_instances_batch()` method that batches all updates before rebuilding
- [ ] Refactor `start_instances()` to use batch API instead of loop
- [ ] Add performance benchmark comparing single vs batch updates
- [ ] Target: 10-100x speedup for batch operations

---

#### Task 3.3: Generate Schema Validation (1 hour)
**File:** `crates/sim-core/src/workflows/engine.rs:90-156`

**Actions:**
- [ ] Replace 66 lines of manual validation with schema comparison:
```rust
fn validate_instance_schema(batch: &RecordBatch) -> Result<()> {
    let expected = workflow_instance_schema();
    if !schemas_compatible(batch.schema(), &expected) {
        return Err(Error::Workflow(format!(
            "Schema mismatch: expected {:?}, got {:?}",
            expected, batch.schema()
        )));
    }
    Ok(())
}
```
- [ ] Save ~50 lines of code

---

#### Task 3.4: Add Performance Benchmarks (1-2 hours)
**New File:** `crates/sim-core/benches/workflow_performance.rs`

**Actions:**
- [ ] Add criterion benchmark for:
  - Single workflow update (baseline)
  - 1,000 workflow updates (O(N²) test)
  - Batch update of 1,000 workflows (target)
- [ ] Target: <100ms for 10k workflows × 1k steps
- [ ] Document results

---

### Track B: sim-geo Simplification (0.5 day)

#### Task 3.5: Create LocationColumns Helper (1-2 hours)
**New File:** `crates/sim-geo/src/locations/columns.rs`

**Actions:**
- [ ] Create helper struct:
```rust
struct LocationColumns<'a> {
    id: &'a BinaryArray,
    name: &'a StringArray,
    lat: &'a Float64Array,
    lon: &'a Float64Array,
    // ... other columns
}

impl LocationHandler {
    fn columns(&self) -> Result<LocationColumns>;
}
```
- [ ] Replace hardcoded column access throughout handler
- [ ] Save ~30 lines, improve readability

---

#### Task 3.6: Extract Distance Calculation (1 hour)
**File:** `crates/sim-geo/src/locations/handler.rs`

**Actions:**
- [ ] Extract common logic from `nearest()` (lines 262-277) and `nearest_brute_force()` (lines 283-298)
- [ ] Create `find_nearest_in_candidates()` helper
- [ ] Save ~30 lines

---

#### Task 3.7: Use Arrow concat_batches (1 hour)
**File:** `crates/sim-geo/src/locations/handler.rs`

**Actions:**
- [ ] Replace manual array building in `append_to_batch()` with:
```rust
let new_row = RecordBatch::try_new(schema, vec![...single row...])?;
self.locations = concat_batches(&schema, &[&self.locations, &new_row])?;
```
- [ ] Save ~80 lines
- [ ] Improve performance (O(1) amortized vs O(n))

---

### Track C: sim-kitchen Enhancement (0.5 day)

#### Task 3.8: Emit All Event Types (1 hour)
**File:** `crates/sim-kitchen/src/simulation.rs`

**Actions:**
- [ ] Add `OrderAssignedEvent` when order assigned to kitchen
- [ ] Add `OrderCompletedEvent` when kitchen finishes order
- [ ] Add `CustomerEatingEvent` when customer starts eating
- [ ] Add `CustomerDoneEvent` when customer finishes
- [ ] Update tests to verify event sequence

---

#### Task 3.9: Create FoodPrepWorkflow Example (1-2 hours)
**File:** `crates/sim-kitchen/src/workflows.rs`

**Actions:**
- [ ] Remove `NoWorkflow` placeholder
- [ ] Implement `FoodPrepWorkflow`:
  - Step 1: Prep (gather ingredients)
  - Step 2: Cook (prepare dish)
  - Step 3: Package (ready for pickup)
- [ ] Integrate with order processing
- [ ] Add workflow progression tests

---

#### Task 3.10: Remove Manual Load Tracking (30 mins)
**File:** `crates/sim-kitchen/src/resources.rs`

**Actions:**
- [ ] Remove `current_load` field from Kitchen
- [ ] Remove `assign_order()` and `complete_order()` methods
- [ ] Use ResourceAllocator's built-in tracking
- [ ] Update tests

---

#### Task 3.11: Add Comprehensive Integration Tests (1-2 hours)
**File:** `crates/sim-kitchen/tests/integration.rs`

**Actions:**
- [ ] Test complete customer lifecycle
- [ ] Test kitchen capacity constraints (create kitchen with capacity 1, send 3 orders)
- [ ] Test concurrent orders (multiple customers ordering simultaneously)
- [ ] Test event emission sequence
- [ ] Test workflow progression

---

### Phase 3 Success Criteria

- ✅ WorkflowEngine under 700 lines (down from 1,100)
- ✅ Batch update API demonstrates 10-100x speedup
- ✅ sim-geo code reduced by ~140 lines
- ✅ sim-kitchen shows complete workflow + event lifecycle
- ✅ Performance benchmarks validate improvements
- ✅ All tests passing (230+)

---

## Phase 4: Continue Framework Development 🚀

**Priority:** NEW FEATURES
**Duration:** Variable (5-10 weeks depending on parallelization)
**Impact:** Framework completion

After completing Phases 1-3 (or at minimum Phase 1), continue with remaining framework phases.

---

### Available Tracks

#### Option A: Phase 7 - Road Network Integration
**Duration:** 5-7 days
**Dependencies:** None (can start immediately after Phase 1)

**Tasks:**
- Port routing from universe crate
- Implement Dijkstra pathfinding
- Add TimeOfDay velocity model
- Integrate with LocationHandler

**See:** `phases/PHASE_07_ROAD_NETWORK.md`

---

#### Option B: Phase 8 - Telemetry Generation
**Duration:** 5-7 days
**Dependencies:** None (can start immediately after Phase 1)

**Tasks:**
- Implement log/trace/metric generation
- Add OTEL export formats
- Leverage snapshot system for time-series
- Create telemetry examples

**See:** `phases/PHASE_08_TELEMETRY.md`

---

#### Option C: Phase 9 - Multiple Concurrent Workflows
**Duration:** 10-14 days
**Dependencies:** Phase 5 ✅ (complete)

**Tasks:**
- Implement supply chain workflows
- Add inventory management
- Run 3 workflows simultaneously
- Validate multi-workflow coordination

**See:** `phases/PHASE_09_MULTI_WORKFLOW.md`

---

### Recommended Execution Strategy

**Parallel Execution (Fastest - 7-9 weeks remaining):**
1. Phase 1: Critical Fixes (2 days) - **MUST DO FIRST**
2. Phase 2 + 3: Documentation & Simplification in parallel (1 day with 3 agents)
3. Phase 7 + 8: Road Network + Telemetry in parallel (5-7 days with 2 agents)
4. Phase 9: Multi-Workflow (10-14 days)
5. Phase 10-14: Remaining features (4-5 weeks)

**Sequential Execution (Conservative - 10-12 weeks remaining):**
1. Phase 1: Critical Fixes (2 days)
2. Phase 2: Documentation Cleanup (0.5 days)
3. Phase 3: Code Simplification (3 days)
4. Phase 7: Road Network (5-7 days)
5. Phase 8: Telemetry (5-7 days)
6. Phase 9-14: Remaining features (6-8 weeks)

---

## Timeline Summary

### Minimum Path (Phase 1 Only)
**Duration:** 1-2 days
**Result:** Bug-free codebase, safe to continue with features
**Remaining work:** Documentation debt + code complexity remains

### Recommended Path (Phase 1 + 2)
**Duration:** 2-3 days
**Result:** Bug-free + clean documentation
**Remaining work:** Code complexity remains but not blocking

### Ideal Path (Phase 1 + 2 + 3)
**Duration:** 5-6 days
**Result:** Bug-free + clean docs + simplified codebase
**Remaining work:** Only new features remain

---

## Risk Assessment

**Overall Risk:** ✅ LOW-MEDIUM

### Mitigations

| Risk | Mitigation |
|------|-----------|
| Breaking changes during refactor | Comprehensive test suite (221 tests) validates correctness |
| Performance regression | Add benchmarks in Phase 3 to measure improvements |
| Documentation becomes stale | Single source of truth in README.md prevents divergence |
| Scope creep in simplification | Phase 3 is optional, can be deferred or run in parallel |

---

## Success Metrics

### Phase 1 Complete
- ✅ Zero critical bugs
- ✅ All tests passing
- ✅ sim-kitchen lifecycle complete
- ✅ Code reduced by ~400 lines

### Phase 2 Complete
- ✅ Documentation reduced by 84%
- ✅ Single source of truth established
- ✅ Clear navigation structure

### Phase 3 Complete
- ✅ Performance benchmarks added
- ✅ Batch operations 10-100x faster
- ✅ Code complexity reduced

### Framework Complete (Phase 1-14)
- ✅ All 14 phases complete
- ✅ Multiple domains implemented
- ✅ Python bindings working
- ✅ Production-ready with <100ms per tick for 10k agents

---

## Next Session Recommendations

### High Priority (Do This)
1. **Execute Phase 1** - Fix all critical bugs (1-2 days)
2. **Execute Phase 2** - Clean up documentation (0.5 days)

### Medium Priority (Consider This)
3. **Execute Phase 3** - Simplify codebase (2-3 days)
   - Can be done in parallel with Phase 7/8
   - Improves developer experience significantly

### Low Priority (Optional)
4. Start Phase 7 or 8 immediately after Phase 1
   - Faster to new features
   - Accepts some technical debt

---

## References

- **Current Status:** `CURRENT_STATUS.md`
- **Code Review Details:** `CODE_REVIEW_SUMMARY.md`
- **Documentation Cleanup:** `DOCUMENTATION_CLEANUP_PLAN.md`
- **Phase Plans:** `phases/PHASE_XX_NAME.md`
- **Completed Phases:** `completed/PHASE_X_COMPLETE.md` (after Phase 2)

---

## Conclusion

The framework is **38% complete** with **solid foundations** but requires **critical bug fixes** before continuing. The recommended path is:

1. **Phase 1** (1-2 days) - Fix critical bugs ← **DO THIS FIRST**
2. **Phase 2** (0.5 days) - Clean up docs ← **High value, low effort**
3. **Phase 3** (2-3 days) - Simplify code ← **Can run parallel with Phase 7/8**
4. **Phase 7+8** (5-7 days) - New features in parallel

This approach ensures a **bug-free, maintainable, high-quality** codebase before accelerating toward completion.

**Total time to framework completion:** 7-12 weeks depending on execution strategy.
