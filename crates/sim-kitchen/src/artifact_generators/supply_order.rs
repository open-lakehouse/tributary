//! Artifact generator for `io.kitchen.supply.order.created` events.
//!
//! Produces the following artifacts:
//! 1. `DocumentArtifact`: purchase_order (HTML)
//! 2. `ApiCallArtifact`: vendor API POST
//! 3. `DataArtifact`: accounts_payable_entry

use sim_artifacts::core::{ApiCallArtifact, DataArtifact, DocumentArtifact};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use crate::profiles::ProfileRegistry;

pub struct SupplyOrderArtifactGenerator;

impl SupplyOrderArtifactGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SupplyOrderArtifactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactGenerator<ProfileRegistry> for SupplyOrderArtifactGenerator {
    fn handles(&self, event_type: &str) -> bool {
        event_type == "io.kitchen.supply.order.created"
    }

    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, ProfileRegistry>,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let data = &ctx.event["data"];
        let po_uuid = uuid::Uuid::new_v4().to_string();
        let po_number = format!("PO-{}", po_uuid[..8].to_uppercase());
        let vendor_name = data["vendor"]
            .as_str()
            .unwrap_or("Fresh Foods Co")
            .to_string();
        let total_amount = data["total_cost"].as_f64().unwrap_or(500.0);
        let now = ctx.sim_time;

        let po_doc = Artifact::Document(DocumentArtifact {
            document_id: uuid::Uuid::new_v4().to_string(),
            document_type: "purchase_order".to_string(),
            content_type: "text/html".to_string(),
            content: format!(
                "<html><body><h1>Purchase Order {}</h1>\
                <p>Vendor: {}</p>\
                <p>Date: {}</p>\
                <p>Total: ${:.2}</p>\
                </body></html>",
                po_number,
                vendor_name,
                now.format("%Y-%m-%d"),
                total_amount
            ),
            related_entity_id: po_number.clone(),
        });

        let vendor_api = Artifact::ApiCall(ApiCallArtifact {
            service: "vendor_api".to_string(),
            method: "POST".to_string(),
            url: "https://vendor-api.example.com/orders".to_string(),
            request_body: serde_json::json!({
                "po_number": po_number,
                "vendor": vendor_name,
                "total": total_amount,
            }),
            response_body: serde_json::json!({
                "order_id": po_number,
                "status": "accepted",
            }),
            response_status: 201,
            duration_ms: 380,
            timestamp: now,
        });

        let ap_entry = Artifact::StructuredData(DataArtifact {
            record_type: "accounts_payable_entry".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "po_number": po_number,
                "vendor": vendor_name,
                "amount": total_amount,
                "due_date": (now + chrono::Duration::days(30)).format("%Y-%m-%d").to_string(),
                "status": "pending",
            }),
            timestamp: now,
        });

        Ok(vec![po_doc, vendor_api, ap_entry])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_correct_event_type() {
        let generator = SupplyOrderArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.supply.order.created"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn test_generate_returns_expected_artifact_count() {
        // TODO: implement once we have the impl
        // Just verify the generator compiles and can be constructed
        let _gen = SupplyOrderArtifactGenerator::new();
    }
}
