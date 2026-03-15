# Generic Agent Simulation Framework - Completion Plan

**Status**: 73% Complete (Phases 1-5 done, Phases 6-14 remaining)
**Last Updated**: 2026-03-15
**Total Estimated Time**: 12 weeks remaining (~3 months)

---

## Executive Summary

The generic agent-based simulation framework has successfully established core abstractions and architectural patterns. However, critical functionality remains incomplete, preventing production use and multi-workflow simulations.

### What Works ✅
- **Core trait system**: Agent, Resource, Task, Workflow, Location traits defined and tested
- **Generic handlers**: PopulationHandler, ResourceAllocator, WorkflowEngine, LocationHandler, MovementHandler
- **Event infrastructure**: CloudEvents 1.0 compliant event emission and Parquet storage
- **Spatial indexing**: H3-based location queries and proximity searches
- **Simulation orchestration**: SimulationRunner with lifecycle hooks
- **Domain example**: Ghost kitchen simulation demonstrating framework usage

### What's Missing ❌
- **Workflow integration**: WorkflowEngine not connected to SimulationRunner tick loop
- **Resource-workflow coordination**: Tasks don't trigger workflow advancement
- **Arrow storage**: Still using HashMap instead of RecordBatch (performance issue)
- **State persistence**: No snapshot save/restore capability
- **Road network routing**: Journey planning not implemented (only distance-based progress)
- **Telemetry generation**: Empty sim-telemetry crate, no log/trace/metric generation
- **Multiple workflows**: Cannot run concurrent workflow types (food ordering + delivery + supply chain)
- **Demand generation**: Only trait defined, no implementations (Poisson, TimeOfDay, Spatial)
- **Second domain**: Logistics domain not implemented (framework generality unproven)

### Critical Path to Completion

**Priority 1**: Workflow Integration (Phase 5)
**Priority 2**: Arrow Migration + Snapshots (Phase 6)
**Priority 3**: Multiple Concurrent Workflows (Phase 9)
**Priority 4**: Validation & Production Hardening (Phases 11-14)

---

## Current State Assessment

### Crate Structure
```
crates/
├── universe/              # EXISTING: Original ghost kitchen (reference)
├── cli/                   # EXISTING: TUI (needs update for new framework)
├── sim-core/              ✅ COMPLETE: Generic simulation engine (40 tests passing)
├── sim-geo/               ✅ COMPLETE: Geospatial/movement abstractions (13 tests)
├── sim-events/            ✅ COMPLETE: Event infrastructure (12 tests)
├── sim-telemetry/         ❌ EMPTY: Data generation hooks (planned, not implemented)
└── sim-kitchen/           ⚠️  PARTIAL: Ghost kitchen domain (14 tests, workflow not integrated)
```

### Test Coverage
- **sim-core**: 40 tests passing, 85%+ coverage
- **sim-geo**: 13 tests passing
- **sim-events**: 12 tests passing
- **sim-kitchen**: 14 tests passing
- **Total**: 79 tests, all green ✅

### Performance Status
- **Current**: HashMap-based storage, not benchmarked
- **Target**: Arrow RecordBatch with 10-100x speedup
- **Gap**: No benchmark suite exists yet

---

## Roadmap Overview

### Phase 5: Workflow Integration (Weeks 1-2) 🔴 CRITICAL
**Goal**: Make workflows actually work in simulation tick loop
**Status**: NOT STARTED
**Blocks**: Phase 9 (multi-workflow), Phase 11 (logistics domain)
**Parallel**: Can work simultaneously with Phase 7 (road network)

[Detailed Plan →](phases/PHASE_05_WORKFLOW_INTEGRATION.md)

### Phase 6: Arrow Migration (Weeks 3-5) 🔴 CRITICAL
**Goal**: Migrate all state to Arrow RecordBatch, implement snapshots
**Status**: NOT STARTED
**Blocks**: Performance optimization, large-scale simulations
**Parallel**: Handlers can be migrated independently (4 parallel tasks)

