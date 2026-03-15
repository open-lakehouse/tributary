# AI Agent Instructions for Chef Casper's Management System

> **Purpose**: This file provides AI coding assistants with essential context and
> guidelines for working with this simulation codebase. Last updated: 2026-03-15

## Quick Reference

| Task | Command | Notes |
|------|---------|-------|
| Code generation | `just generate` | After any `.proto` changes |
| Run simulation | `just run [duration]` | Default duration: 100 steps |
| Init simulation | `just init` | Creates `.caspers/` working directory |
| Run with UI | `just server` | Starts TUI application |
| Python dev | `just build-py` | Build Python bindings |
| Notebooks | `just scratch` | Launch Marimo notebook |

## Project Identity

**What this is**: A high-performance agent-based simulation system for generating realistic
operational data for a fictional ghost kitchen management company.

**What this is NOT**: A production restaurant management system (see legacy Python/SQLite
implementation in README.md for that context)

### Core Technologies Stack
- **Rust** (Primary): Simulation engine, CLI, TUI
- **Apache Arrow/DataFusion**: Columnar data processing
- **Protocol Buffers**: Schema definitions and type generation
- **GeoArrow**: Geospatial operations (H3 indexing)
- **Python**: Analysis tools and bindings (via PyO3/Maturin)

## Architecture Overview

### Project Structure
```
management/
├── crates/
│   ├── universe/              # Core simulation engine
│   │   ├── src/agents/       # Agent behavior implementations
│   │   ├── src/state/        # State management (Arrow tables)
│   │   ├── src/simulation/   # Event-driven simulation loop
│   │   ├── src/models/gen/   # ⚠️ GENERATED - DO NOT EDIT
│   │   ├── src/builders/     # Data builders and helpers
│   │   └── src/functions/    # Custom UDFs for DataFusion
│   └── cli/                  # Binary application (TUI + server)
├── proto/                    # Protocol buffer schemas
├── python/                   # Python bindings (PyO3)
├── notebooks/                # Marimo/Jupyter analysis notebooks
└── .caspers/                 # Runtime working directory (gitignored)
```

### Data Flow
1. **Initialization**: Load JSON configs → Build Arrow tables → Initialize agents
2. **Simulation Loop**:
   - Agent decisions (parallel where possible)
   - State updates (Arrow batch operations)
   - Event recording (Parquet append)
   - Snapshot persistence (configurable intervals)
3. **Output**: Parquet files in `.caspers/` for analysis

### Event-Driven Architecture
The simulation uses an event-sourcing pattern:
- Agents emit events (orders, movements, status changes)
- Events are batched into Arrow RecordBatches
- State is derived from event log
- See `crates/universe/src/simulation/events.rs` for event types

## Critical Rules

### 🚨 NEVER DO THIS
1. **Edit generated code** in `crates/universe/src/models/gen/` - Changes will be overwritten
2. **Skip `just generate`** after modifying `.proto` files - Will cause compilation errors
3. **Use non-workspace dependencies** - Must be defined in root `Cargo.toml` `[workspace.dependencies]`
4. **Commit `.caspers/` directory** - Runtime data only, gitignored
5. **Mix time zones** - All timestamps are UTC, be explicit

### ✅ ALWAYS DO THIS
1. **Run `just generate`** after proto changes
2. **Use DataFusion UDFs** for complex operations on Arrow data
3. **Consider memory usage** when working with large simulations
4. **Add tests** in `*_tests.rs` files alongside implementation
5. **Check `justfile`** for correct command usage

## Domain Model

### Business Entities
- **Location/Site**: Physical building with multiple kitchens
- **Vendor**: Company that operates brands and rents kitchen space
- **Kitchen**: Cooking facility within a location
- **Brand**: Restaurant brand (can operate across multiple kitchens)
- **Menu/Category/Item**: Menu hierarchy
- **Population**: Simulated customers in geographic cells (H3)

