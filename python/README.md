# caspers

Python bindings for the tributary simulations. `caspers` runs the ghost-kitchen
(`sim-kitchen`) and package-delivery (`sim-logistics`) simulations from Python
and returns their results for analysis. It generates demo datasets and event
streams; it is not a benchmarking or performance-testing tool.

```python
from caspers import run_kitchen_simulation, run_logistics_simulation
```

Build and install into the current environment with `just build-py` (from the
repository root). Requires Python 3.11 or later.

Pass `seed=` for a reproducible run: it starts at 2026-01-05T11:00:00Z and
writes the same events (all but the CloudEvents `id`s) every time.

```python
run = run_kitchen_simulation(steps=2880, output_dir="out/kitchen", seed=42)
```

`just test-py` builds the package into `python/.venv` and runs
`python/tests`.
