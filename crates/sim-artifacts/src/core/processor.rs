//! Feeds CloudEvents JSON to an [`ArtifactCollector`] and persists the results

use super::collector::ArtifactCollector;
use super::generator::{ArtifactContext, ArtifactError};
use super::storage::ArtifactStorage;
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Runs events through an [`ArtifactCollector`] and writes the artifacts to
/// an optional [`ArtifactStorage`]
///
/// Feed each event (CloudEvents JSON, e.g. `Event::to_cloudevents_json()`)
/// to [`process`](Self::process); call
/// [`flush_to_storage`](Self::flush_to_storage) to write what has
/// accumulated. Without storage, flushing discards the artifacts.
pub struct ArtifactCollectorProcessor<P> {
    collector: ArtifactCollector<P>,
    profiles: P,
    storage: Option<Box<dyn ArtifactStorage>>,
    run_id: String,
}

impl<P: 'static> ArtifactCollectorProcessor<P> {
    /// A processor with no storage attached
    pub fn new(collector: ArtifactCollector<P>, profiles: P, run_id: impl Into<String>) -> Self {
        Self {
            collector,
            profiles,
            storage: None,
            run_id: run_id.into(),
        }
    }

    /// Attach a storage backend (builder form)
    pub fn with_storage(mut self, storage: Box<dyn ArtifactStorage>) -> Self {
        self.set_storage(storage);
        self
    }

    /// Attach or replace the storage backend
    pub fn set_storage(&mut self, storage: Box<dyn ArtifactStorage>) {
        self.storage = Some(storage);
    }

    /// Generate artifacts for one event; returns how many were produced
    ///
    /// # Errors
    ///
    /// Returns an error if the event has no valid RFC 3339 `time` or a
    /// generator fails.
    pub fn process(&mut self, event: &Value) -> Result<usize, ArtifactError> {
        let sim_time = event
            .get("time")
            .and_then(Value::as_str)
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|t| t.with_timezone(&Utc))
            .ok_or_else(|| {
                ArtifactError::Generator(format!(
                    "event {} has no valid time",
                    event.get("id").and_then(Value::as_str).unwrap_or("<no id>")
                ))
            })?;
        let mut ctx = ArtifactContext {
            event,
            sim_time,
            profiles: &mut self.profiles,
            run_id: &self.run_id,
        };
        self.collector.process_event(&mut ctx)
    }

    /// Hand accumulated artifacts to storage and flush it
    ///
    /// Artifacts move from the collector to storage before the storage
    /// flushes, so a failed write keeps them buffered in the storage.
    pub fn flush_to_storage(&mut self) -> Result<(), ArtifactError> {
        let artifacts = self.collector.flush();
        if let Some(storage) = &mut self.storage {
            storage.persist(artifacts)?;
            storage.flush()?;
        }
        Ok(())
    }

    /// Artifacts generated and not yet flushed
    pub fn artifact_count(&self) -> usize {
        self.collector.artifact_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::artifact::{Artifact, DataArtifact};
    use crate::core::generator::ArtifactGenerator;
    use std::sync::{Arc, Mutex};

    struct Echo;
    impl ArtifactGenerator<()> for Echo {
        fn handles(&self, event_type: &str) -> bool {
            event_type == "test.event"
        }
        fn generate(
            &mut self,
            ctx: &mut ArtifactContext<'_, ()>,
        ) -> Result<Vec<Artifact>, ArtifactError> {
            Ok(vec![Artifact::StructuredData(DataArtifact {
                record_type: "echo".into(),
                source_event_id: ctx.event["id"].as_str().unwrap_or_default().into(),
                data: ctx.event["data"].clone(),
                timestamp: ctx.sim_time,
            })])
        }
    }

    struct Failing;
    impl ArtifactGenerator<()> for Failing {
        fn handles(&self, _: &str) -> bool {
            true
        }
        fn generate(
            &mut self,
            _: &mut ArtifactContext<'_, ()>,
        ) -> Result<Vec<Artifact>, ArtifactError> {
            Err(ArtifactError::Generator("boom".into()))
        }
    }

    #[derive(Default, Clone)]
    struct Recording(Arc<Mutex<Vec<Artifact>>>);
    impl ArtifactStorage for Recording {
        fn persist(&mut self, artifacts: Vec<Artifact>) -> Result<(), ArtifactError> {
            self.0.lock().unwrap().extend(artifacts);
            Ok(())
        }
        fn flush(&mut self) -> Result<(), ArtifactError> {
            Ok(())
        }
    }

    fn event(time: &str) -> Value {
        serde_json::json!({"id": "e1", "type": "test.event", "time": time, "data": {"x": 1}})
    }

    fn processor(generator: Box<dyn ArtifactGenerator<()>>) -> ArtifactCollectorProcessor<()> {
        let mut collector = ArtifactCollector::new();
        collector.add_generator(generator);
        ArtifactCollectorProcessor::new(collector, (), "run")
    }

    #[test]
    fn events_become_artifacts_and_reach_storage() {
        let storage = Recording::default();
        let mut p = processor(Box::new(Echo)).with_storage(Box::new(storage.clone()));
        assert_eq!(p.process(&event("2026-01-01T00:00:00Z")).unwrap(), 1);
        assert_eq!(p.artifact_count(), 1);
        p.flush_to_storage().unwrap();
        assert_eq!(p.artifact_count(), 0);
        let stored = storage.0.lock().unwrap();
        let Artifact::StructuredData(a) = &stored[0] else {
            panic!("unexpected artifact")
        };
        assert_eq!(a.source_event_id, "e1");
        assert_eq!(a.timestamp.to_rfc3339(), "2026-01-01T00:00:00+00:00");
    }

    #[test]
    fn missing_time_is_an_error() {
        let mut p = processor(Box::new(Echo));
        assert!(
            p.process(&serde_json::json!({"type": "test.event"}))
                .is_err()
        );
    }

    #[test]
    fn generator_errors_propagate() {
        let mut p = processor(Box::new(Failing));
        assert!(p.process(&event("2026-01-01T00:00:00Z")).is_err());
    }
}
