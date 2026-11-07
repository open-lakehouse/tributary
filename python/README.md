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
