# Parallel Execution Guide

**Purpose**: Coordinate multiple agents/developers working simultaneously on framework completion.

---

## Overview

The framework completion plan has been designed with parallel execution in mind. Multiple phases can be worked on simultaneously with minimal coordination overhead.

**Total Timeline**: 14 weeks sequential → **~8-10 weeks with parallel execution**

---

## Parallel Execution Waves

### Wave 1: Foundation (Weeks 1-2)

**Independent Streams**:

| Agent | Phase | Files Modified | Coordination |
|-------|-------|----------------|--------------|
| **Agent A** | Phase 5: Workflow Integration | `sim-core/src/runner/mod.rs`<br>`sim-core/src/resources/allocator.rs`<br>`sim-core/src/workflows/engine.rs` | **CRITICAL PATH**<br>Other phases can proceed but benefits from completion |
| **Agent B** | Phase 7: Road Network | `sim-geo/src/graph/`<br>`sim-geo/src/movement/handler.rs` | Independent<br>No conflicts |
| **Agent C** | Phase 8: Telemetry | `sim-telemetry/src/`<br>(empty crate, no conflicts) | Independent<br>No conflicts |

**Sync Point**: End of Week 2
- Merge all branches
- Resolve any conflicts (minimal expected)
- Integration test run

---

### Wave 2: Arrow Migration (Weeks 3-5)

**Highly Parallelizable** - Each handler can be migrated independently:

| Agent | Component | Files | Estimated Time |
|-------|-----------|-------|----------------|
| **Agent 1** | PopulationHandler | `sim-core/src/population/handler.rs`<br>`sim-core/src/population/schema.rs` (new) | 1 week |
| **Agent 2** | ResourceAllocator | `sim-core/src/resources/allocator.rs`<br>`sim-core/src/resources/schema.rs` (new) | 1 week |
| **Agent 3** | WorkflowEngine | `sim-core/src/workflows/engine.rs`<br>`sim-core/src/workflows/schema.rs` (new) | 1 week |
| **Agent 4** | LocationHandler | `sim-geo/src/locations/handler.rs`<br>`sim-geo/src/locations/schema.rs` (new) | 1 week |

**Week 3**: All agents work independently
**Week 4**: Continue, start integration testing
**Week 5**:
- **Agent 1** (after completing PopulationHandler): Implement snapshot system (uses all handlers)
- **Agents 2-4**: Assist with integration, testing, documentation

**Sync Point**: End of Week 5
- All handlers migrated
- Snapshot system complete
- Integration tests passing
- Merge to main

---

### Wave 3: Multi-Workflow (Weeks 6-7)

| Agent | Phase | Dependency | Coordination |
|-------|-------|------------|--------------|
| **Agent A** | Phase 9: Multi-Workflow | Requires Phase 5 complete | Uses workflow integration |
| **Agent B** | Phase 10: Demand Generation | Independent | Can work in parallel |

**Option**: If Phase 5 not complete, Agent A can help with Phase 6 (Arrow) or Phase 10 (Demand).

**Sync Point**: End of Week 7

---

### Wave 4: Validation (Weeks 8-9)

| Agent | Phase | Files | Coordination |
|-------|-------|-------|--------------|
| **Agent A** | Phase 10: Demand Generation | `sim-core/src/demand/models.rs` | Independent |
| **Agent B** | Phase 11: Logistics Domain | `crates/domains/logistics/` (new crate) | Independent |

**No conflicts** - Different directories entirely.

**Sync Point**: End of Week 9
- Both phases complete
- Framework validated with 2 domains
- Integration tests passing

---

### Wave 5: Integration & Polish (Weeks 10-12)

| Agent | Phase | Focus | Coordination |
|-------|-------|-------|--------------|
| **Agent A** | Phase 12: Python Bindings | `crates/python-bindings/`<br>`notebooks/` | Independent |
| **Agent B** | Phase 13: CLI + Docs | `crates/cli/src/main.rs`<br>`docs/` | Independent |
| **Agent C** | Phase 14: Benchmarks | `benches/`<br>CI/CD setup | Independent |

**Week 10**: All agents work independently
**Week 11**: Continue, start cross-validation
**Week 12**: Integration, final polish

**Sync Point**: End of Week 12
- All integration complete
- Ready for production

---

## Git Branch Strategy

### Branch Naming Convention

```bash
# For single-phase work
feat/phase-<number>-<short-name>

# Examples:
feat/phase-05-workflow-integration
feat/phase-07-road-network
feat/phase-08-telemetry

# For multi-agent parallel work on same phase
feat/phase-06-<component>-<initials>

# Examples:
feat/phase-06-population-alice
feat/phase-06-resources-bob
feat/phase-06-workflows-charlie
feat/phase-06-locations-diana
```

### Workflow

1. **Create Branch from Main**
   ```bash
   git checkout main
   git pull origin main
   git checkout -b feat/phase-<number>-<name>
   ```

2. **Commit Often**
   ```bash
   git add -A
   git commit -m "Phase <number>: <task description>"
   git push origin feat/phase-<number>-<name>
   ```

