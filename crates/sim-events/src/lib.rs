//! Event Infrastructure for Agent-Based Simulations
//!
//! `sim-events` provides CloudEvents-compliant event emission and persistence.
//! Events are the primary way to record state changes in the simulation.
//!
//! # CloudEvents 1.0
//!
//! All events follow the CloudEvents specification:
//! - `id` - Unique event identifier (UUID v7)
//! - `source` - URI identifying the event source
//! - `specversion` - CloudEvents version ("1.0")
//! - `type` - Event type (e.g., "com.example.order.created")
//! - `time` - When the event occurred
//! - `data` - Event payload (domain-specific)
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_events::{Event, EventPayload, EventDispatcher};
//!
//! // Define custom event payload
//! struct OrderCreated {
//!     order_id: String,
//!     items: Vec<String>,
//! }
//!
//! impl EventPayload for OrderCreated {
//!     fn event_type(&self) -> &str {
//!         "com.example.order.created"
//!     }
//!
//!     fn source(&self) -> String {
//!         format!("/orders/{}", self.order_id)
//!     }
//! }
//!
//! // Emit events
//! let mut dispatcher = EventDispatcher::new();
//! dispatcher.emit(Event::new(Box::new(payload)))?;
//! ```

pub mod core;
pub mod dispatcher;
pub mod storage;

// Re-export commonly used types
pub use core::{Event, EventPayload, EventRecord};
pub use dispatcher::{
    ChannelSenderProcessor, EventDispatcher, EventProcessor, ParquetEventProcessor,
};
pub use storage::ParquetEventWriter;

/// Result type used throughout the crate
pub type Result<T> = std::result::Result<T, Error>;

/// Error types for the event infrastructure
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("sim-core error: {0}")]
    Core(#[from] sim_core::Error),

    #[error("Arrow error: {0}")]
    Arrow(#[from] arrow::error::ArrowError),

    #[error("Parquet error: {0}")]
    Parquet(#[from] parquet::errors::ParquetError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Event error: {0}")]
    Event(String),

    #[error("{0}")]
    Custom(String),
}
