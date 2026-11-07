# Tributary

[![CI](https://github.com/open-lakehouse/tributary/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/open-lakehouse/tributary/actions/workflows/ci.yml)
[![Release](https://github.com/open-lakehouse/tributary/actions/workflows/release.yml/badge.svg)](https://github.com/open-lakehouse/tributary/actions/workflows/release.yml)

A Rust-based simulation and geospatial processing framework with Python bindings.

## Overview

Tributary is a workspace containing multiple crates for building simulations, event-driven systems, and geospatial data processing, with first-class support for Apache Arrow, Delta Lake, and Python integration.

## Crates

- **sim-core** - Core simulation engine with workflow orchestration
- **sim-events** - CloudEvents 1.0 infrastructure and Parquet persistence
- **sim-geo** - Geospatial functionality and GeoArrow integration
- **sim-kitchen** - Ghost kitchen domain — reference implementation
- **sim-logistics** - Package delivery domain — validates framework generality
- **sim-telemetry** - Observability and telemetry
- **sim-artifacts** - Artifact generation from events (emails, API calls, web events)
- **sim-validate** - Output validation: invariant checks over a run's events and artifacts
- **caspers-cli** - Command-line interface (TUI runner; binary `caspers`)
- **caspers-python** - Python bindings via PyO3

## Requirements

- Rust 1.95 or later
- Python 3.11 or later (for the Python bindings); [uv](https://docs.astral.sh/uv/) and [just](https://github.com/casey/just) for the dev workflow

## Development

```bash
# Everything CI gates on (fmt, clippy, machete, ruff, typos, tests, rustdoc)
just check

# Run tests
just test

# Run benchmarks
cargo bench --workspace

# Build CLI
cargo build --release -p caspers-cli

# Run the TUI simulation (100 steps, 3 kitchens, 20 customers)
cargo run --bin caspers -- run

# Run with custom parameters
cargo run --bin caspers -- run --duration 200 --num-kitchens 5 --num-customers 30

# Headless, seeded run, then validate its output
cargo run --bin caspers -- run --no-tui --duration 200 --seed 42 --events-out events.parquet
cargo run --bin caspers -- validate --events events.parquet --artifacts output/artifacts

# Build and install the Python package into the current environment
just build-py
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE_APACHE](LICENSE_APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE_MIT](LICENSE_MIT) or http://opensource.org/licenses/MIT)

at your option.
