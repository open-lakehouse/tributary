# Framework Documentation

**Welcome to the Generic Agent Simulation Framework documentation!**

This directory contains comprehensive plans and guides for completing the framework implementation.

---

## Quick Navigation

### 🎯 Start Here

**[Main Completion Plan](FRAMEWORK_COMPLETION_PLAN.md)**
- Executive summary of framework status (70% complete)
- High-level roadmap (Phases 5-14)
- Success criteria and metrics
- How to pick up work in a new session

### 🔀 Working in Parallel?

**[Parallel Execution Guide](PARALLEL_EXECUTION_GUIDE.md)**
- Coordination strategies for multiple agents
- Git branching conventions
- Dependency graph
- Sync points and merge order

---

## Phase-Specific Plans

Each phase has a detailed implementation plan with:
- Specific tasks and file locations
- Code examples
- Acceptance criteria
- Testing strategies
- Time estimates

### Critical Path (Must Complete First)

1. **[Phase 5: Workflow Integration](phases/PHASE_05_WORKFLOW_INTEGRATION.md)** ⏱️ 2 weeks
   - Connect WorkflowEngine to SimulationRunner
   - Resource-workflow coordination
   - End-to-end workflow execution
   - **Status**: ✅ COMPLETE
   - **Blocks**: Phase 9, Phase 11

2. **[Phase 6: Arrow Migration](phases/PHASE_06_ARROW_MIGRATION.md)** ⏱️ 3 weeks
   - Migrate to RecordBatch storage
   - Implement snapshots
   - 10-100x performance improvement
   - **Status**: NOT STARTED
   - **Parallelizable**: 4 independent tracks!

### Independent Phases (Can Run Parallel)

3. **[Phase 7: Road Network Integration](phases/PHASE_07_ROAD_NETWORK.md)** ⏱️ 1 week
   - Port routing from universe crate
   - Dijkstra pathfinding
   - TimeOfDay velocity model
   - **Status**: NOT STARTED
   - **Can start**: Immediately

4. **[Phase 8: Telemetry Generation](phases/PHASE_08_TELEMETRY.md)** ⏱️ 1 week
   - Log/trace/metric generation
   - OTEL export formats
   - **Status**: NOT STARTED
   - **Can start**: Immediately

### Multi-Workflow & Validation

5. **[Phase 9: Multiple Concurrent Workflows](phases/PHASE_09_MULTI_WORKFLOW.md)** ⏱️ 2 weeks
   - Supply chain workflows
   - Inventory management
   - 3 workflows running simultaneously
   - **Status**: NOT STARTED
   - **Depends on**: Phase 5

6. **[Phase 10-11: Demand & Logistics Domain](phases/PHASE_10_11_DOMAINS.md)** ⏱️ 2 weeks
   - Demand generators (Poisson, TimeOfDay, Spatial)
   - Second domain implementation
   - Framework validation
   - **Status**: NOT STARTED
   - **Can start**: Anytime (Phase 11 benefits from Phase 5)

### Integration & Polish

7. **[Phase 12-14: Integration, CLI, Performance](phases/PHASE_12_13_14_INTEGRATION.md)** ⏱️ 3 weeks
   - Python bindings
   - CLI updates
   - Documentation
   - Performance benchmarks
   - Production hardening
   - **Status**: NOT STARTED
   - **Depends on**: All previous phases

---

## Timeline Overview

### Sequential Execution: 14 weeks
```
Phase 5 (2w) → Phase 6 (3w) → Phase 7 (1w) → Phase 8 (1w) →
Phase 9 (2w) → Phase 10-11 (2w) → Phase 12-14 (3w)
```

### Parallel Execution: 10-12 weeks
```
Wave 1 (2w): Phase 5 + 7 + 8 (3 agents)
Wave 2 (3w): Phase 6 parts A+B+C+D (4 agents)
Wave 3 (2w): Phase 9 + 10 (2 agents)
Wave 4 (2w): Phase 11 (1-2 agents)
Wave 5 (3w): Phase 12 + 13 + 14 (3 agents)
```

**See [Parallel Execution Guide](PARALLEL_EXECUTION_GUIDE.md) for details.**

---

## Status Dashboard

