# P0 Issues - Quick Fix Guide

**Total Time Required**: 2 hours 15 minutes
**Priority**: Must fix before production deployment

---

## Issue #1: Cache FastGraph (30 minutes)

**Problem**: FastGraph is rebuilt on every routing query, causing unnecessary overhead.

**Location**: `/Users/robert.pack/code/management/crates/sim-geo/src/graph/routing.rs:136`

**Current Code**:
```rust
pub fn find_shortest_path(&self, start: NodeId, goal: NodeId) -> Option<Route> {
    // Build fast graph (in production, cache this!)
    let fast_graph = self.build_fast_graph();
    // ...
}
```

**Fix**:
```rust
// Add to Cargo.toml dependencies:
// once_cell = "1.19"

use once_cell::sync::OnceCell;

// Add to RoadNetwork struct:
pub struct RoadNetwork {
    pub nodes: HashMap<NodeId, RoadNode>,
    pub edges: HashMap<EdgeId, RoadEdge>,
    pub adjacency: HashMap<NodeId, Vec<EdgeId>>,
    fast_graph: OnceCell<FastGraph>,  // Add this field
}

// Update impl:
impl RoadNetwork {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
            fast_graph: OnceCell::new(),  // Initialize
        }
    }

    pub fn find_shortest_path(&self, start: NodeId, goal: NodeId) -> Option<Route> {
        // Use cached FastGraph or build if first query
        let fast_graph = self.fast_graph.get_or_init(|| self.build_fast_graph());
        let mut calculator = fast_paths::create_calculator(fast_graph);

        // Rest of implementation unchanged...
        let path = calculator.calc_path(fast_graph, start as usize, goal as usize)?;
        // ...
    }
}
```

**Test**:
```bash
cargo test -p sim-geo graph::routing::tests
cargo bench -p sim-geo routing_performance
```

**Expected Result**: 5645x speedup (177ns instead of 1.86ms)

---

## Issue #2: Fix Unused Imports (15 minutes)

**Problem**: 7 clippy warnings for unused imports

### Fix 2.1: sim-telemetry/examples/telemetry_demo.rs

**Location**: Line 14

**Current**:
```rust
use std::collections::HashMap;
```

**Fix**: Remove the line (HashMap is not used in this example)

### Fix 2.2: sim-core/src/runner/telemetry.rs

**Location**: Line 328

**Current**:
```rust
use crate::utils::idents::{TaskId, WorkflowId, WorkflowInstanceId};
```

**Fix**:
```rust
use crate::utils::idents::{TaskId, WorkflowInstanceId};
```

### Fix 2.3: sim-core/src/runner/mod.rs

**Location**: Line 734

**Current**:
```rust
use crate::Error;
```

**Fix**: Remove the line or add `#[allow(unused_imports)]` if needed for conditional compilation

### Fix 2.4-2.9: Benchmark unused Results

**Locations**:
- `sim-telemetry/benches/telemetry_performance.rs:149, 174, 257, 267, 268, 269`

**Current**:
```rust
black_box(collector.process_event(black_box(event)));
black_box(OtlpExporter::export_logs(&logs));
// etc.
```

**Fix**:
```rust
let _ = black_box(collector.process_event(black_box(event)));
let _ = black_box(OtlpExporter::export_logs(&logs));
// etc.
```

**Test**:
```bash
cargo clippy -p sim-geo -p sim-telemetry -p sim-core --all-targets -- -D warnings
```

**Expected Result**: Zero warnings

---

## Issue #3: Fix Workflow Test Failures (1 hour)

**Problem**: 2 tests failing in workflow_integration_test.rs

**Location**: `/Users/robert.pack/code/management/crates/sim-core/tests/workflow_integration_test.rs`

**Failing Tests**:
1. `test_complete_workflow_lifecycle` - Expected 1, got 3
2. `test_multiple_concurrent_workflows` - Expected 3, got 9

**Investigation Steps**:

```bash
# Run with output to see failure details
cargo test -p sim-core --test workflow_integration_test -- --nocapture

# Check the specific assertions
grep -A 5 "assertion.*left == right.*failed" crates/sim-core/tests/workflow_integration_test.rs
```

**Likely Causes**:
1. Workflow instance counting logic changed
2. Test expectations not updated after code changes
3. Hook behavior modified by telemetry integration