3. **Keep Updated with Main** (if working for >1 week)
   ```bash
   git checkout main
   git pull origin main
   git checkout feat/phase-<number>-<name>
   git rebase main
   # Resolve conflicts if any
   git push --force-with-lease
   ```

4. **Ready for Merge**
   - Create `PHASE_<number>_COMPLETE.md`
   - Update phase status in `FRAMEWORK_COMPLETION_PLAN.md`
   - Run all tests: `cargo test --all`
   - Create PR with completion summary

---

## Coordination Mechanisms

### Daily Standups (Async)

Post in shared channel:
```
Agent: Alice
Phase: 5 (Workflow Integration)
Yesterday: Implemented workflow_instance_id in Task trait
Today: Adding get_completed_tasks() to ResourceAllocator
Blockers: None
ETA: On track for Week 2 completion
```

### Sync Points (Required)

**Every 2 weeks**: All agents sync
1. Merge all in-progress branches to `dev` branch
2. Run full test suite
3. Resolve any integration issues
4. Demo progress
5. Plan next 2 weeks

### Conflict Resolution

**File Conflicts** (rare, different phases touch different files):
1. Person finishing first merges to main
2. Person finishing second rebases on main, resolves conflicts
3. If complex: pair programming session to resolve

**Design Conflicts** (API changes affecting multiple phases):
1. Post in shared design doc
2. Async discussion with 24-hour decision deadline
3. If no consensus: video call to resolve

---

## Communication Channels

### Shared Documents

1. **This Plan** - `FRAMEWORK_COMPLETION_PLAN.md`
   - Single source of truth for status
   - Update phase status when starting/completing work

2. **Design Decisions** - `docs/DESIGN_DECISIONS.md` (create if needed)
   - Record any API changes
   - Document trade-offs
   - Reference for future phases

3. **Blockers** - GitHub Issues
   - Create issue if blocked
   - Tag relevant agents
   - Link to phase plan

### Status Updates

**Starting a Phase**:
```bash
# Update FRAMEWORK_COMPLETION_PLAN.md
# Change phase status to "IN PROGRESS"
# Add your name to "Assigned" column
git add FRAMEWORK_COMPLETION_PLAN.md
git commit -m "Phase <number>: Starting work (assigned to <name>)"
git push origin feat/phase-<number>-<name>
```

**Completing a Phase**:
```bash
# Create PHASE_<number>_COMPLETE.md
cat > PHASE_<number>_COMPLETE.md << EOF
# Phase <number>: <Name> - COMPLETE

## Summary
<What was delivered>

## Files Changed
- <list>

## Tests Added
- <list>

## Next Steps
- <what can now proceed>
EOF

# Update FRAMEWORK_COMPLETION_PLAN.md
# Change status to "✅ COMPLETE"
git add PHASE_<number>_COMPLETE.md FRAMEWORK_COMPLETION_PLAN.md
git commit -m "Phase <number>: Complete"
```

---

## Dependency Graph

```
Phase 5 (Workflow) ──────┬────> Phase 9 (Multi-Workflow)
                         │
                         └────> Phase 11 (Logistics)

Phase 6 (Arrow) ─────────────> Phase 14 (Performance)

Phase 7 (Road Network) ──────> Realistic simulations

Phase 8 (Telemetry) ─────────> Observability

Phase 9 (Multi-Workflow) ────> Supply chain demo

Phase 10 (Demand) ───────────> Realistic patterns

Phase 5-11 Complete ─────────> Phase 12-14 (Integration)
```

**Critical Path**: Phase 5 → Phase 9 → Phase 11 → Phase 12-14

**Parallel Paths**:
- Phase 6 (Arrow) - Independent
- Phase 7 (Road Network) - Independent
- Phase 8 (Telemetry) - Independent
- Phase 10 (Demand) - Independent after Phase 5

---

## Testing Strategy for Parallel Work

### Unit Tests (Always Required)
```bash
# Before pushing any commit
cargo test --lib

# For specific crate
cd crates/sim-core
cargo test
```

### Integration Tests (Before Merging)
```bash
# Full test suite
cargo test --all

# Include integration tests
cargo test --test '*'
```

### Cross-Phase Integration (At Sync Points)
```bash
# After merging multiple branches
cargo test --all --release

# Run examples
cargo run --example ghost_kitchen_basic
cargo run --example logistics_basic

# Benchmarks (if Phase 14 started)
cargo bench
```

---

## Merge Order (Important!)

When multiple phases complete simultaneously, merge in this order:

1. **Phase 5** (Workflow Integration) - Others may depend on this
2. **Phase 6** (Arrow Migration) - May affect performance tests
3. **Phases 7, 8, 10** (Independent) - Any order
4. **Phase 9** (Multi-Workflow) - Depends on Phase 5
5. **Phase 11** (Logistics) - Validation, depends on Phase 9
6. **Phases 12-14** (Integration) - Last

**Rationale**: Merge dependencies first, integrations last.

---

## Example: 4 Agents Working in Parallel (Week 3)

