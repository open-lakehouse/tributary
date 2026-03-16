# Performance Profiling Guide

**Last Updated:** March 16, 2026

This guide covers profiling strategies, tools, and optimization workflows for the Generic Agent Simulation Framework. Performance profiling was critical throughout the migration phases, delivering 7-785x speedups for batch operations and maintaining sub-millisecond tick times at scale.

---

## Table of Contents

1. [Profiling Strategy Overview](#profiling-strategy-overview)
2. [Criterion.rs Benchmarking](#criterionrs-benchmarking)
3. [CPU Profiling with perf](#cpu-profiling-with-perf)
4. [Memory Profiling](#memory-profiling)
5. [Flamegraph Generation](#flamegraph-generation)
6. [Common Performance Pitfalls](#common-performance-pitfalls)
7. [Optimization Workflow](#optimization-workflow)
8. [Case Studies from Migration Phases](#case-studies)

---

## Profiling Strategy Overview

### When to Profile

**Profile during development when:**
- Implementing new core functionality (workflows, allocation, movement)
- Adding batch operations or bulk data processing
- Integrating external libraries (Arrow, DataFusion, fast_paths)
- Approaching performance targets (100ms/tick, 10k agents)
- Investigating reported slowdowns or regressions

**Profile after deployment when:**
- Real-world workloads differ from test scenarios
- Scaling to larger agent populations
- Adding new domains or workflow types
- Users report responsiveness issues

### Performance Targets

The framework maintains these performance targets:

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Tick time (1,500 agents) | <100ms | 0.06ms | ✅ 1,667x better |
| Batch updates (1,000 instances) | <10ms | 0.5ms | ✅ 20x better |
| Routing query (cached) | <1ms | 177ns | ✅ 5,645x better |
| Snapshot save (1,500 agents) | <1s | 15ms | ✅ 66x better |
| Telemetry overhead | <5% | <1% | ✅ 5x better |

### Profiling Tools Matrix

| Tool | Purpose | Platform | Use Case |
|------|---------|----------|----------|
| Criterion.rs | Microbenchmarks | All | Regression detection, API comparison |
| perf | CPU profiling | Linux | Hotspot identification |
| Instruments | CPU/memory | macOS | XCode integration |
| Valgrind | Memory errors | Linux | Leak detection |
| Massif | Heap profiling | Linux | Memory growth analysis |
| cargo-flamegraph | Visualization | All | Call stack analysis |
| hyperfine | Command timing | All | End-to-end benchmarks |

---

## Criterion.rs Benchmarking

### Overview

Criterion.rs is the primary benchmarking tool for the framework. All performance-critical code paths have Criterion benchmarks for regression detection.

**Benchmark locations:**
- `/Users/robert.pack/code/management/crates/sim-core/benches/workflow_performance.rs` (422 lines, 8 groups)
- `/Users/robert.pack/code/management/crates/sim-core/benches/batch_operations.rs`
- `/Users/robert.pack/code/management/crates/sim-geo/benches/routing_performance.rs`
- `/Users/robert.pack/code/management/crates/sim-telemetry/benches/telemetry_performance.rs`

### Running Benchmarks

**Run all benchmarks:**
```bash
cargo bench
```

**Run specific benchmark file:**
```bash
cargo bench --bench workflow_performance
```

**Run specific benchmark group:**
```bash
cargo bench --bench workflow_performance -- sequential_vs_batch
```

**Save baseline for comparison:**
```bash
cargo bench --bench workflow_performance -- --save-baseline phase3
```

**Compare against baseline:**
```bash
cargo bench --bench workflow_performance -- --baseline phase3
```

### Benchmark Structure Example

From `crates/sim-core/benches/workflow_performance.rs`:

```rust
use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

fn bench_sequential_vs_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequential_vs_batch");

    let test_cases = vec![
        (1_000, 10),     // 10 updates on 1,000 instances
        (1_000, 100),    // 100 updates on 1,000 instances
        (1_000, 500),    // 500 updates on 1,000 instances
        (10_000, 100),   // 100 updates on 10,000 instances
        (10_000, 1_000), // 1,000 updates on 10,000 instances
    ];

    for (total_instances, update_count) in test_cases {
        let label = format!("{}_of_{}", update_count, total_instances);

        // Sequential update benchmark
        group.bench_with_input(
            BenchmarkId::new("sequential", &label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || populate_table(total_instances),
                    |(mut table, instance_ids)| {
                        for i in 0..update_count {
                            let id = instance_ids[i];
                            let row_idx = table.get_row_index(id).unwrap();
                            table.update_instance_at_row(row_idx, |u| {
                                u.current_step_index = Some(1);
                            }).unwrap();
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );

        // Batch update benchmark
        group.bench_with_input(
            BenchmarkId::new("batch", &label),
            &(total_instances, update_count),
            |b, &(total_instances, update_count)| {
                b.iter_batched(
                    || populate_table(total_instances),
                    |(mut table, instance_ids)| {
                        let mut batch = BatchUpdateList::new();
                        for i in 0..update_count {
                            batch.push(WorkflowInstanceRowUpdate {
                                instance_id: instance_ids[i],
                                current_step_index: Some(1),
                                status: None,
                            });
                        }
                        table.batch_update_instances(&batch).unwrap();
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_sequential_vs_batch);
criterion_main!(benches);
```

### Benchmark Results Interpretation

**Phase 3 Workflow Performance Results:**

| Scenario | Batch Size | Table Size | Sequential | Batch | Speedup |
|----------|-----------|------------|------------|-------|---------|
| Small batch | 10 | 1,000 | 75.2µs | 10.0µs | **7.5x** |
| Medium batch | 100 | 1,000 | 753µs | 14.0µs | **53.8x** |
| Large batch | 500 | 1,000 | 3.77ms | 18.0µs | **209x** |
| Medium (large table) | 100 | 10,000 | 897µs | 14.0µs | **64.2x** |
| Large (large table) | 1,000 | 10,000 | 14.1ms | 18.0µs | **785x** |

**Key Finding:** Batch operations scale exceptionally well. The larger the batch and table size, the greater the performance improvement.

### Creating New Benchmarks

**Template for new benchmarks:**

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_my_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_operation");

    for size in [100, 1_000, 10_000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                // Setup
                let data = create_test_data(size);

                // Benchmark the operation
                b.iter(|| {
                    perform_operation(&data)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_my_operation);
criterion_main!(benches);
```

**Add to `Cargo.toml`:**
```toml
[[bench]]
name = "my_operation_performance"
harness = false
```

---

## CPU Profiling with perf

### Setup (Linux)

**Install perf:**
```bash
sudo apt-get install linux-tools-generic
```

**Allow non-root profiling:**
```bash
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

### Recording CPU Profiles

**Profile entire application:**
```bash
perf record --call-graph dwarf ./target/release/my-simulation
```

**Profile specific benchmark:**
```bash
cargo bench --bench workflow_performance --no-run
perf record --call-graph dwarf ./target/release/deps/workflow_performance-*
```

**Record for specific duration:**
```bash
perf record -F 99 --call-graph dwarf -g -- ./target/release/my-simulation
```

### Analyzing perf Data

**Generate text report:**
```bash
perf report
```

**Top functions by CPU time:**
```bash
perf report --stdio | head -50
```

**Annotate source code:**
```bash
perf annotate -s function_name
```

**Export for flamegraph:**
```bash
perf script > perf.data.script
```

### Interpreting perf Output

**Example output:**
```
# Overhead  Command    Shared Object          Symbol
# ........  .........  .....................  .............................
    45.23%  simulation sim-core.so            [.] WorkflowInstanceTable::batch_update
    23.45%  simulation sim-geo.so             [.] RoadNetwork::find_shortest_path
    12.34%  simulation arrow.so               [.] RecordBatch::rebuild
     8.91%  simulation sim-core.so            [.] ResourceAllocator::allocate
     5.67%  simulation libc.so.6              [.] malloc
     2.34%  simulation sim-geo.so             [.] LocationHandler::update_batch
```

**What to look for:**
- Functions consuming >10% of CPU time (optimization candidates)
- Unexpected allocations (malloc, free in hot paths)
- Arrow operations that could be batched
- Repetitive hash lookups that could be cached

---

## Memory Profiling

### Valgrind for Memory Errors

**Check for leaks:**
```bash
valgrind --leak-check=full --show-leak-kinds=all \
  ./target/debug/my-simulation
```

**Detailed leak tracking:**
```bash
valgrind --leak-check=full --track-origins=yes \
  --log-file=valgrind.log ./target/debug/my-simulation
```

**Common issues detected:**
- Memory leaks (unfreed allocations)
- Use-after-free errors
- Double-free errors
- Invalid memory access

### Massif for Heap Profiling

**Profile heap usage:**
```bash
valgrind --tool=massif ./target/debug/my-simulation
```

**Visualize heap growth:**
```bash
ms_print massif.out.12345
```

**Example output:**
```
    KB
19.65^                                               :#
     |                                               :#
     |                                               :#
     |                                               :#
     |                                               :#
     |                                               :#::
     |                                         ::::::# ::
     |                                      :::@     # ::
     |                                  ::::@  @     # ::
     |                              ::::@  @  @     # ::
     |                          ::::@  @  @  @     # ::
     |                      :::::@  @  @  @  @     # ::
     |                  :::::@ @  @  @  @  @     # ::
     |              :::::@  @ @  @  @  @  @     # ::
   0 +----------------------------------------------------------------------->Mi
     0                                                                   100.0

Legend:
@ = WorkflowInstanceTable allocations
: = Arrow RecordBatch allocations
# = HashMap index allocations
```

### Memory Overhead Analysis

**Phase 6 Arrow Migration Impact:**

| Component | Overhead | Justification |
|-----------|----------|---------------|
| RecordBatch storage | +20% | Offset by columnar compression |
| HashMap indices | +15% | Required for O(1) lookups |
| SessionContext (query) | ~10MB | Cached, amortized across queries |
| **Net Overhead** | **<5%** | **Acceptable for performance gains** |

---

## Flamegraph Generation

### Using cargo-flamegraph

**Install:**
```bash
cargo install flamegraph
```

**Generate flamegraph:**
```bash
cargo flamegraph --bench workflow_performance
```

**Profile specific test:**
```bash
cargo flamegraph --test integration_test -- test_24_hour_simulation
```

**Output:** Opens `flamegraph.svg` in browser showing call hierarchy

### Interpreting Flamegraphs

**Reading the graph:**
- **Width:** Time spent in function (including children)
- **Height:** Call stack depth
- **Color:** Function grouping (by crate)

**What to look for:**
- Wide plateaus (functions consuming significant time)
- Tall spikes (deep call stacks, possible recursion issues)
- Unexpected library calls in hot paths
- Allocations in inner loops

**Example findings from Phase 7:**

```
RoadNetwork::find_shortest_path (1.86ms)
└─ FastGraph::find_path (1.85ms)
   └─ ContractedGraph::new (1.84ms)  ← HOTSPOT! Built on every query
      └─ prepare_contraction (1.83ms)
         └─ calculate_shortcuts (1.82ms)
```

**Solution:** Cache FastGraph with OnceLock → 5,645x speedup

---

## Common Performance Pitfalls

### 1. Repeated RecordBatch Rebuilds

**Problem:**
```rust
// BAD: Rebuilds RecordBatch for each update (O(N²))
for update in updates {
    let mut new_columns = Vec::new();
    // ... rebuild entire RecordBatch
    self.instances = RecordBatch::try_new(schema, new_columns)?;
}
```

**Solution:**
```rust
// GOOD: Batch all updates, rebuild once (O(N))
let batch_updates = collect_all_updates();
self.instances = apply_batch_updates(&self.instances, &batch_updates)?;
```

**Performance Impact:** 7-785x speedup (Phase 3)

### 2. Missing Index Caches

**Problem:**
```rust
// BAD: Linear search for every lookup
fn get_by_id(&self, id: AgentId) -> Option<&Agent> {
    self.agents.iter().find(|a| a.id == id)  // O(N)
}
```

**Solution:**
```rust
// GOOD: HashMap index for O(1) lookup
pub struct PopulationHandler {
    agents: RecordBatch,
    id_to_row: HashMap<AgentId, usize>,  // O(1) index
}
```

**Performance Impact:** Critical for large populations (>1,000 agents)

### 3. Uncached Expensive Preprocessing

**Problem:**
```rust
// BAD: Rebuild Contraction Hierarchies on every routing query
pub fn find_path(&self, start: NodeId, end: NodeId) -> Route {
    let fast_graph = FastGraph::new(&self.edges);  // 1.86ms!
    fast_graph.find_path(start, end)
}
```

**Solution:**
```rust
// GOOD: Cache preprocessed graph with OnceLock
pub struct RoadNetwork {
    fast_graph: OnceLock<FastGraph>,
}

pub fn find_path(&self, start: NodeId, end: NodeId) -> Route {
    let fast_graph = self.fast_graph.get_or_init(|| {
        FastGraph::new(&self.edges)  // Only runs once
    });
    fast_graph.find_path(start, end)  // 330ns
}
```

**Performance Impact:** 5,645x speedup (Phase 7)

### 4. Unnecessary Allocations in Hot Paths

**Problem:**
```rust
// BAD: Allocates new Vec on every tick
pub fn tick(&mut self) {
    let events = Vec::new();  // Allocation in hot path
    for agent in &self.agents {
        events.push(agent.update());
    }
}
```

**Solution:**
```rust
// GOOD: Reuse buffer, clear between ticks
pub struct SimulationRunner {
    event_buffer: Vec<Event>,  // Reused buffer
}

pub fn tick(&mut self) {
    self.event_buffer.clear();  // No allocation
    for agent in &self.agents {
        self.event_buffer.push(agent.update());
    }
}
```

**Performance Impact:** Reduces allocations by 90%+ in hot paths

### 5. Unbatched Database Operations

**Problem:**
```rust
// BAD: N separate Arrow operations
for id in ids {
    let row = self.get_row(id)?;
    self.update_row(row, new_value)?;  // Rebuild RecordBatch N times
}
```

**Solution:**
```rust
// GOOD: Single batched operation
let updates = ids.iter().map(|id| {
    (id, new_value)
}).collect();
self.batch_update(updates)?;  // Rebuild RecordBatch once
```

**Performance Impact:** 10-100x speedup for bulk operations

### 6. Synchronous I/O in Tick Loop

**Problem:**
```rust
// BAD: Blocking I/O in tight loop
pub fn tick(&mut self) {
    for event in &events {
        std::fs::write("event.log", format!("{:?}", event))?;  // BLOCKS!
    }
}
```

**Solution:**
```rust
// GOOD: Buffered async I/O
pub fn tick(&mut self) {
    self.event_buffer.extend(events);  // Buffer in memory

    if self.event_buffer.len() > 1000 {
        tokio::spawn(async move {
            write_events_async(event_buffer).await  // Background
        });
        self.event_buffer.clear();
    }
}
```

**Performance Impact:** Maintains consistent tick times

---

## Optimization Workflow

### Step 1: Measure Baseline

**Before any optimization:**
```bash
# Save current performance
cargo bench -- --save-baseline before_optimization

# Run profiler to identify hotspots
cargo flamegraph --bench my_benchmark
```

**Establish targets:**
- What is acceptable performance?
- What is the theoretical limit?
- What are competitors achieving?

### Step 2: Identify Hotspots

**Use profiling tools:**
```bash
# CPU hotspots
perf record --call-graph dwarf ./target/release/simulation
perf report

# Memory growth
valgrind --tool=massif ./target/release/simulation
ms_print massif.out.*

# Visual analysis
cargo flamegraph --bench workflow_performance
```

**Look for:**
- Functions consuming >10% of CPU time
- Allocations in hot paths
- Repeated expensive operations
- Deep call stacks with shallow work

### Step 3: Implement Optimization

**Focus on high-impact changes:**
1. **Algorithmic improvements** (O(N²) → O(N))
2. **Batching operations** (N ops → 1 op)
3. **Caching expensive computations**
4. **Reducing allocations**
5. **Micro-optimizations** (last resort)

**Example from Phase 3:**
```rust
// Before: O(N²) - rebuild RecordBatch N times
pub fn update_status(&mut self, id: WorkflowInstanceId, status: InstanceStatus) {
    let row_idx = self.index.get(&id)?;
    // ... rebuild entire RecordBatch with one field changed
}

// After: O(N) - batch N updates, rebuild once
pub fn batch_update_status(&mut self, updates: &[(WorkflowInstanceId, InstanceStatus)]) {
    // ... collect all updates
    // ... rebuild RecordBatch once with all changes
}
```

### Step 4: Measure Impact

**Compare against baseline:**
```bash
cargo bench -- --baseline before_optimization
```

**Example output:**
```
sequential_vs_batch/batch/100_of_1000
    time:   [13.8 µs 14.0 µs 14.2 µs]
    change: [-98.1% -98.0% -97.9%] (p = 0.00 < 0.05)
    Performance has improved by 98% (50x speedup).
```

**Validate no regressions:**
```bash
# Run full test suite
cargo test --release

# Run integration tests
cargo test --test integration_test --release

# Check memory usage
valgrind --tool=massif ./target/release/simulation
```

### Step 5: Document and Iterate

**Document the optimization:**
- What was the bottleneck?
- What was the solution?
- What was the performance impact?
- Were there trade-offs (memory, complexity)?

**Iterate if needed:**
- If target not met, return to Step 2
- If trade-offs unacceptable, try alternative approach
- If good enough, move to next bottleneck

---

## Case Studies from Migration Phases

### Case Study 1: Phase 3 - Batch Operations (7-785x Speedup)

**Problem:**
Updating workflow instance status required rebuilding the entire Arrow RecordBatch for each update. With 1,000 instances, updating 500 of them required 500 RecordBatch rebuilds.

**Profiling Results:**
```
perf report:
  45.2% - RecordBatch::try_new (called 500 times)
  23.1% - Column::new (2,500 times - 5 columns × 500)
  12.3% - validate_schema (500 times)
```

**Solution:**
Implemented batch update API that collects all updates and rebuilds RecordBatch once:

```rust
pub struct BatchUpdateList {
    updates: Vec<WorkflowInstanceRowUpdate>,
}

pub fn batch_update_instances(&mut self, updates: &BatchUpdateList) -> Result<()> {
    // Build update map
    let update_map: HashMap<WorkflowInstanceId, &WorkflowInstanceRowUpdate> =
        updates.iter().map(|u| (u.instance_id, u)).collect();

    // Rebuild columns with all updates applied
    // ... (rebuild RecordBatch ONCE)
}
```

**Performance Impact:**
- Small batch (10 updates): 7.5x speedup
- Medium batch (100 updates): 53.8x speedup
- Large batch (500 updates): 209x speedup
- Large batch on large table (1,000 updates): **785x speedup**

**Lesson:** Batch operations are critical for Arrow-based storage. Always prefer batch APIs over iterative updates.

**Reference:** `/Users/robert.pack/code/management/migration/completed/PHASE_3_COMPLETE.md`

---

### Case Study 2: Phase 6 - Arrow Migration (10-100x Speedup)

**Problem:**
HashMap-based storage required iteration over all elements for filtering, aggregation, and bulk updates.

**Profiling Results:**
```
Bulk agent status update (1,000 agents):
  HashMap approach: 15ms
  - hash lookups: 8ms
  - individual updates: 7ms
```

**Solution:**
Migrated to Arrow RecordBatch with dual-index architecture:

```rust
pub struct PopulationHandler {
    agents: RecordBatch,              // Columnar storage
    id_to_row: HashMap<AgentId, usize>,  // O(1) index
}
```

**Performance Impact:**
- Bulk agent status update: 30x faster (15ms → 0.5ms)
- Batch task allocation: 60x faster (12ms → 0.2ms)
- SQL aggregations: 3.75x faster (3ms → 0.8ms)
- Snapshot save: 66x faster (<1s target → 15ms)
- Parquet compression: 9x reduction (1.8 MB → 200 KB)

**Lesson:** Columnar storage excels at bulk operations and analytics. O(1) index maintains single-row lookup performance.

**Reference:** `/Users/robert.pack/code/management/migration/completed/PHASE_6_COMPLETE.md`

---

### Case Study 3: Phase 7 - FastGraph Caching (5,645x Speedup)

**Problem:**
Road network routing rebuilt Contraction Hierarchies on every query, spending 1.86ms preprocessing for a 330ns query.

**Profiling with flamegraph:**
```
RoadNetwork::find_shortest_path (1.86ms total)
  └─ FastGraph::new (1.85ms - 99.5% of time!)
     └─ ContractedGraph::new
        └─ prepare_contraction
           └─ calculate_shortcuts
```

**The smoking gun:** 99.5% of time spent in initialization, not actual pathfinding.

**Solution:**
Cache FastGraph with OnceLock, build once on first query:

```rust
pub struct RoadNetwork {
    edges: Vec<Edge>,
    fast_graph: OnceLock<FastGraph>,  // ← Cache here
}

pub fn find_shortest_path(&self, start: NodeId, end: NodeId) -> Option<Route> {
    let fast_graph = self.fast_graph.get_or_init(|| {
        FastGraph::new(&self.edges)  // Only runs once
    });

    fast_graph.find_path(start, end)  // 330ns
}
```

**Performance Impact:**
- Single query (uncached): 1.86ms (537 queries/second)
- Single query (cached): 330ns (3.03M queries/second)
- **Speedup: 5,645x**

**Lesson:** Identify preprocessing vs. actual work. Cache expensive preprocessing, especially for read-heavy operations.

**Reference:** `/Users/robert.pack/code/management/PHASE_7_8_FINAL_REVIEW.md`

---

### Case Study 4: Phase 9 - Multi-Workflow (1,667x Better Than Target)

**Problem:**
Running 3 concurrent workflow types (food orders, supply chain, courier delivery) with inventory management.

**Target:** <100ms per tick (based on human-perceivable responsiveness)

**Profiling Results:**
24-hour simulation (1,440 ticks):
- Average tick time: 0.06ms
- Peak tick time: 0.13ms
- Total simulation time: ~86ms

**Performance Breakdown:**
```
Tick operation breakdown:
  Agent ticking:          <0.01ms  (efficient state machines)
  Workflow updates:       <0.02ms  (O(1) HashMap lookups)
  Resource allocation:    <0.01ms  (minimal overhead)
  Inventory operations:   <0.01ms  (Arc<Mutex> efficient)
  Event emission:         <0.01ms  (direct dispatcher)
  Total:                  0.06ms
```

**Achievement:** **1,667x better than 100ms target**

**Why so fast:**
1. **Batch operations:** All prior optimizations compound
2. **O(1) lookups:** HashMap indices eliminate linear scans
3. **Arrow columnar:** Efficient data access patterns
4. **No allocations:** Reused buffers in hot paths
5. **Thread-safe primitives:** Arc<Mutex> low overhead for this workload

**Lesson:** Incremental optimizations across phases compound. Focus on fundamentals (algorithms, data structures) before micro-optimizations.

**Reference:** `/Users/robert.pack/code/management/migration/completed/PHASE_9_COMPLETE.md`

---

### Case Study 5: Phase 10 - Demand Generation (85% Better Than Target)

**Problem:**
Adding probabilistic demand generation (Poisson arrivals, time-of-day patterns, spatial clustering) without regressing performance.

**Target:** <5ms per-tick overhead for demand generation

**Profiling Results:**
```
Operation                Time      Target    Status
Poisson generation       0.15ms    <1ms      ✅ 85% better
TimeOfDay check          0.03ms    <0.1ms    ✅ 70% better
Spatial lookup           0.12ms    <0.5ms    ✅ 76% better
Composite evaluation     0.45ms    <2ms      ✅ 78% better
Per-tick overhead        0.75ms    <5ms      ✅ 85% better
```

**Integration Impact:**
- Base simulation: 0.06ms/tick (from Phase 9)
- With demand generation: 0.81ms/tick
- **Still 123x better than 100ms target**

**Optimization techniques:**
1. **Cached H3 cell lookups** for spatial generator
2. **Pre-computed hourly multipliers** for time-of-day
3. **RNG with fixed seed** for reproducibility
4. **Batch demand creation** (generate multiple per tick)

**Lesson:** Performance "budget" from prior optimizations allows adding features without regression. Validate that new features stay within budget.

**Reference:** `/Users/robert.pack/code/management/migration/completed/PHASE_10_COMPLETE.md`

---

## Performance Checklist

Use this checklist when implementing new features or investigating slowdowns:

### Algorithmic Complexity
- [ ] Is the algorithm O(N) or better?
- [ ] Are there nested loops that could be batched?
- [ ] Could a HashMap/HashSet eliminate linear scans?
- [ ] Is preprocessing separated from execution?

### Data Structures
- [ ] Is Arrow RecordBatch used for bulk data?
- [ ] Is there a HashMap index for O(1) lookups?
- [ ] Are large structures using Arc to avoid clones?
- [ ] Are hot-path structures cache-friendly?

### Allocations
- [ ] Are vectors pre-allocated with capacity?
- [ ] Are buffers reused between iterations?
- [ ] Are Strings avoided in hot paths?
- [ ] Is cloning minimized (use references)?

### Caching
- [ ] Are expensive computations cached?
- [ ] Is preprocessing done once (not per-query)?
- [ ] Are lookup tables pre-computed?
- [ ] Is OnceLock used for lazy initialization?

### Batching
- [ ] Are operations batched when possible?
- [ ] Is I/O buffered (not per-item)?
- [ ] Are RecordBatch updates batched?
- [ ] Are events buffered before dispatch?

### Benchmarking
- [ ] Does a Criterion benchmark exist?
- [ ] Is performance compared to baseline?
- [ ] Are edge cases benchmarked?
- [ ] Is regression detection enabled in CI?

---

## Additional Resources

### Documentation
- **Architecture:** `/Users/robert.pack/code/management/docs/ARCHITECTURE.md`
- **Performance Tuning Guide:** `/Users/robert.pack/code/management/docs/guides/performance-tuning.md`
- **Phase Completion Reports:** `/Users/robert.pack/code/management/migration/completed/`

### External Tools
- **Criterion.rs:** https://bheisler.github.io/criterion.rs/book/
- **perf tutorial:** https://perf.wiki.kernel.org/index.php/Tutorial
- **Flamegraph:** https://github.com/flamegraph-rs/flamegraph
- **Valgrind:** https://valgrind.org/docs/manual/manual.html

### Rust Performance Book
- **The Rust Performance Book:** https://nnethercote.github.io/perf-book/

---

**Last Updated:** March 16, 2026
**Maintained By:** Framework Team
**Questions?** See main README or architecture docs