### Agent Types
- **Kitchen Agent**: Processes orders, manages cooking workflow
- **Population Agent**: Generates customer orders based on demand patterns
- **Movement Agent**: Simulates delivery logistics (future)

See `crates/universe/src/agents/` for implementations.

### Geospatial Model
- Uses **H3 hexagonal indexing** for location representation
- Functions in `crates/universe/src/functions/` handle geo operations
- GeoArrow arrays for efficient spatial queries

## Development Workflows

### Adding a New Agent
1. Create `crates/universe/src/agents/my_agent.rs`
2. Implement the agent behavior (see `kitchen.rs` as example)
3. Register in `crates/universe/src/agents/mod.rs`
4. Add corresponding tests in `my_agent_tests.rs`
5. Update simulation loop in `crates/universe/src/simulation/mod.rs`

### Modifying Data Schema
1. Edit `.proto` files in `proto/`
2. Run `just generate` (regenerates Rust code)
3. Update builders in `crates/universe/src/builders/`
4. Update state management in `crates/universe/src/state/`
5. Test with `cargo test`

### Working with Arrow Data
```rust
// Preferred: Use DataFusion for complex operations
let ctx = SessionContext::new();
ctx.register_table("orders", orders_table)?;
let df = ctx.sql("SELECT * FROM orders WHERE status = 'pending'").await?;

// For simple operations: Use Arrow compute kernels
use arrow::compute::filter;
let filtered = filter(&array, &predicate_mask)?;
```

### Python Integration
- Build bindings: `just build-py`
- Python code in `python/` exposes Rust simulation to Python
- Use for analysis, NOT for core simulation logic
- Marimo notebooks in `notebooks/` for interactive exploration

## Common Tasks

### Running Different Simulation Modes
```bash
# Standard run
just run 500

# With tracing (requires Docker)
just run-tracing
cargo run --bin caspers -- run --duration 500

# Server mode with TUI
just server

# Initialize fresh working directory
just init
```

### Debugging
- Use `tracing` crate macros: `debug!()`, `info!()`, `warn!()`
- Enable detailed logs: `RUST_LOG=debug cargo run`
- Jaeger tracing: `just run-tracing` then visit http://localhost:16686

### Testing
```bash
cargo test                          # All tests
cargo test --package universe       # Package-specific
cargo test kitchen                  # Pattern match
```

## Performance Guidelines

### DO
- Use vectorized Arrow operations
- Batch event writes
- Pre-allocate Arrow builders with capacity
- Use H3 indexing for spatial queries
- Profile with `cargo flamegraph`

### DON'T
- Iterate row-by-row over Arrow arrays
- Frequent small writes to Parquet
- String allocations in hot loops
- Unnecessary clones of large RecordBatches

## Troubleshooting

### "Generated code out of sync"
```bash
just generate
cargo clean
cargo build
```

### "Schema mismatch" errors
- Check Arrow schema consistency in builders
- Verify protobuf field types match Arrow types
- Use `df.schema()` to inspect DataFusion table schemas

### Python bindings fail to build
```bash
uv pip install maturin[zig]
just build-py
```

### Large simulations OOM
- Reduce snapshot frequency
- Process data in chunks
- Use streaming Parquet writes
- Monitor with `cargo-instruments` (macOS) or `heaptrack` (Linux)

## Related Documentation

- `README.md` - Entity relationships and legacy Python system
- `ARCHITECTURE.md` - High-level design decisions (if exists)
- `justfile` - All available commands and build tasks
- `Cargo.toml` - Workspace dependencies and versions
- `proto/` - Protocol buffer schema definitions

## File Conventions

| Pattern | Usage |
|---------|-------|
| `*.proto` | Protocol buffer definitions |
| `*_tests.rs` | Unit/integration tests |
| `gen/*.rs` | Generated code - DO NOT EDIT |
| `*.parquet` | Simulation output data |
| `*.json` | Configuration files |

---

**Last Updated**: 2026-03-15
**Simulation Version**: 0.0.1
**Rust Edition**: 2024
