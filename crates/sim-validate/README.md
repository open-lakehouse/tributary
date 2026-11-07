# sim-validate

Output validation for simulation runs. Given a finished run's events (Parquet,
via `sim_events::ParquetEventReader`) and, optionally, its artifact directory,
`sim-validate` checks correctness invariants:

- event counts are consistent across lifecycle stages
- events are causally ordered (e.g. an order is created before it is assigned)
- lifecycle pairs match up, with no duplicate terminal events
- resource capacity is never exceeded (when a bound is given)
- artifacts only reference events that exist

The rules are domain-generic. A `DomainProfile` supplies the event types and
lifecycle pairs; built-in profiles cover the kitchen (`DomainProfile::kitchen`)
and logistics (`DomainProfile::logistics`) domains. Rules implement
`InvariantRule` and report `Violation`s instead of panicking.

```rust,no_run
use sim_events::ParquetEventReader;
use sim_validate::{DomainProfile, validate_events};

let events = ParquetEventReader::read_file("events.parquet").unwrap();
let report = validate_events(&events, &DomainProfile::kitchen());
assert!(report.is_ok(), "{:?}", report.violations);
```

From the CLI:

```bash
caspers run --no-tui --duration 200 --seed 42
caspers validate --events output/events.parquet --artifacts output/artifacts
```
