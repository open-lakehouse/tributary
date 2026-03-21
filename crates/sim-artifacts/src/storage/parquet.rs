use super::super::core::artifact::Artifact;
use super::super::core::generator::ArtifactError;
use super::super::core::storage::ArtifactStorage;
use std::path::PathBuf;

/// Routes artifacts to type-specific Parquet files.
///
/// Output structure:
/// ```text
/// {base_dir}/
/// ├── web_events/web_events.parquet
/// ├── emails/emails.parquet
/// ├── sms/sms.parquet
/// ├── api_calls/api_calls.parquet
/// └── structured/
///     ├── payment_authorizations.parquet
///     ├── kitchen_tickets.parquet
///     ├── ingredient_usage.parquet
///     ├── quality_control.parquet
///     ├── loyalty_transactions.parquet
///     ├── accounts_payable.parquet
///     └── inventory_ledger.parquet
/// └── documents/
///     ├── receipts/receipts.parquet
///     ├── purchase_orders/purchase_orders.parquet
///     └── receiving_manifests/receiving_manifests.parquet
/// ```
pub struct ParquetArtifactStorage {
    base_dir: PathBuf,
    /// Buffer artifacts until flush
    buffer: Vec<Artifact>,
}

impl ParquetArtifactStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            buffer: Vec::new(),
        }
    }

    /// Create all required output directories.
    pub fn ensure_dirs(&self) -> Result<(), std::io::Error> {
        let dirs = [
            "web_events",
            "emails",
            "sms",
            "api_calls",
            "structured",
            "documents/receipts",
            "documents/purchase_orders",
            "documents/receiving_manifests",
        ];
        for dir in &dirs {
            std::fs::create_dir_all(self.base_dir.join(dir))?;
        }
        Ok(())
    }
}

fn write_jsonl_artifact(path: PathBuf, records: &[serde_json::Value]) -> Result<(), ArtifactError> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ArtifactError::Generator(e.to_string()))?;
    }
    let file = std::fs::File::create(&path).map_err(|e| ArtifactError::Generator(e.to_string()))?;
    let mut writer = std::io::BufWriter::new(file);
    for record in records {
        serde_json::to_writer(&mut writer, record).map_err(ArtifactError::Serialization)?;
        writeln!(writer).map_err(|e| ArtifactError::Generator(e.to_string()))?;
    }
    Ok(())
}

impl ArtifactStorage for ParquetArtifactStorage {
    fn persist(&mut self, artifacts: Vec<Artifact>) -> Result<(), ArtifactError> {
        self.buffer.extend(artifacts);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), ArtifactError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        self.ensure_dirs()
            .map_err(|e| ArtifactError::Generator(e.to_string()))?;

        let mut web_events: Vec<serde_json::Value> = Vec::new();
        let mut emails: Vec<serde_json::Value> = Vec::new();
        let mut sms_messages: Vec<serde_json::Value> = Vec::new();
        let mut api_calls: Vec<serde_json::Value> = Vec::new();
        let mut push_notifications: Vec<serde_json::Value> = Vec::new();
        let mut structured: std::collections::HashMap<String, Vec<serde_json::Value>> =
            std::collections::HashMap::new();
        let mut documents: std::collections::HashMap<String, Vec<serde_json::Value>> =
            std::collections::HashMap::new();

        for artifact in self.buffer.drain(..) {
            match artifact {
                Artifact::WebEvent(a) => {
                    web_events.push(serde_json::to_value(&a).map_err(ArtifactError::Serialization)?)
                }
                Artifact::EmailMessage(a) => {
                    emails.push(serde_json::to_value(&a).map_err(ArtifactError::Serialization)?)
                }
                Artifact::SmsMessage(a) => sms_messages
                    .push(serde_json::to_value(&a).map_err(ArtifactError::Serialization)?),
                Artifact::ApiCall(a) => {
                    api_calls.push(serde_json::to_value(&a).map_err(ArtifactError::Serialization)?)
                }
                Artifact::PushNotification(a) => push_notifications
                    .push(serde_json::to_value(&a).map_err(ArtifactError::Serialization)?),
                Artifact::StructuredData(a) => {
                    let key = a.record_type.clone();
                    let val = serde_json::to_value(&a).map_err(ArtifactError::Serialization)?;
                    structured.entry(key).or_default().push(val);
                }
                Artifact::Document(a) => {
                    let key = a.document_type.clone();
                    let val = serde_json::to_value(&a).map_err(ArtifactError::Serialization)?;
                    documents.entry(key).or_default().push(val);
                }
            }
        }

        let base = &self.base_dir;

        if !web_events.is_empty() {
            write_jsonl_artifact(base.join("web_events/web_events.jsonl"), &web_events)?;
        }
        if !emails.is_empty() {
            write_jsonl_artifact(base.join("emails/emails.jsonl"), &emails)?;
        }
        if !sms_messages.is_empty() {
            write_jsonl_artifact(base.join("sms/sms.jsonl"), &sms_messages)?;
        }
        if !api_calls.is_empty() {
            write_jsonl_artifact(base.join("api_calls/api_calls.jsonl"), &api_calls)?;
        }
        if !push_notifications.is_empty() {
            write_jsonl_artifact(
                base.join("push_notifications/push_notifications.jsonl"),
                &push_notifications,
            )?;
        }
        for (record_type, records) in &structured {
            let filename = format!("structured/{}.jsonl", record_type.replace(' ', "_"));
            write_jsonl_artifact(base.join(&filename), records)?;
        }
        for (doc_type, records) in &documents {
            let filename = format!("documents/{}/{}.jsonl", doc_type, doc_type);
            write_jsonl_artifact(base.join(&filename), records)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_storage() {
        let dir = TempDir::new().unwrap();
        let storage = ParquetArtifactStorage::new(dir.path());
        // Should not panic
        drop(storage);
    }

    #[test]
    fn test_persist_empty_vec() {
        let dir = TempDir::new().unwrap();
        let mut storage = ParquetArtifactStorage::new(dir.path());
        // Should not panic on empty
        storage.persist(vec![]).unwrap();
    }

    #[test]
    fn test_flush_empty() {
        let dir = TempDir::new().unwrap();
        let mut storage = ParquetArtifactStorage::new(dir.path());
        storage.flush().unwrap();
    }
}
