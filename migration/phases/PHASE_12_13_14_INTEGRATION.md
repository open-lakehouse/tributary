# Phase 12-14: Integration, Polish & Production Readiness

**Status**: NOT STARTED
**Priority**: 🟢 PRODUCTION
**Estimated Effort**: 3 weeks
**Dependencies**: All previous phases complete
**Deliverable**: Production-ready framework

---

## Phase 12: Python Bindings (Week 1)

### Objectives
1. Create PyO3 bindings for framework
2. Wrap ghost-kitchen and logistics simulations
3. Expose event query API returning pandas DataFrames
4. Update analysis notebooks

### Implementation

**File**: `crates/python-bindings/src/ghost_kitchen.rs`

```rust
use pyo3::prelude::*;
use ghost_kitchen::GhostKitchenSimulation;

#[pyclass]
struct PyGhostKitchenSim {
    inner: GhostKitchenSimulation,
}

#[pymethods]
impl PyGhostKitchenSim {
    #[new]
    fn new(config_path: String) -> PyResult<Self> {
        let inner = GhostKitchenSimulation::from_config(&config_path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(Self { inner })
    }

    fn run_hours(&mut self, hours: f64) -> PyResult<()> {
        self.inner.run_for_hours(hours)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    fn query_events(&self, sql: String) -> PyResult<PyObject> {
        let batch = self.inner.query_events(&sql)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        // Convert RecordBatch to pandas DataFrame
        Python::with_gil(|py| {
            let pandas = py.import("pandas")?;
            let pyarrow = py.import("pyarrow")?;

            // RecordBatch → PyArrow Table → Pandas DataFrame
            let table = pyarrow.call_method1("Table.from_batches", ([batch],))?;
            let df = table.call_method0("to_pandas")?;

            Ok(df.to_object(py))
        })
    }
}

#[pymodule]
fn sim_python(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyGhostKitchenSim>()?;
    Ok(())
}
```

### Notebook Usage

```python
import sim_python

# Create simulation
sim = sim_python.PyGhostKitchenSim("config.json")

# Run for 24 hours
sim.run_hours(24.0)

# Query events as pandas DataFrame
orders_df = sim.query_events("""
    SELECT * FROM events
    WHERE type = 'caspers.order.created'
""")

print(f"Total orders: {len(orders_df)}")
print(orders_df.head())

# Analysis
import matplotlib.pyplot as plt
orders_df['hour'] = pd.to_datetime(orders_df['time']).dt.hour
orders_df.groupby('hour').size().plot(kind='bar')
plt.title("Orders by Hour of Day")
plt.show()
```

### Acceptance Criteria
- [ ] PyO3 bindings crate created
- [ ] GhostKitchen and Logistics simulations wrapped
- [ ] query_events() returns pandas DataFrames
- [ ] Notebooks updated and working
- [ ] Installation guide (pip install via maturin)

---

## Phase 13: CLI & Documentation (Week 2)

### CLI Updates

**File**: `crates/cli/src/main.rs`

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run ghost kitchen simulation
    GhostKitchen {
        #[arg(short, long, default_value = "24")]
        hours: f64,

        #[arg(short, long)]
        config: Option<String>,

        #[arg(short, long)]
        output: Option<String>,
    },

    /// Run logistics simulation
    Logistics {
        #[arg(short, long, default_value = "24")]
        hours: f64,

        #[arg(short, long)]
        config: Option<String>,
    },

    /// Save simulation snapshot
    Snapshot {
        #[arg(long)]
        save: Option<String>,

        #[arg(long)]
        load: Option<String>,
    },

    /// Query event logs
    Query {
        #[arg(short, long)]
        sql: String,

        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GhostKitchen { hours, config, output } => {
            let mut sim = if let Some(cfg) = config {
                GhostKitchenSimulation::from_config(&cfg)?
            } else {
                GhostKitchenSimulation::default()?
            };

            println!("Running ghost kitchen simulation for {} hours...", hours);
            sim.run_for_hours(hours)?;

            if let Some(out) = output {
                sim.save_snapshot(&out)?;
                println!("Snapshot saved to {}", out);
            }

            print_summary(&sim);
        },
        Commands::Logistics { hours, config } => {
            // Similar pattern
        },
        Commands::Snapshot { save, load } => {
            // Snapshot operations
        },
        Commands::Query { sql, output } => {
            // Query event logs
        },
    }

    Ok(())
}
```

### Documentation Structure

Create comprehensive docs in `docs/` directory:

```
docs/
├── README.md                    - Documentation index
├── architecture/
│   ├── overview.md              - Framework design
│   ├── traits.md                - Core trait reference
│   ├── data-model.md            - Arrow schemas
│   └── event-model.md           - CloudEvents structure
├── guides/
│   ├── getting-started.md       - Quick start tutorial
│   ├── creating-agents.md       - Implement Agent trait
│   ├── defining-workflows.md    - Implement Workflow trait
│   ├── event-schemas.md         - Custom EventPayload
│   ├── telemetry.md             - Data generation
│   ├── snapshots.md             - Save/restore state
│   └── performance.md           - Optimization tips
├── examples/
│   ├── ghost-kitchen.md         - Full walkthrough
│   ├── logistics.md             - Second example
│   └── custom-domain.md         - Template for new domains
├── api/
│   ├── sim-core.md              - API reference
│   ├── sim-geo.md
│   ├── sim-events.md
│   └── sim-telemetry.md
└── migration/
    └── from-universe.md         - Migration guide
