//! Storage trait and implementations for persisting artifacts.

use super::artifact::Artifact;
use super::generator::ArtifactError;

/// Trait for persisting artifacts to durable storage.
pub trait ArtifactStorage: Send + Sync {
    /// Persist a batch of artifacts.
    fn persist(&mut self, artifacts: Vec<Artifact>) -> Result<(), ArtifactError>;
    /// Flush any buffered writes to durable storage.
    fn flush(&mut self) -> Result<(), ArtifactError>;
}

/// A no-op storage implementation for testing.
pub struct NoOpStorage;

impl ArtifactStorage for NoOpStorage {
    fn persist(&mut self, _artifacts: Vec<Artifact>) -> Result<(), ArtifactError> {
        Ok(())
    }
    fn flush(&mut self) -> Result<(), ArtifactError> {
        Ok(())
    }
}
