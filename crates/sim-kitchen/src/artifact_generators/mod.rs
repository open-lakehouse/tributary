//! Artifact generators that turn kitchen events into downstream artifacts

pub mod customer_done;
pub mod order_completed;
pub mod order_created;

pub use customer_done::CustomerDoneArtifactGenerator;
pub use order_completed::OrderCompletedArtifactGenerator;
pub use order_created::OrderCreatedArtifactGenerator;
