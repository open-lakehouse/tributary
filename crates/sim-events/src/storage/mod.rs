//! Event storage backends (Parquet, Delta Lake, etc.)

use crate::core::EventRecord;
use arrow::array::{ArrayRef, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, Encoding};
use parquet::file::properties::WriterProperties;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

/// Writer for persisting events to Parquet files
///
/// Converts CloudEvents to Arrow schema and writes to Parquet format.
///
/// # Schema
///
/// The Parquet schema follows CloudEvents 1.0:
/// - id: String (unique event identifier)
/// - source: String (event source URI)
/// - spec_version: String (CloudEvents version)
/// - event_type: String (event type)
/// - time: String (ISO 8601 timestamp)
/// - subject: String (optional)
/// - data_content_type: String (optional)
/// - data: String (JSON payload)
///
/// # Example
///
/// ```rust,ignore
/// use sim_events::ParquetEventWriter;
///
/// let mut writer = ParquetEventWriter::new("events.parquet")?;
/// writer.write_events(&records)?;
/// writer.close()?;
/// ```
pub struct ParquetEventWriter {
    writer: ArrowWriter<File>,
    schema: Arc<Schema>,
}

impl ParquetEventWriter {
    /// Create a new Parquet writer
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the Parquet file will be written
    ///
    /// # Returns
    ///
    /// A new ParquetEventWriter instance
    pub fn new(path: PathBuf) -> crate::Result<Self> {
        let schema = Self::create_schema();
        let file = File::create(&path)?;

        // Configure Parquet writer properties
        let props = WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .set_encoding(Encoding::PLAIN)
            .build();

        let writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

        Ok(Self { writer, schema })
    }

    /// Create the Arrow schema for CloudEvents
    fn create_schema() -> Arc<Schema> {
        Arc::new(Schema::new(vec![
            Field::new("id", DataType::Utf8, false),
            Field::new("source", DataType::Utf8, false),
            Field::new("spec_version", DataType::Utf8, false),
            Field::new("event_type", DataType::Utf8, false),
            Field::new("time", DataType::Utf8, false),
            Field::new("subject", DataType::Utf8, true),
            Field::new("data_content_type", DataType::Utf8, true),
            Field::new("data", DataType::Utf8, false),
        ]))
    }

    /// Write a batch of event records
    ///
    /// # Arguments
    ///
    /// * `records` - Slice of EventRecord to write
    ///
    /// # Returns
    ///
    /// Number of records written
    pub fn write_events(&mut self, records: &[EventRecord]) -> crate::Result<usize> {
        if records.is_empty() {
            return Ok(0);
        }

        let batch = Self::records_to_batch(records, &self.schema)?;
        self.writer.write(&batch)?;

        Ok(records.len())
    }

    /// Convert EventRecords to Arrow RecordBatch
    fn records_to_batch(
        records: &[EventRecord],
        schema: &Arc<Schema>,
    ) -> crate::Result<RecordBatch> {
        let mut ids = Vec::with_capacity(records.len());
        let mut sources = Vec::with_capacity(records.len());
        let mut spec_versions = Vec::with_capacity(records.len());
        let mut event_types = Vec::with_capacity(records.len());
        let mut times = Vec::with_capacity(records.len());
        let mut subjects = Vec::with_capacity(records.len());
        let mut data_content_types = Vec::with_capacity(records.len());
        let mut data = Vec::with_capacity(records.len());

        for record in records {
            ids.push(record.id.as_str());
            sources.push(record.source.as_str());
            spec_versions.push(record.spec_version.as_str());
            event_types.push(record.event_type.as_str());
            times.push(record.time.as_str());
            subjects.push(record.subject.as_deref());
            data_content_types.push(record.data_content_type.as_deref());
            data.push(record.data.to_string());
        }

        let columns: Vec<ArrayRef> = vec![
            Arc::new(StringArray::from(ids)),
            Arc::new(StringArray::from(sources)),
            Arc::new(StringArray::from(spec_versions)),
            Arc::new(StringArray::from(event_types)),
            Arc::new(StringArray::from(times)),
            Arc::new(StringArray::from(subjects)),
            Arc::new(StringArray::from(data_content_types)),
            Arc::new(StringArray::from(
                data.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
        ];

        Ok(RecordBatch::try_new(schema.clone(), columns)?)
    }

    /// Flush and close the writer
    ///
    /// This must be called to ensure all data is written to disk.
    pub fn close(self) -> crate::Result<()> {
        self.writer.close()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Event, EventPayload};
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    #[derive(Debug)]
    struct TestPayload {
        value: i32,
    }

    impl EventPayload for TestPayload {
        fn event_type(&self) -> &str {
            "test.event"
        }

        fn source(&self) -> String {
            "/test".to_string()
        }

        fn to_json(&self) -> serde_json::Value {
            json!({ "value": self.value })
        }
    }

    #[test]
    fn test_parquet_write() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.parquet");

        let event1 = Event::new(Box::new(TestPayload { value: 1 }));
        let event2 = Event::new(Box::new(TestPayload { value: 2 }));

        let records = vec![event1.to_record(), event2.to_record()];

        let mut writer = ParquetEventWriter::new(file_path.clone()).unwrap();
        let count = writer.write_events(&records).unwrap();
        writer.close().unwrap();

        assert_eq!(count, 2);
        assert!(file_path.exists());

        // Verify file is not empty
        let metadata = fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_parquet_write_empty() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.parquet");

        let mut writer = ParquetEventWriter::new(file_path.clone()).unwrap();
        let count = writer.write_events(&[]).unwrap();
        writer.close().unwrap();

        assert_eq!(count, 0);
        assert!(file_path.exists());
    }

    #[test]
    fn test_parquet_schema() {
        let schema = ParquetEventWriter::create_schema();

        assert_eq!(schema.fields().len(), 8);
        assert_eq!(schema.field(0).name(), "id");
        assert_eq!(schema.field(1).name(), "source");
        assert_eq!(schema.field(2).name(), "spec_version");
        assert_eq!(schema.field(3).name(), "event_type");
        assert_eq!(schema.field(4).name(), "time");
        assert_eq!(schema.field(5).name(), "subject");
        assert_eq!(schema.field(6).name(), "data_content_type");
        assert_eq!(schema.field(7).name(), "data");

        // Check nullable fields
        assert!(!schema.field(0).is_nullable()); // id
        assert!(schema.field(5).is_nullable()); // subject
        assert!(schema.field(6).is_nullable()); // data_content_type
    }
}
