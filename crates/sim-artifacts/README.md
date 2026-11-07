# sim-artifacts

Turns simulation events into the downstream records a real business would
produce: payment API calls, confirmation emails, SMS and push notifications, web
analytics events, and data or document exports.

- Implement `ArtifactGenerator<P>` for each event type you want to translate.
  `P` is your domain's profile data (customers, pricing tables, …), and each
  generator receives an `ArtifactContext` with the event, simulation time,
  profiles and run id.
- Register generators on an `ArtifactCollector<P>`, then wrap it in an
  `ArtifactCollectorProcessor<P>`. The processor implements
  `LocalEventProcessor`, so it plugs into the `sim-events` pipeline.
- Call `flush_to_storage()` at the end of a run. `ParquetArtifactStorage` writes
  one Parquet file per artifact kind (under `output/artifacts/` by default);
  `NoOpStorage` discards them, for tests.

`sim-kitchen` is the reference user; see its `artifact_generators` module.
