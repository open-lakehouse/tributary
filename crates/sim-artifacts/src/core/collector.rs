//! ArtifactCollector — aggregates generators and accumulates produced artifacts.

use super::artifact::Artifact;
use super::generator::{ArtifactContext, ArtifactError, ArtifactGenerator};

/// Collects artifacts produced by registered generators.
pub struct ArtifactCollector<P> {
    generators: Vec<Box<dyn ArtifactGenerator<P>>>,
    artifacts: Vec<Artifact>,
    _phantom: std::marker::PhantomData<P>,
}

impl<P: 'static> ArtifactCollector<P> {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self {
            generators: Vec::new(),
            artifacts: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Register an artifact generator.
    pub fn add_generator(&mut self, generator: Box<dyn ArtifactGenerator<P>>) {
        self.generators.push(generator);
    }

    /// Route the event to all matching generators and accumulate produced artifacts.
    /// Returns the number of new artifacts produced.
    pub fn process_event(
        &mut self,
        ctx: &mut ArtifactContext<'_, P>,
    ) -> Result<usize, ArtifactError> {
        let event_type = ctx.event.get("type").and_then(|v| v.as_str()).unwrap_or("");

        let mut count = 0;
        let mut new_artifacts = Vec::new();

        for generator in &mut self.generators {
            if generator.handles(event_type) {
                let artifacts = generator.generate(ctx)?;
                count += artifacts.len();
                new_artifacts.extend(artifacts);
            }
        }

        self.artifacts.extend(new_artifacts);
        Ok(count)
    }

    /// Drain and return all accumulated artifacts.
    pub fn flush(&mut self) -> Vec<Artifact> {
        std::mem::take(&mut self.artifacts)
    }

    /// Return the current number of buffered artifacts without draining.
    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }
}

impl<P: 'static> Default for ArtifactCollector<P> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};
    use chrono::Utc;
    use serde_json::json;

    struct NoOpGenerator;
    impl ArtifactGenerator<()> for NoOpGenerator {
        fn handles(&self, _: &str) -> bool {
            true
        }
        fn generate(
            &mut self,
            _ctx: &mut ArtifactContext<'_, ()>,
        ) -> Result<Vec<Artifact>, ArtifactError> {
            Ok(vec![])
        }
    }

    #[test]
    fn test_collector_new() {
        let collector: ArtifactCollector<()> = ArtifactCollector::new();
        assert_eq!(collector.artifact_count(), 0);
    }

    #[test]
    fn test_collector_add_generator() {
        let mut collector: ArtifactCollector<()> = ArtifactCollector::new();
        collector.add_generator(Box::new(NoOpGenerator));
        // If we got here without panic, it worked
    }

    #[test]
    fn test_collector_process_event() {
        let mut collector: ArtifactCollector<()> = ArtifactCollector::new();
        collector.add_generator(Box::new(NoOpGenerator));

        let event = json!({"type": "test.event", "data": {}});
        let mut profiles = ();
        let mut ctx = ArtifactContext {
            event: &event,
            sim_time: Utc::now(),
            profiles: &mut profiles,
            run_id: "test-run",
        };

        let count = collector.process_event(&mut ctx).unwrap();
        assert_eq!(count, 0); // NoOpGenerator returns empty vec
    }

    #[test]
    fn test_collector_flush() {
        let mut collector: ArtifactCollector<()> = ArtifactCollector::new();
        let artifacts = collector.flush();
        assert!(artifacts.is_empty());
    }
}
