//! Artifact generator for `io.kitchen.order.completed` events.
//!
//! Produces the following artifacts:
//! 1. `DataArtifact`: kitchen_ticket
//! 2. `DataArtifact`: ingredient_usage
//! 3. `DataArtifact`: quality_control

use sim_artifacts::core::DataArtifact;
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use super::{event_id, str_field};
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
        let source_event_id = event_id(ctx.event)?.to_string();
        let order_id = str_field(ctx.event, "order_id")?;
        let kitchen_id = str_field(ctx.event, "kitchen_id")?;
        let now = ctx.sim_time;

        let record = |record_type: &str, data: serde_json::Value| {
            Artifact::StructuredData(DataArtifact {
                record_type: record_type.to_string(),
                source_event_id: source_event_id.clone(),
                data,
                timestamp: now,
            })
        };

        let ticket = record(
            "kitchen_ticket",
            serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "completed_at": now.to_rfc3339(),
            }),
        );

        let ingredient_usage = record(
            "ingredient_usage",
            serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "recorded_at": now.to_rfc3339(),
            }),
        );

        let temp_check = 165.0 + (now.timestamp() % 10) as f64;
        let qc = record(
            "quality_control",
            serde_json::json!({
                "order_id": order_id,
                "kitchen_id": kitchen_id,
                "temp_check_fahrenheit": temp_check,
                "passed": temp_check >= 165.0,
                "checked_at": now.to_rfc3339(),
            }),
        );

        Ok(vec![ticket, ingredient_usage, qc])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_generators::test_support::{event, generate};

    fn order_completed() -> serde_json::Value {
        event(
            "io.kitchen.order.completed",
            serde_json::json!({
                "order_id": "order-abc-1",
                "customer_id": "cust-1",
                "kitchen_id": "kitchen-1",
                "prep_time_seconds": 480,
            }),
        )
    }

    #[test]
    fn test_handles_correct_event_type() {
        let generator = OrderCompletedArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.order.completed"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn generates_records_for_the_order_and_kitchen() {
        let artifacts = generate(
            &mut OrderCompletedArtifactGenerator::new(),
            &order_completed(),
        )
        .expect("valid event");
        let records: Vec<&DataArtifact> = artifacts
            .iter()
            .map(|a| match a {
                Artifact::StructuredData(d) => d,
                other => panic!("unexpected artifact {other:?}"),
            })
            .collect();
        let types: Vec<&str> = records.iter().map(|d| d.record_type.as_str()).collect();
        assert_eq!(
            types,
            ["kitchen_ticket", "ingredient_usage", "quality_control"]
        );
        for d in records {
            assert_eq!(d.source_event_id, "evt-1");
            assert_eq!(d.data["order_id"], "order-abc-1");
            assert_eq!(d.data["kitchen_id"], "kitchen-1");
        }
    }

    #[test]
    fn missing_kitchen_is_an_error() {
        let mut e = order_completed();
        e["data"].as_object_mut().unwrap().remove("kitchen_id");
        let err = generate(&mut OrderCompletedArtifactGenerator::new(), &e).unwrap_err();
        assert!(err.to_string().contains("kitchen_id"), "{err}");
    }
}
