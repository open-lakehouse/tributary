# Documentation Cleanup Plan (Phase 2)

**Created:** March 16, 2026
**Status:** Planning complete, ready for execution
**Duration:** 0.5 days (3-4 hours)
**Priority:** HIGH (Essential for future development context)

---

## Executive Summary

The migration documentation has grown to **8,557 lines across 19 files** with **~40% redundancy**. This plan will reorganize and consolidate docs to reduce reading requirements by **84%** (8,557 → 1,400 essential lines) while maintaining all valuable context.

### Goals
1. **Eliminate redundancy** - Single source of truth (README.md)
2. **Improve navigation** - Clear directory structure
3. **Archive superseded docs** - Preserve history without clutter
4. **Maintain searchability** - Nothing deleted, everything archived

---

## Current State Analysis

### Documentation Inventory

**Total:** 19 files, 8,557 lines

#### Root Level (12 files, 5,282 lines)
- README.md (260 lines) ✅ Keep
- PARALLEL_EXECUTION_GUIDE.md (521 lines) ✅ Keep
- FRAMEWORK_COMPLETION_PLAN.md (404 lines) ⚠️ Archive after extraction
- MIGRATION_STATUS.md (422 lines) ⚠️ Archive after extraction
- PHASE_5_COMPLETE.md (190 lines) ✅ Move to completed/
- PHASE_5_REFINEMENT.md (207 lines) ✅ Move to completed/
- PHASE_5_REVIEW.md (403 lines) ❌ Archive (superseded)
- PHASE_6_WEEK_1_COMPLETE.md (410 lines) ✅ Move to completed/
- PHASE_6_WEEK_1_REFINEMENT.md (439 lines) ✅ Move to completed/
- PHASE_6_WEEK_1_VERIFICATION_REPORT.md (276 lines) ❌ Archive (redundant)
- PHASE_6_WEEK_2_COMPLETE.md (1,168 lines) ✅ Move to completed/
- PHASE_6_WEEK_2_PLAN.md (582 lines) ❌ Archive (now complete)

#### New Files (3 files, created in code review)
- CURRENT_STATUS.md ✅ Keep
- CODE_REVIEW_SUMMARY.md ✅ Keep
- FORWARD_PLAN.md ✅ Keep

#### phases/ Directory (7 files, 3,275 lines)
- PHASE_05_WORKFLOW_INTEGRATION.md (1,261 lines) ❌ Archive (complete)
- PHASE_06_ARROW_MIGRATION.md (420 lines) ❌ Archive (complete)
- PHASE_07_ROAD_NETWORK.md (631 lines) ✅ Keep
- PHASE_08_TELEMETRY.md (118 lines) ✅ Keep
- PHASE_09_MULTI_WORKFLOW.md (324 lines) ✅ Keep
- PHASE_10_11_DOMAINS.md (421 lines) ✅ Keep
- PHASE_12_13_14_INTEGRATION.md (420 lines) ✅ Keep

---

## Redundancy Analysis

### Major Overlaps (40% of content)

| Content | README.md | MIGRATION_STATUS.md | FRAMEWORK_PLAN.md |
|---------|-----------|---------------------|-------------------|
| Status dashboard | Lines 123-142 | Lines 88-98 | Lines 70-136 |
| Phase 6 progress | Lines 48-56 | Lines 21-35 | Lines 83-89 |
| Next steps | Implicit | Lines 104-193 | Scattered |
| Success criteria | Lines 204-225 | Lines 324-343 | Lines 200-250 |
| Timeline estimates | Lines 102-119 | Lines 225-228 | Lines 150-170 |

**Impact:** Same information maintained in 2-3 places, leading to:
- Inconsistent updates
- Confusion about which is canonical
- Wasted reading time

---

## Proposed Structure

