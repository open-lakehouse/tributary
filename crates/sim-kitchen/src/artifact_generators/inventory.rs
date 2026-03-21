//! Artifact generator for `io.kitchen.inventory.replenished` events.
//!
//! Produces the following artifacts:
//! 1. `DocumentArtifact`: receiving_manifest (HTML)
//! 2. `DataArtifact`: inventory_ledger_entry

use sim_artifacts::core::{DataArtifact, DocumentArtifact};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use crate::profiles::ProfileRegistry;

pub struct InventoryArtifactGenerator;

impl InventoryArtifactGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InventoryArtifactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactGenerator<ProfileRegistry> for InventoryArtifactGenerator {
    fn handles(&self, event_type: &str) -> bool {
        event_type == "io.kitchen.inventory.replenished"
    }

    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, ProfileRegistry>,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let data = &ctx.event["data"];
        let item_name = data["item"].as_str().unwrap_or("ingredient").to_string();
        let qty_added = data["quantity_added"].as_f64().unwrap_or(10.0);
        let kitchen_id = data["kitchen_id"].as_str().unwrap_or("unknown").to_string();
        let now = ctx.sim_time;
        let lot_uuid = uuid::Uuid::new_v4().to_string();
        let lot_number = format!("LOT-{}", lot_uuid[..8].to_uppercase());

        let manifest = Artifact::Document(DocumentArtifact {
            document_id: uuid::Uuid::new_v4().to_string(),
            document_type: "receiving_manifest".to_string(),
            content_type: "text/html".to_string(),
            content: format!(
                "<html><body><h1>Receiving Manifest</h1>\
                <p>Item: {}</p><p>Qty: {}</p><p>Lot: {}</p>\
                <p>Kitchen: {}</p><p>Date: {}</p>\
                </body></html>",
                item_name,
                qty_added,
                lot_number,
                kitchen_id,
                now.format("%Y-%m-%d")
            ),
            related_entity_id: lot_number.clone(),
        });

        let ledger = Artifact::StructuredData(DataArtifact {
            record_type: "inventory_ledger_entry".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "item": item_name,
                "qty_added": qty_added,
                "lot_number": lot_number,
                "kitchen_id": kitchen_id,
                "received_at": now.to_rfc3339(),
            }),
            timestamp: now,
        });

        Ok(vec![manifest, ledger])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_correct_event_type() {
        let generator = InventoryArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.inventory.replenished"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn test_generate_returns_expected_artifact_count() {
        // TODO: implement once we have the impl
        // Just verify the generator compiles and can be constructed
        let _gen = InventoryArtifactGenerator::new();
    }
}
