# caspers-universe-cli

Command-line interface for running and managing Chef Casper's ghost kitchen simulations.

## Installation

```bash
cargo build --bin caspers
```

## Usage

```
caspers <COMMAND> [OPTIONS]
```

### Global options

| Option     | Env var        | Default                  | Description |
| ---------- | -------------- | ------------------------ | ----------- |
| `--server` | `UC_SERVER_URL` | `http://localhost:8080` | Server URL  |

### `caspers init`

Bootstrap a new simulation environment. Interactively selects sites (Amsterdam,
Berlin, London) and brands (Asian, Fast Food, Mexican) to include, then writes
the initial data to the working directory.

```bash
caspers init --working-directory .caspers/
```

| Flag                    | Short | Default | Description                              |
| ----------------------- | ----- | ------- | ---------------------------------------- |
| `--working-directory`   | `-w`  | —       | Path where simulation data is stored     |
| `--template`            | `-t`  | `true`  | Use built-in site/brand templates        |

### `caspers run`

Run a simulation. Prompts you to select a simulation and snapshot, then
executes the simulation in a background thread while displaying a live TUI
dashboard with:

- A progress bar showing the current step and simulation time
- Cumulative event totals grouped by type
- A scrolling event log with color-coded entries

```bash
caspers run --duration 200 --working-directory .caspers/
```

| Flag                    | Short | Default | Description                              |
| ----------------------- | ----- | ------- | ---------------------------------------- |
| `--duration`            | `-d`  | `100`   | Number of simulation steps to run        |
| `--working-directory`   | `-w`  | —       | Path where simulation data is stored     |
| `--dry-run`             | —     | `false` | Skip writing the final snapshot to disk  |

**Keyboard shortcuts (during simulation):**

| Key          | Action                  |
| ------------ | ----------------------- |
| `q` / `Esc`  | Quit                   |
| `Ctrl+C`     | Quit                   |

### `caspers server`

Start the web server that serves the UI and documentation assets.

```bash
caspers server
```

| Flag       | Env var              | Default          | Description              |
| ---------- | -------------------- | ---------------- | ------------------------ |
| `--server` | `CASPERS_SERVER_URL` | `0.0.0.0:8000`  | Address to listen on     |

## Quick start

```bash
# 1. Initialize a simulation with default templates
caspers init -w .caspers/

# 2. Run for 200 steps
caspers run -d 200 -w .caspers/

# 3. (Optional) Start the web server
caspers server
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
collector.