```
migration/
├── README.md                          # ✅ Single source of truth (enhanced)
├── CURRENT_STATUS.md                  # ✅ Latest code review findings
├── CODE_REVIEW_SUMMARY.md             # ✅ Detailed crate reviews
├── FORWARD_PLAN.md                    # ✅ Implementation roadmap
├── PARALLEL_EXECUTION_GUIDE.md        # ✅ Multi-agent coordination
│
├── completed/                         # 📁 Historical records (5 files)
│   ├── PHASE_5_COMPLETE.md
│   ├── PHASE_5_REFINEMENT.md
│   ├── PHASE_6_WEEK_1_COMPLETE.md
│   ├── PHASE_6_WEEK_1_REFINEMENT.md
│   └── PHASE_6_WEEK_2_COMPLETE.md
│
├── phases/                            # 📁 Future work plans (5 files)
│   ├── PHASE_07_ROAD_NETWORK.md
│   ├── PHASE_08_TELEMETRY.md
│   ├── PHASE_09_MULTI_WORKFLOW.md
│   ├── PHASE_10_11_DOMAINS.md
│   └── PHASE_12_13_14_INTEGRATION.md
│
└── archive/                           # 📁 Superseded/old docs (9 files)
    ├── phase5/
    │   ├── PHASE_5_REVIEW.md
    │   └── PHASE_05_WORKFLOW_INTEGRATION.md
    ├── phase6/
    │   ├── PHASE_06_ARROW_MIGRATION.md
    │   ├── PHASE_6_WEEK_1_VERIFICATION_REPORT.md
    │   └── PHASE_6_WEEK_2_PLAN.md
    └── planning/
        ├── FRAMEWORK_COMPLETION_PLAN.md
        └── MIGRATION_STATUS.md
```

---

## Implementation Tasks

### Task 1: Create Directory Structure (5 mins)

**Actions:**
```bash
mkdir -p migration/completed
mkdir -p migration/archive/phase5
mkdir -p migration/archive/phase6
mkdir -p migration/archive/planning
```

**Verification:**
```bash
ls -la migration/
# Should show: completed/, archive/, phases/, + root files
```

---

### Task 2: Extract Unique Content (30 mins)

#### From MIGRATION_STATUS.md

**Unique content to extract:**
- Lines 39-87: Code review findings

**Destination:** Merge into PHASE_6_WEEK_2_COMPLETE.md

**Actions:**
1. Read MIGRATION_STATUS.md lines 39-87
2. Open PHASE_6_WEEK_2_COMPLETE.md
3. Add new section "## Code Review Findings (Added March 16, 2026)"
4. Paste extracted content
5. Save file

#### From FRAMEWORK_COMPLETION_PLAN.md

**Unique content to extract:**
- Design Decisions section (if exists)
- Architecture philosophy
- Key patterns (RecordBatch + HashMap, TaskFactory, etc.)

**Destination:** Create new `docs/ARCHITECTURE.md` (see Task 5)

**Actions:**
1. Review FRAMEWORK_COMPLETION_PLAN.md for design philosophy
2. Extract sections not in README.md or other docs
3. Will be incorporated into ARCHITECTURE.md

---

### Task 3: Move Completion Docs (10 mins)

**Files to move:**
- PHASE_5_COMPLETE.md → completed/
- PHASE_5_REFINEMENT.md → completed/
- PHASE_6_WEEK_1_COMPLETE.md → completed/
- PHASE_6_WEEK_1_REFINEMENT.md → completed/
- PHASE_6_WEEK_2_COMPLETE.md → completed/ (after Task 2)

**Commands:**
```bash
mv migration/PHASE_5_COMPLETE.md migration/completed/
mv migration/PHASE_5_REFINEMENT.md migration/completed/
mv migration/PHASE_6_WEEK_1_COMPLETE.md migration/completed/
mv migration/PHASE_6_WEEK_1_REFINEMENT.md migration/completed/
mv migration/PHASE_6_WEEK_2_COMPLETE.md migration/completed/
```

**Verification:**
```bash
ls migration/completed/
# Should show all 5 files
```

---

### Task 4: Archive Superseded Docs (15 mins)

**Phase 5 files:**
```bash
mv migration/PHASE_5_REVIEW.md migration/archive/phase5/
mv migration/phases/PHASE_05_WORKFLOW_INTEGRATION.md migration/archive/phase5/
```

**Phase 6 files:**
```bash
mv migration/PHASE_6_WEEK_1_VERIFICATION_REPORT.md migration/archive/phase6/
mv migration/PHASE_6_WEEK_2_PLAN.md migration/archive/phase6/
mv migration/phases/PHASE_06_ARROW_MIGRATION.md migration/archive/phase6/
```

**Planning docs (after extraction in Task 2):**
```bash
mv migration/FRAMEWORK_COMPLETION_PLAN.md migration/archive/planning/
mv migration/MIGRATION_STATUS.md migration/archive/planning/
```

**Verification:**
```bash
ls migration/archive/phase5/
ls migration/archive/phase6/
ls migration/archive/planning/
# Should show archived files
```

---

### Task 5: Create ARCHITECTURE.md (1 hour)

**File:** `docs/ARCHITECTURE.md` (create at repository root)