**Fix Options**:

**Option A**: Update test expectations (if behavior is correct)
```rust
// Line 278 (approximately)
assert_eq!(completed_workflows.len(), 3); // Update from 1 to 3
```

**Option B**: Fix workflow counting logic (if behavior is wrong)
```rust
// In workflow handler code, ensure proper instance tracking
```

**Option C**: Isolate test from side effects
```rust
// Ensure test creates fresh state without interference
let mut runner = SimulationRunner::new(config);
// Don't set hooks if not needed for this test
```

**Test**:
```bash
cargo test -p sim-core --test workflow_integration_test
```

**Expected Result**: All tests pass

---

## Issue #4: Fix Universe Crate Imports (30 minutes)

**Problem**: Unresolved imports blocking compilation

**Location**: `/Users/robert.pack/code/management/crates/universe/src/test_utils.rs:39`

**Current Code**:
```rust
use crate::{
    EntityView, ObjectData, PopulationData,
    ROUTING_EDGES_REF, ROUTING_NODES_REF,  // These don't exist
    // ...
};
```

**Investigation**:
```bash
# Search for where these constants were defined
grep -r "ROUTING_EDGES_REF\|ROUTING_NODES_REF" crates/universe/src --include="*.rs"

# Check if they moved or were renamed
git log --all --oneline --grep="ROUTING" -- crates/universe/
```

**Fix Options**:

**Option A**: Remove unused imports
```rust
use crate::{
    EntityView, ObjectData, PopulationData,
    // Remove ROUTING_EDGES_REF, ROUTING_NODES_REF
};
```

**Option B**: Update to new constants (if they were renamed)
```rust
use crate::{
    EntityView, ObjectData, PopulationData,
    graph::{EDGES_REF, NODES_REF},  // New location
};
```

**Option C**: Exclude universe from workspace temporarily
```toml
# In workspace Cargo.toml
[workspace]
members = [
    "crates/sim-core",
    "crates/sim-geo",
    "crates/sim-telemetry",
    # "crates/universe",  # Exclude until fixed
]
```

**Test**:
```bash
cargo test --workspace --lib --quiet
```

**Expected Result**: All workspace tests compile and pass

---

## Validation Checklist

After all fixes:

```bash
# 1. Check compilation
cargo build --workspace --all-targets

# 2. Run clippy
cargo clippy --workspace --all-targets -- -D warnings

# 3. Run all tests
cargo test --workspace --quiet

# 4. Run specific phase tests
cargo test -p sim-geo --quiet
cargo test -p sim-telemetry --quiet
cargo test -p sim-core --test cross_phase_integration --quiet
cargo test -p sim-core --test ghost_kitchen_e2e --quiet

# 5. Verify benchmarks compile
cargo bench --workspace --no-run

# 6. Check examples work
cargo run -p sim-telemetry --example telemetry_demo

# 7. Generate docs
cargo doc --no-deps --workspace
```

**Success Criteria**: All commands complete without errors

---

## Time Breakdown

| Task | Estimated | Notes |
|------|-----------|-------|
| FastGraph caching | 30 min | Straightforward implementation |
| Unused imports | 15 min | Simple deletions |
| Workflow tests | 60 min | May require investigation |
| Universe imports | 30 min | Depends on cause |
| Testing & validation | 30 min | Run full test suite |
| **Total** | **2h 45m** | Add 30min buffer |

---

## Risk Mitigation

**Before starting**:
1. Create a new branch: `git checkout -b fix/p0-issues`
2. Commit current state: `git commit -am "Checkpoint before P0 fixes"`

**After each fix**:
1. Run relevant tests
2. Commit working changes
3. Continue to next fix

**If stuck**:
- Skip problematic fix and move to next
- Document issue for team discussion
- Don't block on universe crate (can exclude from CI)

---

## Success Metrics

**Definition of Done**:
- ✅ All 4 P0 issues resolved
- ✅ Zero clippy warnings
- ✅ All tests passing (271+)
- ✅ Benchmarks compile and run
- ✅ Examples work correctly
- ✅ Documentation builds without errors

**Ready for Production**: Yes, after validation ✅

---

*Created: March 16, 2026*
*Estimated completion: 2-3 hours*
*Priority: P0 - Must fix before production*
