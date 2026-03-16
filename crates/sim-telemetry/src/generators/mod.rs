//! Telemetry generators for transforming events into observability data.
//!
//! This module contains implementations of the `TelemetryGenerator` trait
//! for different types of telemetry output.

pub mod logs;
pub mod traces;

pub use logs::EventToLogGenerator;
pub use traces::EventToTraceGenerator;
