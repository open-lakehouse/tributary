//! Synthetic artifacts generated from simulation events
//!
//! Domain crates implement [`ArtifactGenerator`] to turn CloudEvents (as
//! JSON) into [`Artifact`]s: web events, emails, SMS, API calls, documents
//! and structured records. An [`ArtifactCollectorProcessor`] runs events
//! through the generators and writes the results to an [`ArtifactStorage`]
//! such as [`JsonlArtifactStorage`].

pub mod core;
pub mod storage;

pub use core::{
    Artifact, ArtifactCollector, ArtifactCollectorProcessor, ArtifactContext, ArtifactError,
    ArtifactGenerator, ArtifactStorage, NoOpStorage,
};
pub use storage::JsonlArtifactStorage;
