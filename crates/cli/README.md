# caspers-cli

Command-line interface for running Chef Casper's ghost kitchen simulations.

## Installation

```bash
cargo build --bin caspers
```

## Usage

```
caspers <COMMAND> [OPTIONS]
```

### `caspers run`

Run a simulation in a background thread while displaying a live TUI dashboard with:

- A progress bar showing the current step and simulation time
- Cumulative event totals grouped by type
- A scrolling event log with color-coded entries

```bash
caspers run [OPTIONS]
```

| Flag | Short | Default | Description |
| ---- | ----- | ------- | ----------- |
| `--duration` | `-d` | `100` | Steps to run (30 s each) in range mode |
| `--num-kitchens` | — | `3` | Number of ghost kitchens |
| `--kitchen-capacity` | — | `5` | Orders each kitchen prepares at once |
| `--num-customers` | — | `20` | Number of customer agents |
| `--mode` | — | `range` | `range`, `live` (wall-clock paced) or `catchup` |
| `--start-time` / `--end-time` | — | now / none | `now`, `now+3h`, `2026-01-01`, RFC 3339 |
| `--seed` | — | none | Global seed; same seed → same event stream |
| `--events-out` | — | none | Write events to this Parquet file at the end |
| `--artifacts-out` | — | `output/artifacts` | Directory for generated artifacts (JSON Lines) |
| `--no-tui` | — | off | Headless: print a JSON summary; exit non-zero on failure |

**Keyboard shortcuts (during simulation):**

| Key | Action |
| --- | ------ |
| `q` / `Esc` | Quit |
| `Ctrl+C` | Quit |

## Quick start

```bash
# Run for 100 steps with defaults (3 kitchens, 20 customers)
cargo run --bin caspers -- run

# Run 50 steps with 10 customers
cargo run --bin caspers -- run --duration 50 --num-customers 10

# Run with custom kitchen config
cargo run --bin caspers -- run --duration 200 --num-kitchens 5 --kitchen-capacity 8
```

## Telemetry

The CLI ships with OpenTelemetry tracing and metrics. Point an OTLP collector
(e.g. Jaeger) at the default gRPC endpoint and spans will be exported
automatically. Use the justfile recipe to spin up a local Jaeger instance:

```bash
just run-tracing
```

When running with `caspers run`, terminal log output is suppressed to keep the
TUI clean. Traces and metrics are still exported to the configured OTLP
collector. `RUST_LOG` overrides the default log filter
(`caspers`/`sim_*` at debug, everything else at warn).

### `caspers validate`

Check a run's events (and optionally artifacts) against the domain
invariants:

```bash
caspers run --no-tui --duration 200 --seed 42 --events-out events.parquet
caspers validate --events events.parquet --artifacts output/artifacts --kitchen-capacity 15
```