| Phase | Name | Priority | Status | Est. Time | Depends On |
|-------|------|----------|--------|-----------|------------|
| 5 | Workflow Integration | 🔴 CRITICAL | ✅ COMPLETE | 2 weeks | None |
| 6 | Arrow Migration | 🔴 CRITICAL | ⏳ NOT STARTED | 3 weeks | None |
| 7 | Road Network | 🟡 MEDIUM | ⏳ NOT STARTED | 1 week | None |
| 8 | Telemetry | 🟡 MEDIUM | ⏳ NOT STARTED | 1 week | None |
| 9 | Multi-Workflow | 🔴 CRITICAL | ⏳ NOT STARTED | 2 weeks | Phase 5 ✅ |
| 10 | Demand Generation | 🟢 VALIDATION | ⏳ NOT STARTED | 1 week | None |
| 11 | Logistics Domain | 🟢 VALIDATION | ⏳ NOT STARTED | 1 week | Phase 5* ✅ |
| 12 | Python Bindings | 🟢 INTEGRATION | ⏳ NOT STARTED | 1 week | Phase 5-11 |
| 13 | CLI & Docs | 🟢 INTEGRATION | ⏳ NOT STARTED | 1 week | Phase 5-11 |
| 14 | Performance | 🟢 PRODUCTION | ⏳ NOT STARTED | 1 week | Phase 5-11 |

*Recommended but not strictly required

**Total Progress**: 5/14 phases complete (36%) → 9/14 remaining (64%)

---

## How to Use These Docs

### New Session Starting Fresh

1. Read [Main Plan](../FRAMEWORK_COMPLETION_PLAN.md) - Executive Summary section
2. Check **Phase Status Tracker** (in main plan)
3. Choose available phase (green checkboxes empty)
4. Read corresponding phase plan (`phases/PHASE_XX_NAME.md`)
5. Create feature branch: `feat/phase-XX-name`
6. Start working through tasks
7. Update status when starting and completing

### Picking Up Existing Work

1. Check last `PHASE_X_COMPLETE.md` file to see what's done
2. Read [Main Plan](../FRAMEWORK_COMPLETION_PLAN.md) - Current State section
3. Find next phase with satisfied dependencies
4. Read phase plan
5. Continue from where previous session left off

### Working in Parallel

1. Read [Parallel Execution Guide](PARALLEL_EXECUTION_GUIDE.md)
2. Check **Parallel Execution Waves** section
3. Coordinate with other agents on communication channel
4. Follow branching and merge conventions
5. Attend sync points every 2 weeks

---

## File Locations Reference

### Framework Crates
```
crates/
├── sim-core/              Core simulation engine
├── sim-geo/               Geospatial & movement
├── sim-events/            Event infrastructure
├── sim-telemetry/         Telemetry generation (empty)
└── sim-kitchen/           Ghost kitchen domain
```

### Domain Crates (To Be Created)
```
crates/domains/
├── ghost-kitchen/         Ghost kitchen rebuilt (Phase 9)
└── logistics/             Logistics domain (Phase 11)
```

### Key Files Frequently Modified
- `crates/sim-core/src/runner/mod.rs:242` - Workflow integration point
- `crates/sim-core/src/population/handler.rs` - Agent storage
- `crates/sim-core/src/resources/allocator.rs` - Resource allocation
- `crates/sim-core/src/workflows/engine.rs` - Workflow processing
- `crates/sim-geo/src/movement/handler.rs` - Journey management

---

## Success Criteria (Overall)

### Functional
- [ ] Workflows execute automatically in simulation
- [ ] Multiple workflow types run concurrently
- [ ] Arrow RecordBatch storage for all state
- [ ] Snapshot save/restore works
- [ ] Second domain (logistics) validates framework
- [ ] Supply chain simulation operational

### Performance
- [ ] 10k agents @ 1-min steps: <100ms per tick
- [ ] 1M events emit + persist: <1 second
- [ ] Memory scales linearly with population
- [ ] 10-100x speedup vs HashMap storage

### Production
- [ ] >85% test coverage
- [ ] Zero clippy warnings
- [ ] Comprehensive documentation
- [ ] Python bindings working
- [ ] CLI supports all operations

---

## Questions?

- **Implementation questions**: See specific phase plan
- **Coordination questions**: See [Parallel Execution Guide](PARALLEL_EXECUTION_GUIDE.md)
- **Architecture questions**: See [Main Plan](../FRAMEWORK_COMPLETION_PLAN.md) - Design Decisions section
- **Stuck?**: Create GitHub issue with `help-wanted` label

---

## Documentation Structure

```
docs/
├── README.md                          ← You are here
├── PARALLEL_EXECUTION_GUIDE.md        Coordination guide
└── phases/
    ├── PHASE_05_WORKFLOW_INTEGRATION.md
    ├── PHASE_06_ARROW_MIGRATION.md
    ├── PHASE_07_ROAD_NETWORK.md
    ├── PHASE_08_TELEMETRY.md
    ├── PHASE_09_MULTI_WORKFLOW.md
    ├── PHASE_10_11_DOMAINS.md
    └── PHASE_12_13_14_INTEGRATION.md
```

---

**Ready to start?** Pick a phase from the table above and dive into its detailed plan!

**Working with others?** Coordinate using the [Parallel Execution Guide](PARALLEL_EXECUTION_GUIDE.md).

**Questions?** All phase plans include troubleshooting sections and clear acceptance criteria.