[Detailed Plan →](phases/PHASE_06_ARROW_MIGRATION.md)

### Phase 7: Road Network Integration (Week 6) 🟡 MEDIUM
**Goal**: Port road network routing from universe crate
**Status**: NOT STARTED
**Blocks**: Realistic delivery time estimates
**Parallel**: Independent of Phases 5, 6, 8

[Detailed Plan →](phases/PHASE_07_ROAD_NETWORK.md)

### Phase 8: Telemetry Generation (Week 7) 🟡 MEDIUM
**Goal**: Implement telemetry generators and OTEL export
**Status**: NOT STARTED
**Blocks**: Observability data generation
**Parallel**: Independent of Phases 5, 6, 7

[Detailed Plan →](phases/PHASE_08_TELEMETRY.md)

### Phase 9: Multiple Concurrent Workflows (Weeks 8-9) 🔴 CRITICAL
**Goal**: Support food ordering + courier delivery + supply chain simultaneously
**Status**: NOT STARTED
**Blocks**: Supply chain simulation
**Depends On**: Phase 5 (workflow integration)

[Detailed Plan →](phases/PHASE_09_MULTI_WORKFLOW.md)

### Phase 10-11: Demand & Logistics Domain (Weeks 10-11) 🟢 VALIDATION
**Goal**: Implement demand generators, build logistics domain
**Status**: NOT STARTED
**Blocks**: Framework validation, generality proof
**Parallel**: Demand generators (10) and logistics domain (11) can run parallel

[Detailed Plan →](phases/PHASE_10_11_DOMAINS.md)

### Phase 12-14: Integration & Polish (Weeks 12-14) 🟢 PRODUCTION
**Goal**: Python bindings, CLI, docs, performance benchmarks
**Status**: NOT STARTED
**Blocks**: Production readiness
**Parallel**: Multiple tasks can run in parallel (see detailed plan)

[Detailed Plan →](phases/PHASE_12_13_14_INTEGRATION.md)

---

## Parallel Execution Strategy

The following phases can be executed in parallel by different agents/developers:

### Wave 1 (Weeks 1-2): Foundation
- **Agent A**: Phase 5 - Workflow Integration ← CRITICAL PATH
- **Agent B**: Phase 7 - Road Network (independent)

### Wave 2 (Weeks 3-5): Storage & Infrastructure
- **Agent A**: Phase 6.1 - PopulationHandler Arrow migration
- **Agent B**: Phase 6.2 - ResourceAllocator Arrow migration
- **Agent C**: Phase 6.3 - WorkflowEngine Arrow migration
- **Agent D**: Phase 6.4 - LocationHandler Arrow migration
- *(Agent A resumes)* Phase 6.5 - Snapshot system (after 6.1-6.4 complete)

### Wave 3 (Weeks 6-7): Telemetry & Enhancement
- **Agent A**: Phase 8 - Telemetry Generation (independent)
- **Agent B**: Phase 9 - Multi-Workflow (depends on Phase 5)

### Wave 4 (Weeks 8-11): Validation
- **Agent A**: Phase 10 - Demand Generation
- **Agent B**: Phase 11 - Logistics Domain

### Wave 5 (Weeks 12-14): Integration
- **Agent A**: Phase 12 - Python Bindings
- **Agent B**: Phase 13 - CLI + Documentation
- **Agent C**: Phase 14 - Performance Benchmarks

**See [Parallel Execution Guide](docs/PARALLEL_EXECUTION_GUIDE.md) for coordination details.**

---

## How to Pick Up Work (New Session/Agent)

### 1. Check Current Status
```bash
# See what's been completed
ls -la PHASE_*_COMPLETE.md

# Check this plan for phase status
grep "Status:" FRAMEWORK_COMPLETION_PLAN.md
```

### 2. Choose a Phase
- Review **Roadmap Overview** section above
- Check **Depends On** and **Parallel** indicators
- Choose phase that:
  - Prerequisites are complete
  - Not blocked by other work
  - Matches available time (see phase docs for estimates)

