//! Artifact generator for `io.kitchen.order.completed` events.
//!
//! Produces the following artifacts:
//! 1. `DataArtifact`: kitchen_ticket
//! 2. `DataArtifact`: ingredient_usage (per ingredient)
//! 3. `DataArtifact`: quality_control

use sim_artifacts::core::DataArtifact;
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use crate::profiles::ProfileRegistry;

pub struct OrderCompletedArtifactGenerator;

impl OrderCompletedArtifactGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OrderCompletedArtifactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactGenerator<ProfileRegistry> for OrderCompletedArtifactGenerator {
    fn handles(&self, event_type: &str) -> bool {
        event_type == "io.kitchen.order.completed"
    }

    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, ProfileRegistry>,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let data = &ctx.event["data"];
        let order_id = data["order_id"].as_str().unwrap_or("unknown").to_string();
        let kitchen_id = data["kitchen_id"].as_str().unwrap_or("unknown").to_string();
        let now = ctx.sim_time;

        let ticket = Artifact::StructuredData(DataArtifact {
            record_type: "kitchen_ticket".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "completed_at": now.to_rfc3339(),
            }),
            timestamp: now,
        });

        let ingredient_usage = Artifact::StructuredData(DataArtifact {
            record_type: "ingredient_usage".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "recorded_at": now.to_rfc3339(),
            }),
            timestamp: now,
        });

        let temp_check = 165.0 + (now.timestamp() % 10) as f64;
        let qc = Artifact::StructuredData(DataArtifact {
            record_type: "quality_control".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "temp_check_fahrenheit": temp_check,
                "passed": temp_check >= 165.0,
                "checked_at": now.to_rfc3339(),
            }),
            timestamp: now,
        });

        Ok(vec![ticket, ingredient_usage, qc])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_correct_event_type() {
        let generator = OrderCompletedArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.order.completed"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn test_generate_returns_expected_artifact_count() {
        // TODO: implement once we have the impl
        // Just verify the generator compiles and can be constructed
        let _gen = OrderCompletedArtifactGenerator::new();
    }
}