**Content structure:**
```markdown
# Generic Agent Simulation Framework - Architecture

## Design Philosophy

- Event-driven simulation
- Arrow RecordBatch for columnar storage
- DataFusion for analytics
- Snapshot-based persistence

## Core Patterns

### 1. Dual Index Pattern (RecordBatch + HashMap)
- RecordBatch for storage (columnar, efficient)
- HashMap for O(1) lookups by ID
- Trade-off: memory for speed

**Implementation locations:**
- PopulationHandler: crates/sim-core/src/population/handler.rs
- ResourceAllocator: crates/sim-core/src/resources/allocator.rs
- WorkflowEngine: crates/sim-core/src/workflows/engine.rs
- LocationHandler: crates/sim-geo/src/locations/handler.rs

### 2. TaskFactory Pattern
- Workflows define task templates
- Factory creates concrete tasks
- Resource allocation decoupled from workflow logic

**Implementation:** crates/sim-core/src/workflows/engine.rs

### 3. SnapshotManager Dual Backend
- Memory backend for testing (fast, ephemeral)
- Parquet backend for production (persistent, compressed)
- Same API for both

**Implementation:** crates/sim-core/src/snapshot/manager.rs

### 4. Demand-Driven Allocation
- Agents emit Demands, not Tasks directly
- SimulationHooks convert demands to tasks
- Decouples agent logic from resource management

**Implementation:** crates/sim-kitchen/src/simulation.rs

## Component Relationships

[Diagram or detailed description of how components interact]

## Performance Characteristics

- RecordBatch operations: O(n) for full batch rebuild
- HashMap lookups: O(1) by ID
- H3 spatial queries: O(k log n) where k = cells in radius
- Workflow updates: Currently O(n), target O(1) with batch API

## Design Decisions

### Why Arrow/DataFusion?
- Industry-standard columnar format
- Interoperability with analytics tools
- Efficient serialization (Parquet)
- Zero-copy data access

### Why HashMap Index?
- O(1) lookups by ID critical for agent updates
- RecordBatch alone requires O(n) scan
- Memory overhead acceptable for performance gain

### Why Dual Backends (Memory/Parquet)?
- Testing needs fast, ephemeral storage
- Production needs persistent, compressed storage
- Abstraction allows switching without code changes

## Future Considerations

- Incremental snapshots (save only deltas)
- SQL query interface (use SessionContext)
- Batch update APIs (fix O(n²) issue)
- Schema evolution strategy
```

**Actions:**
1. Create `docs/` directory if not exists
2. Extract design philosophy from FRAMEWORK_COMPLETION_PLAN.md
3. Add cross-references to implementation files
4. Document key patterns with code examples

---

### Task 6: Enhance README.md (1 hour)

**File:** `migration/README.md` (already updated in previous task)

**Additional enhancements:**
- ✅ Already added: Last Updated timestamp
- ✅ Already added: Quick Decision Tree
- ✅ Already added: Links to new docs (CURRENT_STATUS, CODE_REVIEW_SUMMARY, FORWARD_PLAN)
- ✅ Already updated: Status dashboard

**Remaining actions:**
- Add note about archive/ directory at top
- Update "How to Use These Docs" section to reference new structure
- Add link to docs/ARCHITECTURE.md

**Updates needed:**
```markdown
## Documentation Structure

```
migration/
├── README.md                          ← You are here (single source of truth)
├── CURRENT_STATUS.md                  Latest status & code review
├── CODE_REVIEW_SUMMARY.md             Detailed crate reviews
├── FORWARD_PLAN.md                    Implementation roadmap
├── PARALLEL_EXECUTION_GUIDE.md        Multi-agent coordination
├── completed/                         Historical completion records
├── phases/                            Future phase plans
└── archive/                           Old/superseded documents
```

**Also see:**
- Architecture & Design: `../docs/ARCHITECTURE.md`
- Codebase: `../crates/`
```
```

---

### Task 7: Update Phase Plans (15 mins)

**Update references in phase plans:**

**Files to update:**
- phases/PHASE_07_ROAD_NETWORK.md
- phases/PHASE_08_TELEMETRY.md
- phases/PHASE_09_MULTI_WORKFLOW.md
- phases/PHASE_10_11_DOMAINS.md
- phases/PHASE_12_13_14_INTEGRATION.md

**Changes:**
- Update "Dependencies" section if referencing Phase 5/6 plans
- Change references to completion docs to point to `../completed/`
- Verify all cross-references still work

---

### Task 8: Create Archive README (15 mins)

**File:** `migration/archive/README.md`

**Content:**
```markdown
# Archived Documentation

