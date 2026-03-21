//! Artifact types produced by artifact generators.

/// Top-level artifact enum covering all artifact kinds.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Artifact {
    Document(DocumentArtifact),
    StructuredData(DataArtifact),
    WebEvent(WebEventArtifact),
    EmailMessage(EmailArtifact),
    SmsMessage(SmsArtifact),
    PushNotification(PushArtifact),
    ApiCall(ApiCallArtifact),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WebEventArtifact {
    pub session_id: String,
    pub event_name: String,
    pub user_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub device_type: String,
    pub browser: String,
    pub ip_address: String,
    pub utm_source: Option<String>,
    pub utm_medium: Option<String>,
    pub utm_campaign: Option<String>,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmailArtifact {
    pub message_id: String,
    pub from_address: String,
    pub to_address: String,
    pub subject: String,
    pub body_html: String,
    pub email_type: String,
    pub related_entity_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DocumentArtifact {
    pub document_id: String,
    pub document_type: String,
    pub content_type: String,
    pub content: String, // HTML string
    pub related_entity_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiCallArtifact {
    pub service: String,
    pub method: String,
    pub url: String,
    pub request_body: serde_json::Value,
    pub response_body: serde_json::Value,
    pub response_status: u16,
    pub duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataArtifact {
    pub record_type: String,
    pub source_event_id: String,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SmsArtifact {
    pub message_id: String,
    pub from_number: String,
    pub to_number: String,
    pub body: String,
    pub related_entity_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PushArtifact {
    pub notification_id: String,
    pub device_token: String,
    pub title: String,
    pub body: String,
    pub related_entity_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
