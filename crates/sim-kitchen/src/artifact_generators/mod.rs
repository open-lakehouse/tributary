//! Artifact generators that turn kitchen events into downstream artifacts
//!
//! Generators fail on an event that lacks a field they need rather than
//! filling in a placeholder. Artifact IDs (web sessions, messages, documents)
//! are UUID v5s derived from the order ID, so a seeded run reproduces them.

pub mod customer_done;
pub mod order_completed;
pub mod order_created;

pub use customer_done::CustomerDoneArtifactGenerator;
pub use order_completed::OrderCompletedArtifactGenerator;
pub use order_created::OrderCreatedArtifactGenerator;

use crate::resources::kitchen_order::KITCHEN_NAMESPACE;
use sim_artifacts::ArtifactError;
use uuid::Uuid;

/// Deterministic ID for the `kind` artifact of an order
fn artifact_uuid(order_id: &str, kind: &str) -> Uuid {
    Uuid::new_v5(&KITCHEN_NAMESPACE, format!("{order_id}/{kind}").as_bytes())
}

/// Deterministic ID for the `kind` artifact of an order, as a string
fn artifact_id(order_id: &str, kind: &str) -> String {
    artifact_uuid(order_id, kind).to_string()
}

/// The CloudEvents `id` of the event being processed
fn event_id(event: &serde_json::Value) -> Result<&str, ArtifactError> {
    event["id"]
        .as_str()
        .ok_or_else(|| ArtifactError::Generator("event has no `id`".into()))
}

/// A string field of the event's `data`
fn str_field<'a>(event: &'a serde_json::Value, key: &str) -> Result<&'a str, ArtifactError> {
    event["data"][key]
        .as_str()
        .ok_or_else(|| missing(event, key))
}

/// A numeric field of the event's `data`
fn f64_field(event: &serde_json::Value, key: &str) -> Result<f64, ArtifactError> {
    event["data"][key]
        .as_f64()
        .ok_or_else(|| missing(event, key))
}

fn missing(event: &serde_json::Value, key: &str) -> ArtifactError {
    ArtifactError::Generator(format!(
        "`{}` event has no `data.{key}`",
        event["type"].as_str().unwrap_or("?")
    ))
}

/// First eight characters of an ID, for display
fn short(id: &str) -> &str {
    &id[..8.min(id.len())]
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Build CloudEvents JSON the way the processor hands it to generators

    use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

    use crate::profiles::ProfileRegistry;

    pub fn event(event_type: &str, data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "id": "evt-1",
            "type": event_type,
            "source": "/test",
            "specversion": "1.0",
            "time": "2026-01-05T12:00:00Z",
            "data": data,
        })
    }

    pub fn generate(
        generator: &mut dyn ArtifactGenerator<ProfileRegistry>,
        event: &serde_json::Value,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let mut profiles = ProfileRegistry::new();
        let mut ctx = ArtifactContext {
            event,
            sim_time: "2026-01-05T12:00:00Z".parse().unwrap(),
            profiles: &mut profiles,
            run_id: "test",
        };
        generator.generate(&mut ctx)
    }
}
