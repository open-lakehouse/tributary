//! Artifact generator for `io.kitchen.order.created` events.
//!
//! Produces the following artifacts per order:
//! 1. 4x `WebEventArtifact`: page_view, add_to_cart, checkout_initiated, order_placed (same session_id)
//! 2. `EmailArtifact`: order_confirmation
//! 3. `SmsArtifact`: "Your order #XXXX has been received!"
//! 4. `ApiCallArtifact`: payment_processor POST
//! 5. `DataArtifact`: payment_authorization
//!
//! The payment authorizes the order's full [`OrderCharges::total`]
//! (subtotal, tax and tip), the amount the receipt shows when the customer
//! finishes.

use sim_artifacts::core::{
    ApiCallArtifact, DataArtifact, EmailArtifact, SmsArtifact, WebEventArtifact,
};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

use super::{artifact_id, artifact_uuid, event_id, f64_field, short, str_field};
use crate::pricing::OrderCharges;
use crate::profiles::ProfileRegistry;

pub struct OrderCreatedArtifactGenerator;

impl OrderCreatedArtifactGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OrderCreatedArtifactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtifactGenerator<ProfileRegistry> for OrderCreatedArtifactGenerator {
    fn handles(&self, event_type: &str) -> bool {
        event_type == "io.kitchen.order.created"
    }