```

### Key Documentation Files

**docs/guides/getting-started.md**:
- Installation instructions
- First simulation in 10 minutes
- Query events
- Visualize results

**docs/examples/custom-domain.md**:
- Template for creating new domain
- Step-by-step guide
- Copy-paste starting point

**docs/migration/from-universe.md**:
- Mapping old APIs to new framework
- Common patterns
- Migration checklist

### Acceptance Criteria
- [ ] CLI supports both domains
- [ ] CLI has snapshot save/load commands
- [ ] CLI has query command for event logs
- [ ] Documentation complete (10+ markdown files)
- [ ] Getting started guide tested by new user
- [ ] API reference generated from rustdoc

---

## Phase 14: Performance & Production Hardening (Week 3)

### Performance Benchmarks

**File**: `crates/sim-core/benches/population.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sim_core::*;

fn bench_agent_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("population");

    for size in [100, 1_000, 10_000, 100_000] {
        group.bench_with_input(
            BenchmarkId::new("tick_agents", size),
            &size,
            |b, &size| {
                let mut handler = create_population(size);
                let ctx = create_context();

                b.iter(|| {
                    handler.tick_agents(black_box(&ctx)).unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_agent_tick);
criterion_main!(benches);
```

**Similar benchmarks for**:
- Resource allocation (10k tasks)
- Workflow step progression (100k instances)
- Journey advancement (10k journeys)
- Event emission (1M events)
- Snapshot save/restore (10k agents)

### Performance Targets

| Operation | Target | Measurement |
|-----------|--------|-------------|
| 10k agent tick | <100ms | Criterion benchmark |
| 1k journey planning | <100ms | With road network |
| 10k task allocation | <20ms | Round-robin strategy |
| 100k workflow step | <100ms | Batch progression |
| 1M event emit | <500ms | Parquet write |
| 10k agent snapshot | <200ms | Save + restore |

### Production Hardening Checklist

**Error Handling**:
- [ ] Remove all `.unwrap()` calls in production paths
- [ ] Use `anyhow::Context` for error context
- [ ] Add error recovery strategies
- [ ] Log errors with structured logging (tracing crate)

**Code Quality**:
- [ ] Run `cargo clippy --all-features --all-targets`
- [ ] Fix all warnings
- [ ] Run `cargo fmt --all`
- [ ] Add `#![deny(unsafe_code)]` where possible

**Testing**:
- [ ] Achieve >85% code coverage
- [ ] Add property-based tests (proptest)
- [ ] Add stress tests (1M agents, 24-hour sim)
- [ ] Add concurrency tests (no data races)

**Monitoring**:
- [ ] Add structured logging with tracing
- [ ] Add progress indicators for long operations
- [ ] Add graceful shutdown on Ctrl+C
- [ ] Add simulation stats dashboard

**CI/CD**:
- [ ] GitHub Actions workflow for tests
- [ ] Benchmark regression detection
- [ ] Automated doc generation
- [ ] Release automation

### Memory Profiling

Use Valgrind/Heaptrack to validate:
- [ ] No memory leaks
- [ ] Linear memory scaling with population
- [ ] Memory usage acceptable (<10 GB for 1M agents)

### Acceptance Criteria

**Performance**:
- [ ] All benchmark targets met
- [ ] No performance regressions vs baseline
- [ ] Benchmarks run in CI
- [ ] Performance documented

**Code Quality**:
- [ ] Zero clippy warnings
- [ ] >85% test coverage
- [ ] No `.unwrap()` in production
- [ ] Comprehensive error handling

**Production**:
- [ ] Graceful shutdown
- [ ] Structured logging
- [ ] Progress indicators
- [ ] Memory profiled
- [ ] CI/CD pipeline

---

## Success Criteria (All Three Phases)

### Phase 12 (Python Bindings)
- [ ] PyO3 bindings working
- [ ] Notebooks updated
- [ ] pandas integration
- [ ] Installation documented

### Phase 13 (CLI & Docs)
- [ ] CLI supports all operations
- [ ] Comprehensive documentation (15+ docs)
- [ ] Getting started guide works
- [ ] Migration guide complete

### Phase 14 (Performance & Polish)
- [ ] All performance targets met
- [ ] Benchmark suite in CI
- [ ] >85% test coverage
- [ ] Zero clippy warnings
- [ ] Production-ready code quality

---

## Parallel Execution Opportunities

**Week 1** (Phase 12):
- Agent A: Python bindings
- Agent B: Begin documentation writing

**Week 2** (Phase 13):
- Agent A: CLI updates
- Agent B: Complete documentation
- Agent C: Begin benchmarks

**Week 3** (Phase 14):
- Agent A: Performance optimization
- Agent B: Code quality (clippy, unwraps)
- Agent C: CI/CD setup

---

**Estimated Effort**: 3 weeks
**Depends On**: All previous phases
**Deliverable**: Production-ready generic simulation framework