This directory contains superseded planning documents and historical records. These files are preserved for reference but are no longer the primary source of truth.

## Why These Files Are Archived

### phase5/
- **PHASE_5_REVIEW.md** - Pre-completion review, superseded by PHASE_5_COMPLETE.md
- **PHASE_05_WORKFLOW_INTEGRATION.md** - Original phase plan, phase now complete

### phase6/
- **PHASE_06_ARROW_MIGRATION.md** - Original phase plan, phase now complete
- **PHASE_6_WEEK_1_VERIFICATION_REPORT.md** - Test verification log, redundant with PHASE_6_WEEK_1_REFINEMENT.md
- **PHASE_6_WEEK_2_PLAN.md** - Planning document, superseded by PHASE_6_WEEK_2_COMPLETE.md

### planning/
- **FRAMEWORK_COMPLETION_PLAN.md** - Early roadmap, information consolidated into README.md and FORWARD_PLAN.md
- **MIGRATION_STATUS.md** - Status tracking, replaced by CURRENT_STATUS.md

## Current Documentation

See `../README.md` for current documentation structure and navigation.
```

---

## Success Criteria

### Quantitative
- ✅ Essential reading reduced from 8,557 to ~1,400 lines (84% reduction)
- ✅ Active docs reduced from 19 to 10 files
- ✅ Redundancy reduced from 40% to <5%
- ✅ 3 clear directories (completed/, phases/, archive/)

### Qualitative
- ✅ README.md is clear single source of truth
- ✅ New users can navigate to correct doc in <30 seconds
- ✅ Zero information loss (all content preserved in archive/)
- ✅ Cross-references work correctly
- ✅ Architecture decisions documented

---

## Validation Checklist

After completing all tasks:

- [ ] All completion docs in `completed/` directory
- [ ] All future phase plans in `phases/` directory
- [ ] All superseded docs in `archive/` directory
- [ ] README.md has decision tree and clear navigation
- [ ] docs/ARCHITECTURE.md created with design patterns
- [ ] archive/README.md explains why files are archived
- [ ] No broken links in active documentation
- [ ] Git diff shows only file moves (no content changes except extractions)

---

## Before/After Comparison

### Before (Current State)
```
migration/
├── 12 files at root level
├── phases/ (7 files)
└── No organization

Reading requirement: 8,557 lines
Redundancy: 40%
Navigation: Unclear which doc is authoritative
```

### After (Target State)
```
migration/
├── 5 files at root (README + 4 key docs)
├── completed/ (5 historical records)
├── phases/ (5 future plans)
└── archive/ (9 superseded docs)

Reading requirement: 1,400 lines (for onboarding)
Redundancy: <5%
Navigation: Clear decision tree, single source of truth
```

---

## Timeline

**Total Duration:** 3-4 hours

| Task | Duration | Dependencies |
|------|----------|--------------|
| 1. Create directories | 5 mins | None |
| 2. Extract unique content | 30 mins | None |
| 3. Move completion docs | 10 mins | Task 2 |
| 4. Archive superseded docs | 15 mins | Task 2 |
| 5. Create ARCHITECTURE.md | 1 hour | Task 2 |
| 6. Enhance README.md | 1 hour | Tasks 1-5 |
| 7. Update phase plans | 15 mins | Tasks 3-4 |
| 8. Create archive README | 15 mins | Task 4 |
| **Total** | **3-4 hours** | |

---

## Rollback Plan

If issues arise:

1. **Nothing is deleted** - All files moved to archive, can be moved back
2. **Git history preserved** - Use `git mv` for all operations
3. **Easy rollback** - Single git revert if needed

**Rollback command:**
```bash
git reset --hard HEAD  # If not committed
git revert <commit>    # If committed
```

---

## Next Steps After Completion

Once Phase 2 (Documentation Cleanup) is complete:

1. **Verify all links work** - Click through README.md navigation
2. **Test onboarding flow** - Have someone new read docs and provide feedback
3. **Update CURRENT_STATUS.md** - Mark Phase 2 as complete
4. **Proceed to Phase 3** - Code Simplification (if desired)
   OR
5. **Proceed to Phase 7/8** - New features (if Phase 1 bugs fixed)

---

## References

- **Current State:** See README.md (before cleanup)
- **Target State:** See structure diagram above
- **Design Decisions:** Will be in docs/ARCHITECTURE.md
- **Code Review Findings:** CURRENT_STATUS.md, CODE_REVIEW_SUMMARY.md
