pub mod artifact;
pub mod collector;
pub mod generator;
pub mod processor;
pub mod storage;

pub use artifact::{
    ApiCallArtifact, Artifact, DataArtifact, DocumentArtifact, EmailArtifact, PushArtifact,
    SmsArtifact, WebEventArtifact,
};
pub use collector::ArtifactCollector;
pub use generator::{ArtifactContext, ArtifactError, ArtifactGenerator};
pub use processor::{ArtifactCollectorProcessor, LocalEventProcessor};
pub use storage::{ArtifactStorage, NoOpStorage};
