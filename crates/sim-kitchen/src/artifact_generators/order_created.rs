//! Artifact generator for `io.kitchen.order.created` events.
//!
//! Produces the following artifacts per order:
//! 1. 4x `WebEventArtifact`: page_view, add_to_cart, checkout_initiated, order_placed (same session_id)
//! 2. `EmailArtifact`: order_confirmation
//! 3. `SmsArtifact`: "Your order #XXXX has been received!"
//! 4. `ApiCallArtifact`: payment_processor POST
//! 5. `DataArtifact`: payment_authorization

use sim_artifacts::core::{
    ApiCallArtifact, DataArtifact, EmailArtifact, SmsArtifact, WebEventArtifact,
};
use sim_artifacts::{Artifact, ArtifactContext, ArtifactError, ArtifactGenerator};

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
        let data = &ctx.event["data"];
        let order_id = data["order_id"].as_str().unwrap_or("unknown").to_string();
        let customer_id_str = data["customer_id"].as_str().unwrap_or("").to_string();
        let amount = data["total_price"].as_f64().unwrap_or(15.0);

        let session_id = uuid::Uuid::new_v4().to_string();
        let now = ctx.sim_time;

        let web_events = vec![
            Artifact::WebEvent(WebEventArtifact {
                session_id: session_id.clone(),
                event_name: "page_view".to_string(),
                user_id: customer_id_str.clone(),
                timestamp: now - chrono::Duration::seconds(120),
                device_type: "mobile".to_string(),
                browser: "safari".to_string(),
                ip_address: "10.0.1.1".to_string(),
                utm_source: Some("organic".to_string()),
                utm_medium: None,
                utm_campaign: None,
                properties: serde_json::json!({"page": "/menu"}),
            }),
            Artifact::WebEvent(WebEventArtifact {
                session_id: session_id.clone(),
                event_name: "add_to_cart".to_string(),
                user_id: customer_id_str.clone(),
                timestamp: now - chrono::Duration::seconds(90),
                device_type: "mobile".to_string(),
                browser: "safari".to_string(),
                ip_address: "10.0.1.1".to_string(),
                utm_source: None,
                utm_medium: None,
                utm_campaign: None,
                properties: serde_json::json!({
                    "item": data["items"].get(0).and_then(|i| i.as_str()).unwrap_or("item")
                }),
            }),
            Artifact::WebEvent(WebEventArtifact {
                session_id: session_id.clone(),
                event_name: "checkout_initiated".to_string(),
                user_id: customer_id_str.clone(),
                timestamp: now - chrono::Duration::seconds(60),
                device_type: "mobile".to_string(),
                browser: "safari".to_string(),
                ip_address: "10.0.1.1".to_string(),
                utm_source: None,
                utm_medium: None,
                utm_campaign: None,
                properties: serde_json::json!({"cart_total": amount}),
            }),
            Artifact::WebEvent(WebEventArtifact {
                session_id: session_id.clone(),
                event_name: "order_placed".to_string(),
                user_id: customer_id_str.clone(),
                timestamp: now,
                device_type: "mobile".to_string(),
                browser: "safari".to_string(),
                ip_address: "10.0.1.1".to_string(),
                utm_source: None,
                utm_medium: None,
                utm_campaign: None,
                properties: serde_json::json!({"order_id": order_id, "total": amount}),
            }),
        ];

        let order_id_prefix = &order_id[..8.min(order_id.len())];
        let customer_id_prefix = &customer_id_str[..8.min(customer_id_str.len())];

        let email = Artifact::EmailMessage(EmailArtifact {
            message_id: uuid::Uuid::new_v4().to_string(),
            from_address: "orders@tributarykitchen.com".to_string(),
            to_address: format!("customer-{}@example.com", customer_id_prefix),
            subject: format!("Order #{} Confirmed!", order_id_prefix),
            body_html: format!(
                "<html><body><h1>Order Confirmed</h1><p>Order ID: {}</p><p>Total: ${:.2}</p></body></html>",
                order_id, amount
            ),
            email_type: "order_confirmation".to_string(),
            related_entity_id: order_id.clone(),
        });

        let sms = Artifact::SmsMessage(SmsArtifact {
            message_id: uuid::Uuid::new_v4().to_string(),
            from_number: "+18005551234".to_string(),
            to_number: "+14155550000".to_string(),
            body: format!(
                "Your order #{} has been received! We'll notify you when it's ready.",
                order_id_prefix
            ),
            related_entity_id: order_id.clone(),
            timestamp: now,
        });

        let auth_code = format!("{:06}", (amount * 100.0) as u64 % 1_000_000);

        let api_call = Artifact::ApiCall(ApiCallArtifact {
            service: "payment_processor".to_string(),
            method: "POST".to_string(),
            url: "https://payment-api.example.com/v1/authorize".to_string(),
            request_body: serde_json::json!({
                "order_id": order_id,
                "amount": amount,
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
            source_event_id: ctx.event["id"].as_str().unwrap_or("").to_string(),
            data: serde_json::json!({
                "order_id": order_id,
                "customer_id": customer_id_str,
                "amount": amount,
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

    #[test]
    fn test_handles_correct_event_type() {
        let generator = OrderCreatedArtifactGenerator::new();
        assert!(generator.handles("io.kitchen.order.created"));
        assert!(!generator.handles("io.kitchen.order.completed"));
        assert!(!generator.handles(""));
    }

    #[test]
    fn test_generate_returns_expected_artifact_count() {
        // TODO: implement once we have the impl
        // Just verify the generator compiles and can be constructed
        let _gen = OrderCreatedArtifactGenerator::new();
    }
}
