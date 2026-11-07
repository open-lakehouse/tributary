//! EventProcessor adapter wrapping ArtifactCollector.
//!
//! Bridges the sim-events EventProcessor interface to `ArtifactCollector<P>`.
//! Since sim-artifacts cannot depend on sim-events (to avoid cycles),
//! this processor uses the same local EventProcessor pattern as sim-telemetry.

use super::collector::ArtifactCollector;
use super::generator::{ArtifactContext, ArtifactError};
use super::storage::ArtifactStorage;
use chrono::Utc;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Local EventProcessor trait (mirrors sim_events::EventProcessor but uses Value).
/// This avoids a circular dependency: sim-events → sim-core, sim-artifacts has no sim-events dep.
pub trait LocalEventProcessor: Send + Sync {
    fn handles(&self, event_type: &str) -> bool;
    fn process(&self, event: &Value);
}

/// Wraps an `ArtifactCollector<P>` as a `LocalEventProcessor`.
///
/// On each event, extracts the event type, routes to matching generators,
/// and accumulates artifacts. Call flush_to_storage() at run end.
pub struct ArtifactCollectorProcessor<P: Send + 'static> {
    inner: Arc<Mutex<ArtifactCollectorInner<P>>>,
    run_id: String,
}

struct ArtifactCollectorInner<P> {
    collector: ArtifactCollector<P>,
    profiles: P,
    storage: Option<Box<dyn ArtifactStorage>>,
}

impl<P: Send + 'static> ArtifactCollectorProcessor<P> {
    /// Create a new processor with the given collector and profiles.
    pub fn new(collector: ArtifactCollector<P>, profiles: P, run_id: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ArtifactCollectorInner {
                collector,
                profiles,
                storage: None,
            })),
            run_id: run_id.into(),
        }
    }

    /// Attach a storage backend.
    pub fn with_storage(self, storage: Box<dyn ArtifactStorage>) -> Self {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.storage = Some(storage);
        }
        self
    }

    /// Flush all accumulated artifacts to the attached storage.
    pub fn flush_to_storage(&self) -> Result<(), ArtifactError> {
        let mut inner = self.inner.lock().unwrap();
        let artifacts = inner.collector.flush();
        if let Some(storage) = &mut inner.storage {
            storage.persist(artifacts)?;
            storage.flush()?;
        }
        Ok(())
    }

    /// Get the count of accumulated artifacts.
    pub fn artifact_count(&self) -> usize {
        self.inner.lock().unwrap().collector.artifact_count()
    }
}

impl<P: Send + 'static> LocalEventProcessor for ArtifactCollectorProcessor<P> {
    fn handles(&self, _event_type: &str) -> bool {
        true
    }

    fn process(&self, event: &Value) {
        let run_id = self.run_id.clone();
        let sim_time = event
            .get("time")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|| {
                // Should never happen for correctly-emitted events.
                // Use Unix epoch so bad data is immediately obvious in output.
                chrono::DateTime::UNIX_EPOCH.with_timezone(&Utc)
            });

        let mut inner = self.inner.lock().unwrap();
        // Split the borrow: take a raw pointer to collector so we can borrow profiles separately.
        // SAFETY: collector and profiles are distinct fields of the same struct; no aliasing occurs.
        let collector = &mut inner.collector as *mut ArtifactCollector<P>;
        let mut ctx = ArtifactContext {
            event,
            sim_time,
            profiles: &mut inner.profiles,
            run_id: &run_id,
        };
        let _ = unsafe { (*collector).process_event(&mut ctx) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::collector::ArtifactCollector;

    #[test]
    fn test_processor_new() {
        let collector: ArtifactCollector<()> = ArtifactCollector::new();
        let _processor = ArtifactCollectorProcessor::new(collector, (), "test-run");
    }

    #[test]
    fn test_artifact_count_initially_zero() {
        let collector: ArtifactCollector<()> = ArtifactCollector::new();
        let processor = ArtifactCollectorProcessor::new(collector, (), "test-run");
        assert_eq!(processor.artifact_count(), 0);
    }
}