### 3. Read Detailed Phase Plan
```bash
# Navigate to phase-specific documentation
cd phases/
cat PHASE_<number>_<name>.md
```

Each phase doc contains:
- Prerequisites checklist
- Specific file locations and line numbers
- Code examples
- Test requirements
- Acceptance criteria

### 4. Create Feature Branch
```bash
# For single phase
git checkout -b feat/phase-<number>-<short-name>

# For parallel work
git checkout -b feat/phase-<number>-<component>-<your-initials>
```

### 5. Execute Tasks
- Follow task order in phase document
- Run tests after each task
- Commit incremental progress
- Update phase doc with completion status

### 6. Mark Phase Complete
```bash
# Create completion summary
cat > PHASE_<number>_COMPLETE.md << EOF
# Phase <number>: <Name> - COMPLETE

## Summary
<What was delivered>

## Files Changed
<List of modified files>

## Tests Added
<New test files and coverage>

## Next Steps
<What subsequent phases can now proceed>
EOF

# Update this plan
# Change phase status from "NOT STARTED" → "COMPLETE"
```

---

## Success Criteria (Overall)

### Functional Completeness
- [ ] Workflows integrate with runner tick loop and execute automatically
- [ ] Resource completion triggers workflow step advancement
- [ ] Multiple workflow types run concurrently (≥3 types: cooking, delivery, supply ordering)
- [ ] Ghost kitchen simulation produces identical results to universe crate
- [ ] Supply chain workflows operational (kitchens order supplies, distributors fulfill)
- [ ] Logistics domain implemented and working
- [ ] All state stored in Arrow RecordBatch
- [ ] Snapshot save/restore works
- [ ] DataFusion queries enabled for all handlers

### Performance Targets
- [ ] 10k agent simulation @ 1-min steps runs in <100ms per step
- [ ] 1M events emit + persist in <1 second
- [ ] Memory usage scales linearly with agent population
- [ ] No performance regression vs universe crate baseline

### Code Quality
- [ ] >85% test coverage on all framework crates
- [ ] All public APIs documented with examples
- [ ] Zero clippy warnings
- [ ] No `.unwrap()` in production code paths
- [ ] Benchmark suite in CI with regression detection

### Deliverables
- [ ] 4 framework crates production-ready (sim-core, sim-geo, sim-events, sim-telemetry)
- [ ] 2 domain examples working (ghost-kitchen, logistics)
- [ ] Python bindings for both domains
- [ ] CLI supporting new framework
- [ ] Comprehensive documentation (guides + API reference + tutorials)

---

## Key Design Decisions

### 1. Arrow vs HashMap Storage
**Decision**: Migrate to Arrow RecordBatch in Phase 6
**Rationale**: 10-100x performance gain, DataFusion query support, columnar efficiency
**Trade-off**: More complex API, but worth it for scale

### 2. Workflow-Resource Integration
**Decision**: Tasks carry workflow_instance_id, ResourceAllocator notifies WorkflowEngine
**Rationale**: Decoupled architecture, testable components
**Alternative Rejected**: Tight coupling would limit flexibility

### 3. Multiple Workflow Types
**Decision**: WorkflowRegistry pattern with type discrimination
**Rationale**: Different workflows need different step logic and events
**Alternative Rejected**: Single monolithic workflow would be inflexible

### 4. Road Network Strategy
**Decision**: Load pre-built graphs from universe crate, skip OSM parsing
**Rationale**: Leverage existing infrastructure, focus on simulation not map processing
**Future**: Could add OSM parsing later if needed

### 5. Event Storage
**Decision**: Parquet for events (defer Delta Lake to later)
**Rationale**: Simpler, faster implementation, good enough for current needs
**Future**: Add Delta Lake for transactional semantics if needed

---

## Risk Register

### Risk 1: Workflow Integration Complexity
**Impact**: HIGH - Blocks multi-workflow simulations
**Probability**: MEDIUM
**Mitigation**: Start with simple integration test, incremental approach, thorough code review

