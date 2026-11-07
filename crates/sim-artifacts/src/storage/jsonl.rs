//! JSON Lines artifact storage

use super::super::core::artifact::Artifact;
use super::super::core::generator::ArtifactError;
use super::super::core::storage::ArtifactStorage;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

/// Writes artifacts as JSON Lines files, one per artifact type
///
/// Output structure (files appear only for types that occur):
/// ```text
/// {base_dir}/
/// ├── web_events/web_events.jsonl
/// ├── emails/emails.jsonl
/// ├── sms/sms.jsonl
/// ├── api_calls/api_calls.jsonl
/// ├── push_notifications/push_notifications.jsonl
/// ├── structured/{record_type}.jsonl
/// └── documents/{document_type}/{document_type}.jsonl
/// ```
///
/// The first flush of a storage instance replaces existing files; later
/// flushes append, so a run can flush repeatedly. If a write fails, the
/// buffered artifacts are kept and the next flush retries them (files written
/// before the failure may then contain duplicates).
pub struct JsonlArtifactStorage {
    base_dir: PathBuf,
    /// Artifacts waiting for the next flush
    buffer: Vec<Artifact>,
    /// Files this instance has written (appended to on later flushes)
    written: HashSet<PathBuf>,
}

impl JsonlArtifactStorage {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            buffer: Vec::new(),
            written: HashSet::new(),
        }
    }

    /// Relative file an artifact belongs in
    fn file_for(artifact: &Artifact) -> String {
        match artifact {
            Artifact::WebEvent(_) => "web_events/web_events.jsonl".into(),
            Artifact::EmailMessage(_) => "emails/emails.jsonl".into(),
            Artifact::SmsMessage(_) => "sms/sms.jsonl".into(),
            Artifact::ApiCall(_) => "api_calls/api_calls.jsonl".into(),
            Artifact::PushNotification(_) => "push_notifications/push_notifications.jsonl".into(),
            Artifact::StructuredData(a) => {
                format!("structured/{}.jsonl", a.record_type.replace(' ', "_"))
            }
            Artifact::Document(a) => format!("documents/{0}/{0}.jsonl", a.document_type),
        }
    }

    fn to_json(artifact: &Artifact) -> Result<serde_json::Value, serde_json::Error> {
        match artifact {
            Artifact::WebEvent(a) => serde_json::to_value(a),
            Artifact::EmailMessage(a) => serde_json::to_value(a),
            Artifact::SmsMessage(a) => serde_json::to_value(a),
            Artifact::ApiCall(a) => serde_json::to_value(a),
            Artifact::PushNotification(a) => serde_json::to_value(a),
            Artifact::StructuredData(a) => serde_json::to_value(a),
            Artifact::Document(a) => serde_json::to_value(a),
        }
    }

    fn write(&mut self, rel: &str, records: &[serde_json::Value]) -> Result<(), ArtifactError> {
        use std::io::Write;
        let io = |e: std::io::Error| ArtifactError::Generator(e.to_string());
        let path = self.base_dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let append = self.written.contains(&path);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&path)
            .map_err(io)?;
        let mut writer = std::io::BufWriter::new(file);
        for record in records {
            serde_json::to_writer(&mut writer, record)?;
            writeln!(writer).map_err(io)?;
        }
        writer.flush().map_err(io)?;
        self.written.insert(path);
        Ok(())
    }
}

impl ArtifactStorage for JsonlArtifactStorage {
    fn persist(&mut self, artifacts: Vec<Artifact>) -> Result<(), ArtifactError> {
        self.buffer.extend(artifacts);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), ArtifactError> {
        let mut files: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
        for artifact in &self.buffer {
            files
                .entry(Self::file_for(artifact))
                .or_default()
                .push(Self::to_json(artifact)?);
        }
        for (rel, records) in &files {
            self.write(rel, records)?;
        }
        self.buffer.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::artifact::DataArtifact;
    use tempfile::TempDir;

    fn record(kind: &str) -> Artifact {
        Artifact::StructuredData(DataArtifact {
            record_type: kind.into(),
            source_event_id: "e1".into(),
            data: serde_json::json!({}),
            timestamp: chrono::DateTime::UNIX_EPOCH,
        })
    }

    fn lines(path: std::path::PathBuf) -> usize {
        std::fs::read_to_string(path).unwrap().lines().count()
    }

    #[test]
    fn flush_empty_writes_nothing() {
        let dir = TempDir::new().unwrap();
        let mut storage = JsonlArtifactStorage::new(dir.path());
        storage.flush().unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn later_flushes_append_but_a_new_run_replaces() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("structured/ticket.jsonl");

        let mut storage = JsonlArtifactStorage::new(dir.path());
        storage
            .persist(vec![record("ticket"), record("ticket")])
            .unwrap();
        storage.flush().unwrap();
        storage.persist(vec![record("ticket")]).unwrap();
        storage.flush().unwrap();
        assert_eq!(lines(file.clone()), 3);

        let mut next_run = JsonlArtifactStorage::new(dir.path());
        next_run.persist(vec![record("ticket")]).unwrap();
        next_run.flush().unwrap();
        assert_eq!(lines(file), 1);
    }

    #[test]
    fn failed_write_keeps_the_buffer() {
        let dir = TempDir::new().unwrap();
        // A file where the "structured" directory should go makes the write fail.
        std::fs::write(dir.path().join("structured"), "").unwrap();
        let mut storage = JsonlArtifactStorage::new(dir.path());
        storage.persist(vec![record("ticket")]).unwrap();
        assert!(storage.flush().is_err());

        std::fs::remove_file(dir.path().join("structured")).unwrap();
        storage.flush().unwrap();
        assert_eq!(lines(dir.path().join("structured/ticket.jsonl")), 1);
    }
}
