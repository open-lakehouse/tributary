//! Artifact generator for `io.kitchen.customer.done` events.
//!
//! Produces the following artifacts:
//! 1. `DocumentArtifact`: receipt (HTML)
//! 2. `DataArtifact`: loyalty_transaction
//! 3. `EmailArtifact`: review_request
//!
//! The receipt shows the same [`OrderCharges`] the payment authorized when
//! the order was placed.

use sim_artifacts::core::{DataArtifact, DocumentArtifact, EmailArtifact};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use super::{artifact_id, event_id, f64_field, short, str_field};
use crate::pricing::{OrderCharges, TAX_RATE, TIP_RATE};
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
        let source_event_id = event_id(ctx.event)?.to_string();
        let order_id = str_field(ctx.event, "order_id")?.to_string();
        let customer_id = str_field(ctx.event, "customer_id")?.to_string();
        let charges = OrderCharges::for_subtotal(f64_field(ctx.event, "total_price")?);
        let now = ctx.sim_time;

        let receipt = Artifact::Document(DocumentArtifact {
            document_id: artifact_id(&order_id, "receipt"),
            document_type: "receipt".to_string(),
            content_type: "text/html".to_string(),
            content: format!(
                "<html><body><h1>Tributary Kitchen Receipt</h1>\
                <p>Order: {}</p>\
                <table><tr><td>Subtotal</td><td>${:.2}</td></tr>\
                <tr><td>Tax ({:.1}%)</td><td>${:.2}</td></tr>\
                <tr><td>Tip ({:.0}%)</td><td>${:.2}</td></tr>\
                <tr><td><strong>Total</strong></td><td><strong>${:.2}</strong></td></tr>\
                </table></body></html>",
                order_id,
                charges.subtotal,
                TAX_RATE * 100.0,
                charges.tax,
                TIP_RATE * 100.0,
                charges.tip,
                charges.total
            ),
            related_entity_id: order_id.clone(),
        });

        let loyalty = Artifact::StructuredData(DataArtifact {
            record_type: "loyalty_transaction".to_string(),
            source_event_id,
            data: serde_json::json!({
                "customer_id": customer_id,
                "order_id": order_id,
                "amount": charges.total,
                "points_earned": charges.total.floor() as u64,
                "transaction_type": "earn",
            }),
            timestamp: now,
        });

        let review_email = Artifact::EmailMessage(EmailArtifact {
            message_id: artifact_id(&order_id, "review-request-email"),
            from_address: "feedback@tributarykitchen.com".to_string(),
            to_address: format!("customer-{}@example.com", short(&customer_id)),
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
    use crate::artifact_generators::test_support::{event, generate};

    fn customer_done() -> serde_json::Value {
        event(
            "io.kitchen.customer.done",
            serde_json::json!({
                "order_id": "order-abc-1",
                "customer_id": "cust-1234567890",
                "total_price": 12.5,
                "total_time_seconds": 1500,
            }),
        )
    }

    #[test]
    fn test_handles_correct_event_type() {
        let generator = CustomerDoneArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.customer.done"));
        assert!(!generator.handles("io.kitchen.other.event"));
    }

    #[test]
    fn receipt_and_loyalty_use_the_full_charge() {
        let artifacts = generate(&mut CustomerDoneArtifactGenerator::new(), &customer_done())
            .expect("valid event");
        assert_eq!(artifacts.len(), 3);
        let total = OrderCharges::for_subtotal(12.5).total;

        let Artifact::Document(receipt) = &artifacts[0] else {
            panic!("expected the receipt first, got {:?}", artifacts[0]);
        };
        assert_eq!(receipt.related_entity_id, "order-abc-1");
        assert!(
            receipt.content.contains(&format!("${total:.2}")),
            "{}",
            receipt.content
        );

        let Artifact::StructuredData(loyalty) = &artifacts[1] else {
            panic!("expected the loyalty record, got {:?}", artifacts[1]);
        };
        assert_eq!(loyalty.source_event_id, "evt-1");
        assert_eq!(loyalty.data["amount"], total);
        assert_eq!(loyalty.data["points_earned"], total.floor() as u64);
    }

    #[test]
    fn missing_customer_is_an_error() {
        let mut e = customer_done();
        e["data"].as_object_mut().unwrap().remove("customer_id");
        let err = generate(&mut CustomerDoneArtifactGenerator::new(), &e).unwrap_err();
        assert!(err.to_string().contains("customer_id"), "{err}");
    }
}
