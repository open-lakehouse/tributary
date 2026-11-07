//! Event storage backends (Parquet, Delta Lake, etc.)

use crate::core::EventRecord;
use arrow::array::{Array, ArrayRef, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use parquet::arrow::ArrowWriter;
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::{Compression, Encoding};
use parquet::file::properties::WriterProperties;
use std::fs::File;
use std::path::{Path, PathBuf};
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
/// let mut writer = ParquetEventWriter::new("events.parquet".into())?;
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
    /// Creates `path`, replacing any existing file. Call
    /// [`write_events`](Self::write_events) any number of times, then
    /// [`close`](Self::close) to finish the file.
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

/// Reader for loading events back from a Parquet file written by
/// [`ParquetEventWriter`].
///
/// Reconstructs [`EventRecord`]s from the columnar Parquet representation. The
/// Parquet/Arrow reader may surface `Utf8` columns as `Utf8View`, so every
/// string column is read through [`arrow::compute::cast`] to `Utf8` first.
pub struct ParquetEventReader;

impl ParquetEventReader {
    /// Read all event records from a Parquet file.
    pub fn read_file(path: impl AsRef<Path>) -> crate::Result<Vec<EventRecord>> {
        let file = File::open(path.as_ref())?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let reader = builder.build()?;

        let mut records = Vec::new();
        for batch in reader {
            let batch = batch?;
            Self::append_batch(&batch, &mut records)?;
        }
        Ok(records)
    }

    /// Read a single string column as `Vec<Option<String>>`, coercing `Utf8View`
    /// back to `Utf8` if the Parquet reader optimized it.
    fn string_column(batch: &RecordBatch, name: &str) -> crate::Result<Vec<Option<String>>> {
        let array = batch
            .column_by_name(name)
            .ok_or_else(|| crate::Error::Event(format!("missing column `{name}`")))?;
        let array = if matches!(array.data_type(), DataType::Utf8View) {
            arrow::compute::cast(array, &DataType::Utf8)?
        } else {
            array.clone()
        };
        let strings = array
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                crate::Error::Event(format!("column `{name}` is not a string column"))
            })?;
        Ok((0..strings.len())
            .map(|i| {
                if strings.is_null(i) {
                    None
                } else {
                    Some(strings.value(i).to_string())
                }
            })
            .collect())
    }

    fn append_batch(batch: &RecordBatch, out: &mut Vec<EventRecord>) -> crate::Result<()> {
        let ids = Self::string_column(batch, "id")?;
        let sources = Self::string_column(batch, "source")?;
        let spec_versions = Self::string_column(batch, "spec_version")?;
        let event_types = Self::string_column(batch, "event_type")?;
        let times = Self::string_column(batch, "time")?;
        let subjects = Self::string_column(batch, "subject")?;
        let data_content_types = Self::string_column(batch, "data_content_type")?;
        let data = Self::string_column(batch, "data")?;

        let req = |opt: Option<String>, col: &str| {
            opt.ok_or_else(|| crate::Error::Event(format!("unexpected null in `{col}`")))
        };

        for i in 0..batch.num_rows() {
            let data_str = req(data[i].clone(), "data")?;
            out.push(EventRecord {
                id: req(ids[i].clone(), "id")?,
                source: req(sources[i].clone(), "source")?,
                spec_version: req(spec_versions[i].clone(), "spec_version")?,
                event_type: req(event_types[i].clone(), "event_type")?,
                time: req(times[i].clone(), "time")?,
                subject: subjects[i].clone(),
                data_content_type: data_content_types[i].clone(),
                data: serde_json::from_str(&data_str)?,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Event, EventPayload};
    use chrono::DateTime;
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

        let event1 = Event::new(Box::new(TestPayload { value: 1 }), DateTime::UNIX_EPOCH);
        let event2 = Event::new(Box::new(TestPayload { value: 2 }), DateTime::UNIX_EPOCH);

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
    fn test_parquet_roundtrip_read() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("roundtrip.parquet");

        let records = vec![
            Event::new(Box::new(TestPayload { value: 1 }), DateTime::UNIX_EPOCH).to_record(),
            Event::new(Box::new(TestPayload { value: 2 }), DateTime::UNIX_EPOCH).to_record(),
            Event::new(Box::new(TestPayload { value: 3 }), DateTime::UNIX_EPOCH).to_record(),
        ];

        let mut writer = ParquetEventWriter::new(file_path.clone()).unwrap();
        writer.write_events(&records).unwrap();
        writer.close().unwrap();

        let read_back = ParquetEventReader::read_file(&file_path).unwrap();

        assert_eq!(read_back.len(), 3);
        for (orig, got) in records.iter().zip(read_back.iter()) {
            assert_eq!(orig.id, got.id);
            assert_eq!(orig.event_type, got.event_type);
            assert_eq!(orig.source, got.source);
            assert_eq!(orig.data, got.data);
        }
        // Spot-check the JSON payload survived the string roundtrip.
        assert_eq!(read_back[0].data, json!({ "value": 1 }));
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