### Initial Setup (Monday Morning)

**Agent Alice**:
```bash
git checkout main && git pull
git checkout -b feat/phase-06-population-alice
# Start PopulationHandler Arrow migration
```

**Agent Bob**:
```bash
git checkout main && git pull
git checkout -b feat/phase-06-resources-bob
# Start ResourceAllocator Arrow migration
```

**Agent Charlie**:
```bash
git checkout main && git pull
git checkout -b feat/phase-06-workflows-charlie
# Start WorkflowEngine Arrow migration
```

**Agent Diana**:
```bash
git checkout main && git pull
git checkout -b feat/phase-06-locations-diana
# Start LocationHandler Arrow migration
```

### Daily Work (Tuesday-Thursday)

Each agent:
1. Works on their component
2. Commits progress daily
3. Runs unit tests before committing
4. Posts async standup update

### Integration (Friday)

**Agent Alice** (finishes first):
```bash
# Create completion doc
cat > PHASE_06A_POPULATION_COMPLETE.md << EOF
## PopulationHandler Arrow Migration - COMPLETE
- RecordBatch storage implemented
- All tests passing (15 new tests)
- Ready for integration
EOF

# Merge to dev
git checkout dev
git pull origin dev
git merge feat/phase-06-population-alice
git push origin dev
```

**Agents B, C, D**:
```bash
# Rebase on dev to get Alice's changes
git checkout dev && git pull
git checkout feat/phase-06-resources-bob
git rebase dev
# Resolve conflicts if any
git push --force-with-lease

# Continue work
```

### Week End Sync (Friday 4pm)

All agents join call:
1. Review integration test results
2. Discuss any issues found
3. Plan next week (snapshot system)
4. Assign Agent Alice to snapshot (depends on all handlers)

---

## Troubleshooting Parallel Execution

### Issue: Merge Conflicts

**Prevention**:
- Work on different files when possible
- Communicate API changes early
- Rebase on main weekly

**Resolution**:
```bash
git rebase main
# Fix conflicts in editor
git add <resolved-files>
git rebase --continue
cargo test --all  # Ensure still works
git push --force-with-lease
```

### Issue: Dependent Phase Blocked

**Example**: Phase 9 needs Phase 5, but Phase 5 delayed.

**Options**:
1. Agent A helps with Phase 5 to unblock
2. Agent A works on independent task (Phase 10)
3. Agent A does preparatory work for Phase 9 (design, tests)

### Issue: Integration Test Failures After Merge

**Protocol**:
1. Identify which merge caused failure
2. Original author fixes (or pair with another agent)
3. Create hotfix branch: `hotfix/phase-<number>-integration`
4. Fix, test, merge immediately
5. Other agents rebase on fix

### Issue: Design Decision Needed Across Phases

**Example**: Should Task trait method be `workflow_id()` or `workflow_instance_id()`?

**Protocol**:
1. Agent encountering issue creates design doc in `docs/decisions/`
2. Posts link in shared channel with @mention of affected agents
3. 24-hour async discussion
4. If no consensus by deadline: 30-min video call
5. Decision recorded in doc with rationale

---

## Success Metrics for Parallel Execution

**Velocity**:
- [ ] Complete 2+ phases simultaneously each wave
- [ ] Sync points occur on schedule (every 2 weeks)
- [ ] Minimal integration issues (<1 day to resolve)

**Quality**:
- [ ] All tests pass at each sync point
- [ ] No merge conflicts take >4 hours to resolve
- [ ] Code review turnaround <24 hours

**Coordination**:
- [ ] Daily async standups posted by all agents
- [ ] Design decisions documented
- [ ] Status updates in plan accurate

---

## Final Timeline with Parallel Execution

| Weeks | Phases | Agents | Deliverable |
|-------|--------|--------|-------------|
| 1-2 | 5, 7, 8 | 3 | Workflows + Road Network + Telemetry |
| 3-5 | 6 (A,B,C,D) | 4 | Arrow Migration Complete |
| 6-7 | 9, 10 | 2 | Multi-Workflow + Demand |
| 8-9 | 11 | 1-2 | Logistics Domain |
| 10-12 | 12, 13, 14 | 3 | Integration + Polish |

**Total**: **12 weeks with parallel execution** (vs 14 weeks sequential)

---

## Getting Started with Parallel Work

1. **Read this guide** completely
2. **Review phase plan** you're assigned to (`docs/phases/PHASE_<number>_<name>.md`)
3. **Check dependencies** in main plan (`FRAMEWORK_COMPLETION_PLAN.md`)
4. **Create branch** using naming convention
5. **Post starting update** (update main plan, async standup)
6. **Begin work** following phase plan tasks
7. **Commit often** with descriptive messages
8. **Test continuously** (`cargo test` before each push)
9. **Communicate blockers** immediately
10. **Complete phase** and post completion update

---

**Questions?** Create GitHub issue tagged with `coordination` label.

**Need help?** Tag relevant agents in issue or shared channel.

**Stuck?** Don't stay blocked >4 hours - reach out!
