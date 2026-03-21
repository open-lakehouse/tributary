//! ArtifactGenerator trait and supporting types.

/// Errors that can occur during artifact generation.
#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Generator error: {0}")]
    Generator(String),
}

/// Context passed to each generator when processing an event.
pub struct ArtifactContext<'a, P> {
    pub event: &'a serde_json::Value,
    pub sim_time: chrono::DateTime<chrono::Utc>,
    pub profiles: &'a mut P,
    pub run_id: &'a str,
}

/// Trait implemented by types that can produce artifacts from simulation events.
pub trait ArtifactGenerator<P>: Send + Sync {
    /// Return true if this generator handles the given event type.
    fn handles(&self, event_type: &str) -> bool;
    /// Generate zero or more artifacts from the given context.
    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, P>,
    ) -> Result<Vec<super::artifact::Artifact>, ArtifactError>;
}
