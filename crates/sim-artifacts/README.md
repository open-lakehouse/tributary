# sim-artifacts

Turns simulation events into the downstream records a real business would
produce: payment API calls, confirmation emails, SMS and push notifications, web
analytics events, and data or document exports.

- Implement `ArtifactGenerator<P>` for each event type you want to translate.
  `P` is your domain's profile data (customers, pricing tables, …), and each
  generator receives an `ArtifactContext` with the event, simulation time,
  profiles and run id.
- Register generators on an `ArtifactCollector<P>`, then wrap it in an
  `ArtifactCollectorProcessor<P>` and feed it each event's CloudEvents JSON
  (`Event::to_cloudevents_json()`) with `process`. Generator errors and events
  without a valid `time` are returned as errors.
- Call `flush_to_storage()` to write what has accumulated (it can be called
  repeatedly). `JsonlArtifactStorage` writes one JSON Lines file per artifact
  kind under the directory you give it; the first flush replaces old files and
  later ones append. `NoOpStorage` discards artifacts.

`sim-kitchen` is the reference user; see its `artifact_generators` module.