### Risk 2: Arrow Migration Performance
**Impact**: MEDIUM - May not achieve 10x speedup
**Probability**: LOW
**Mitigation**: Benchmark early, profile before/after, optimize hot paths

### Risk 3: Framework Not Generic Enough
**Impact**: HIGH - Logistics domain reveals abstraction gaps
**Probability**: MEDIUM
**Mitigation**: Build logistics domain as validation, iterate on traits if needed

### Risk 4: Timeline Slippage
**Impact**: MEDIUM - Delays production readiness
**Probability**: MEDIUM
**Mitigation**: Parallel execution, prioritize critical path, cut scope if needed

---

## Dependencies Between Phases

```
Phase 5 (Workflow Integration) ──┬──> Phase 9 (Multi-Workflow)
                                  │
                                  └──> Phase 11 (Logistics Domain)

Phase 6 (Arrow Migration) ────────┬──> Phase 14 (Performance Benchmarks)
                                  │
                                  └──> Large-scale simulations

Phase 7 (Road Network) ───────────> Realistic delivery simulations

Phase 8 (Telemetry) ──────────────> Observability validation

Phase 9 (Multi-Workflow) ─────────> Phase 11 (Supply chain demo)

Phase 10 (Demand Gen) ────────────> Realistic order patterns

Phase 11 (Logistics) ─────────────> Framework validation

Phase 5-11 Complete ──────────────> Phase 12-14 (Integration & Polish)
```

---

## Quick Reference: File Locations

### Framework Crates
- **sim-core**: `/Users/robert.pack/code/management/crates/sim-core/`
- **sim-geo**: `/Users/robert.pack/code/management/crates/sim-geo/`
- **sim-events**: `/Users/robert.pack/code/management/crates/sim-events/`
- **sim-telemetry**: `/Users/robert.pack/code/management/crates/sim-telemetry/`

### Domain Crates
- **sim-kitchen**: `/Users/robert.pack/code/management/crates/sim-kitchen/`
- **universe** (reference): `/Users/robert.pack/code/management/crates/universe/`

### Key Files to Modify
- Workflow integration: `crates/sim-core/src/runner/mod.rs:242`
- Resource traits: `crates/sim-core/src/resources/traits.rs`
- Population handler: `crates/sim-core/src/population/handler.rs`
- Movement handler: `crates/sim-geo/src/movement/handler.rs`

---

## Phase Status Tracker

| Phase | Name | Status | Assigned | Start | Complete |
|-------|------|--------|----------|-------|----------|
| 1 | Core Foundation | ✅ COMPLETE | - | - | Done |
| 2 | Geospatial Foundation | ✅ COMPLETE | - | - | Done |
| 3 | Event Infrastructure | ✅ COMPLETE | - | - | Done |
| 4 | Simulation Runner | ✅ COMPLETE | - | - | Done |
| 5 | Workflow Integration | ⏳ NOT STARTED | - | - | - |
| 6 | Arrow Migration | ⏳ NOT STARTED | - | - | - |
| 7 | Road Network | ⏳ NOT STARTED | - | - | - |
| 8 | Telemetry | ⏳ NOT STARTED | - | - | - |
| 9 | Multi-Workflow | ⏳ NOT STARTED | - | - | - |
| 10 | Demand Generation | ⏳ NOT STARTED | - | - | - |
| 11 | Logistics Domain | ⏳ NOT STARTED | - | - | - |
| 12 | Python Bindings | ⏳ NOT STARTED | - | - | - |
| 13 | CLI & Docs | ⏳ NOT STARTED | - | - | - |
| 14 | Performance | ⏳ NOT STARTED | - | - | - |

---

## Contact & Questions

For questions about this plan or to coordinate parallel work:
- Review detailed phase plans in `phases/`
- Check parallel execution guide in `docs/PARALLEL_EXECUTION_GUIDE.md`
- See original plan in `GENERIC_SIMULATION_FRAMEWORK_PLAN.md`

---

**Next Action**: Begin Phase 5 (Workflow Integration) - See [detailed plan](phases/PHASE_05_WORKFLOW_INTEGRATION.md)