    fn generate(
        &mut self,
        ctx: &mut ArtifactContext<'_, ProfileRegistry>,
    ) -> Result<Vec<Artifact>, ArtifactError> {
        let source_event_id = event_id(ctx.event)?.to_string();
        let order_id = str_field(ctx.event, "order_id")?.to_string();
        let customer_id = str_field(ctx.event, "customer_id")?.to_string();
        let charges = OrderCharges::for_subtotal(f64_field(ctx.event, "total_price")?);
        let items = ctx.event["data"]["items"].clone();

        let session_id = artifact_id(&order_id, "web-session");
        let now = ctx.sim_time;

        let web_event = |name: &str, seconds_before: i64, properties: serde_json::Value| {
            Artifact::WebEvent(WebEventArtifact {
                session_id: session_id.clone(),
                event_name: name.to_string(),
                user_id: customer_id.clone(),
                timestamp: now - chrono::Duration::seconds(seconds_before),
                device_type: "mobile".to_string(),
                browser: "safari".to_string(),
                ip_address: "10.0.1.1".to_string(),
                utm_source: (name == "page_view").then(|| "organic".to_string()),
                utm_medium: None,
                utm_campaign: None,
                properties,
            })
        };
        let web_events = vec![
            web_event("page_view", 120, serde_json::json!({"page": "/menu"})),
            web_event("add_to_cart", 90, serde_json::json!({"items": items})),
            web_event(
                "checkout_initiated",
                60,
                serde_json::json!({"cart_total": charges.subtotal}),
            ),
            web_event(
                "order_placed",
                0,
                serde_json::json!({"order_id": order_id, "total": charges.total}),
            ),
        ];

        let email = Artifact::EmailMessage(EmailArtifact {
            message_id: artifact_id(&order_id, "order-confirmation-email"),
            from_address: "orders@tributarykitchen.com".to_string(),
            to_address: format!("customer-{}@example.com", short(&customer_id)),
            subject: format!("Order #{} Confirmed!", short(&order_id)),
            body_html: format!(
                "<html><body><h1>Order Confirmed</h1><p>Order ID: {}</p><p>Total: ${:.2}</p></body></html>",
                order_id, charges.total
            ),
            email_type: "order_confirmation".to_string(),
            related_entity_id: order_id.clone(),
        });

        let sms = Artifact::SmsMessage(SmsArtifact {
            message_id: artifact_id(&order_id, "order-received-sms"),
            from_number: "+18005551234".to_string(),
            to_number: "+14155550000".to_string(),
            body: format!(
                "Your order #{} has been received! We'll notify you when it's ready.",
                short(&order_id)
            ),
            related_entity_id: order_id.clone(),
            timestamp: now,
        });

        let auth_code = format!(
            "{:06}",
            artifact_uuid(&order_id, "payment-auth").as_u128() % 1_000_000
        );

        let api_call = Artifact::ApiCall(ApiCallArtifact {
            service: "payment_processor".to_string(),
            method: "POST".to_string(),
            url: "https://payment-api.example.com/v1/authorize".to_string(),
            request_body: serde_json::json!({
                "order_id": order_id,
                "amount": charges.total,
                "currency": "USD",
                "card_last_four": "4242",
            }),
            response_body: serde_json::json!({
                "auth_code": auth_code,
                "status": "approved",
            }),
            response_status: 200,
            duration_ms: 245,
            timestamp: now,
        });

        let payment_data = Artifact::StructuredData(DataArtifact {
            record_type: "payment_authorization".to_string(),
            source_event_id,
            data: serde_json::json!({
                "order_id": order_id,
                "customer_id": customer_id,
                "subtotal": charges.subtotal,
                "tax": charges.tax,
                "tip": charges.tip,
                "amount": charges.total,
                "currency": "USD",
                "card_last_four": "4242",
                "auth_code": auth_code,
                "status": "approved",
            }),
            timestamp: now,
        });

        let mut artifacts = web_events;
        artifacts.push(email);
        artifacts.push(sms);
        artifacts.push(api_call);
        artifacts.push(payment_data);

        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_generators::test_support::{event, generate};

    fn order_created() -> serde_json::Value {
        event(
            "io.kitchen.order.created",
            serde_json::json!({
                "order_id": "order-abc-1",
                "customer_id": "cust-1234567890",
                "items": ["burger"],
                "total_price": 12.5,
            }),
        )
    }

    #[test]
    fn test_handles_correct_event_type() {
        let generator = OrderCreatedArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.order.created"));
        assert!(!generator.handles("io.kitchen.order.completed"));
        assert!(!generator.handles(""));
    }

    #[test]
    fn generates_one_session_and_a_payment_for_the_full_charge() {
        let artifacts = generate(&mut OrderCreatedArtifactGenerator::new(), &order_created())
            .expect("valid event");
        assert_eq!(artifacts.len(), 8);

        let sessions: Vec<&str> = artifacts
            .iter()
            .filter_map(|a| match a {
                Artifact::WebEvent(w) => Some(w.session_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(sessions.len(), 4);
        assert!(sessions.iter().all(|s| *s == sessions[0]));

        let total = OrderCharges::for_subtotal(12.5).total;
        let payment = artifacts
            .iter()
            .find_map(|a| match a {
                Artifact::StructuredData(d) if d.record_type == "payment_authorization" => Some(d),
                _ => None,
            })
            .expect("payment_authorization");
        assert_eq!(payment.source_event_id, "evt-1");
        assert_eq!(payment.data["amount"], total);
        assert_eq!(payment.data["subtotal"], 12.5);

        let api = artifacts
            .iter()
            .find_map(|a| match a {
                Artifact::ApiCall(c) => Some(c),
                _ => None,
            })
            .expect("payment API call");
        assert_eq!(api.request_body["amount"], total);
        assert_eq!(api.response_body["auth_code"], payment.data["auth_code"]);
    }

    #[test]
    fn ids_derive_from_the_order() {
        let ids = |e: &serde_json::Value| -> Vec<String> {
            generate(&mut OrderCreatedArtifactGenerator::new(), e)
                .unwrap()
                .iter()
                .filter_map(|a| match a {
                    Artifact::WebEvent(w) => Some(w.session_id.clone()),
                    Artifact::EmailMessage(m) => Some(m.message_id.clone()),
                    Artifact::SmsMessage(m) => Some(m.message_id.clone()),
                    _ => None,
                })
                .collect()
        };
        let a = order_created();
        let mut b = order_created();
        b["data"]["order_id"] = "order-abc-2".into();
        assert_eq!(ids(&a), ids(&a));
        assert_ne!(ids(&a), ids(&b));
    }

    #[test]
    fn missing_price_is_an_error() {
        let mut e = order_created();
        e["data"].as_object_mut().unwrap().remove("total_price");
        let err = generate(&mut OrderCreatedArtifactGenerator::new(), &e).unwrap_err();
        assert!(err.to_string().contains("total_price"), "{err}");
    }
}
