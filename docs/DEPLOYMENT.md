# Running Tributary

Tributary is a library plus the `caspers` CLI. There is no server, config
file or environment-variable configuration: a run is configured by CLI flags
(or by the Rust/Python API) and writes files.

## Build

```bash
cargo build --release --bin caspers
just build-py          # Python package `caspers` (maturin)
```

The minimum Rust version is `rust-version` in the workspace `Cargo.toml`.

## Run the kitchen simulation

```bash
# Interactive TUI: pause, step, change speed, take/restore snapshots
caspers run --duration 240

# Headless and reproducible; writes output/events.parquet and output/artifacts/
caspers run --no-tui --duration 2880 --seed 42
caspers validate --events output/events.parquet --artifacts output/artifacts \
    --kitchen-capacity 15
```

A seeded range-mode run without `--start-time` starts at
`2026-01-05T11:00:00Z`, so two runs with the same flags emit the same events
at the same times. Only the CloudEvents `id`s differ.

## Run the logistics simulation

```bash
caspers run --no-tui --domain logistics --duration 2880 --seed 42
caspers validate --domain logistics --events output/events.parquet
```

Logistics runs headless and in range mode only, and writes events but no
artifacts.

A step is 30 simulated seconds, so `--duration 2880` is one day. `--mode
live` paces the simulation against the wall clock and `--mode catchup`
fast-forwards from `--start-time` to now, then continues live. See
[`crates/cli/README.md`](../crates/cli/README.md) for every flag.

Headless runs print a JSON summary on stdout and exit non-zero if the
simulation fails or outputs can't be written, so they can gate CI jobs.

## Outputs

| Path | Contents |
|---|---|
| `--events-out FILE` (default `output/events.parquet`) | CloudEvents as Parquet (`id`, `source`, `type`, `time`, `subject`, `data` JSON) |
| `--artifacts-out DIR` (default `output/artifacts`) | Kitchen only: JSON Lines per artifact type (`web_events/`, `emails/`, `structured/`, …) |
| `.caspers/system/` | Catalog of simulations and snapshots (JSON) |
| `.caspers/snapshots/<simulation-id>/` | Snapshots taken from the TUI (Parquet per entity type) |

Everything is written relative to the working directory. `output/` and
`.caspers/` are runtime data and are gitignored.

## Python

```python
import caspers

kitchen = caspers.run_kitchen_simulation(
    steps=2880,
    num_kitchens=3,
    kitchen_capacity=5,
    num_customers=20,
    output_dir="out/kitchen",
    seed=42,
)
logistics = caspers.run_logistics_simulation(steps=2880, output_dir="out/logistics", seed=42)
print(kitchen.events_path, logistics.packages_delivered)
```

Each call writes `events.parquet` (and, for the kitchen, `artifacts/`) under
`output_dir`.

## Observability

The CLI exports OpenTelemetry traces and metrics over OTLP/gRPC to the
default endpoint (`localhost:4317`). `just run-tracing` starts a local Jaeger
(UI at <http://localhost:16686>). Without a collector the exports fail
quietly and the run is unaffected.

`RUST_LOG` sets the log filter (default: `caspers` and `sim_*` at `debug`,
everything else at `warn`). Logs go to stderr in headless mode and are
suppressed in the TUI.

## Sizing

Runs are single-threaded. The default kitchen (3 kitchens, 20 customers)
simulates a day in a few seconds; logistics (two warehouses, 20 drivers)
takes a few seconds per simulated day in debug builds. Memory grows with the
number of entities ever created (workflow instances and packages are kept
after completion), so very long runs with high demand grow linearly.
