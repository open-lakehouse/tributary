//! Artifact generator for `io.kitchen.customer.done` events.
//!
//! Produces the following artifacts:
//! 1. `DocumentArtifact`: receipt (HTML)
//! 2. `DataArtifact`: loyalty_transaction
//! 3. `EmailArtifact`: review_request

use sim_artifacts::core::{DataArtifact, DocumentArtifact, EmailArtifact};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use crate::profiles::ProfileRegistry;

pub struct CustomerDoneArtifactGenerator;

impl CustomerDoneArtifactGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CustomerDoneArtifactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactGenerator<ProfileRegistry> for CustomerDoneArtifactGenerator {
    fn handles(&self, event_type: &str) -> bool {
        event_type == "io.kitchen.customer.done"
    }

    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, ProfileRegistry>,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let data = &ctx.event["data"];
        let order_id = data["order_id"].as_str().unwrap_or("unknown").to_string();
        let customer_id = data["customer_id"].as_str().unwrap_or("").to_string();
        let amount = data["total_price"].as_f64().unwrap_or(15.0);
        let tax = amount * 0.085;
        let tip = amount * 0.18;
        let total = amount + tax + tip;
        let now = ctx.sim_time;

        let customer_id_prefix = &customer_id[..8.min(customer_id.len())];

        let receipt = Artifact::Document(DocumentArtifact {
            document_id: uuid::Uuid::new_v4().to_string(),
            document_type: "receipt".to_string(),
            content_type: "text/html".to_string(),
            content: format!(
                "<html><body><h1>Tributary Kitchen Receipt</h1>\
                <p>Order: {}</p>\
                <table><tr><td>Subtotal</td><td>${:.2}</td></tr>\
                <tr><td>Tax (8.5%)</td><td>${:.2}</td></tr>\
                <tr><td>Tip (18%)</td><td>${:.2}</td></tr>\
                <tr><td><strong>Total</strong></td><td><strong>${:.2}</strong></td></tr>\
                </table></body></html>",
                order_id, amount, tax, tip, total
            ),
            related_entity_id: order_id.clone(),
        });

        let loyalty = Artifact::StructuredData(DataArtifact {
            record_type: "loyalty_transaction".to_string(),
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "customer_id": customer_id,
                "order_id": order_id,
                "points_earned": (total as u64),
                "transaction_type": "earn",
            }),
            timestamp: now,
        });

        let review_email = Artifact::EmailMessage(EmailArtifact {
            message_id: uuid::Uuid::new_v4().to_string(),
            from_address: "feedback@tributarykitchen.com".to_string(),
            to_address: format!("customer-{}@example.com", customer_id_prefix),
            subject: "How was your order?".to_string(),
            body_html: format!(
                "<html><body><h2>How was your order?</h2>\
                <p>We'd love to hear your feedback on order {}.</p>\
                <p><a href='https://review.example.com/{}'>Leave a review</a></p>\
                </body></html>",
                order_id, order_id
            ),
            email_type: "review_request".to_string(),
            related_entity_id: order_id.clone(),
        });

        Ok(vec![receipt, loyalty, review_email])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_correct_event_type() {
        let generator = CustomerDoneArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.customer.done"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn test_generate_returns_expected_artifact_count() {
        // TODO: implement once we have the impl
        // Just verify the generator compiles and can be constructed
        let _gen = CustomerDoneArtifactGenerator::new();
    }
}
