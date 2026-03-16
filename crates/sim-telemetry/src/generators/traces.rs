//! Trace generator for converting events to distributed trace spans.
//!
//! This module provides `EventToTraceGenerator` which transforms CloudEvents
//! into OpenTelemetry-compatible trace spans with automatic trace correlation
//! and parent-child relationships.
//!
//! # Correlation Strategy
//!
//! The generator implements intelligent trace correlation:
//!
//! - **Same Source Correlation**: Events from the same source share a trace_id
//! - **Parent-Child Relationships**: Sequential events from a source form a span hierarchy
//! - **Workflow Support**: Workflow steps are automatically linked in execution order
//! - **Agent Isolation**: Different agents get independent trace trees
//!
//! # Example
//!
//! ```rust
//! use sim_telemetry::generators::EventToTraceGenerator;
//! use sim_telemetry::core::TelemetryGenerator;
//! use serde_json::json;
//!
//! let mut generator = EventToTraceGenerator::new();
//!
//! // First event creates a new trace
//! let event1 = json!({
//!     "id": "evt-123",
//!     "type": "com.simulation.agent_created",
//!     "source": "urn:sim:agent:agent-001",
//!     "time": "2026-03-16T09:17:46Z",
//!     "data": {"agent_id": "agent-001"}
//! });
//!
//! let records1 = generator.generate(&event1).unwrap();
//! // Creates root span with no parent
//!
//! // Second event from same source creates child span
//! let event2 = json!({
//!     "id": "evt-124",
//!     "type": "com.simulation.agent_ticked",
//!     "source": "urn:sim:agent:agent-001",
//!     "time": "2026-03-16T09:17:47Z",
//!     "data": {"agent_id": "agent-001", "tick": 1}
//! });
//!
//! let records2 = generator.generate(&event2).unwrap();
//! // Creates child span linked to first span
//! ```

use crate::core::{TelemetryError, TelemetryGenerator, TelemetryRecord};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// Generator that converts events into trace spans with correlation.
///
/// This generator maintains trace context across events from the same source,
/// automatically creating parent-child relationships between sequential operations.
///
/// # Trace Context Management
///
/// The generator tracks:
/// - Active trace IDs per event source
/// - Current span IDs for correlation
/// - Automatic cleanup of stale contexts
///
/// # Examples
///
/// ```rust
/// use sim_telemetry::generators::EventToTraceGenerator;
///
/// let mut generator = EventToTraceGenerator::new();
/// // Generator is ready to process events
/// ```
#[derive(Debug, Default)]
pub struct EventToTraceGenerator {
    /// Active trace contexts indexed by event source
    trace_contexts: HashMap<String, TraceContext>,
}

/// Context for tracking trace state across related events.
#[derive(Debug, Clone)]
struct TraceContext {
    /// Unique identifier for the entire trace
    trace_id: String,
    /// Most recent span ID in this trace
    current_span_id: String,
}

impl EventToTraceGenerator {
    /// Create a new trace generator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a trace ID for the given event source.
    ///
    /// Each unique source gets its own trace tree. This ensures that
    /// different agents or workflow instances have isolated traces.
    fn get_or_create_trace_id(&mut self, source: &str) -> String {
        self.trace_contexts
            .entry(source.to_string())
            .or_insert_with(|| TraceContext {
                trace_id: Uuid::new_v4().to_string(),
                current_span_id: String::new(),
            })
            .trace_id
            .clone()
    }

    /// Determine the parent span ID based on event correlation.
    ///
    /// # Correlation Rules
    ///
    /// - **Workflow/Step Events**: Use previous span from same source
    /// - **Agent Lifecycle Events**: Create events have no parent, others link to current
    /// - **Resource Events**: Could correlate with requesting agent (future enhancement)
    ///
    /// # Arguments
    ///
    /// * `event_type` - The CloudEvent type field
    /// * `source` - The CloudEvent source field
    ///
    /// # Returns
    ///
    /// `Some(span_id)` if there's a parent span to link to, `None` for root spans
    fn get_parent_span_id(&self, event_type: &str, source: &str) -> Option<String> {
        // For workflow steps or continuation events, link to previous span
        if event_type.contains("workflow")
            || event_type.contains("step")
            || event_type.contains("ticked")
            || event_type.contains("updated")
        {
            self.trace_contexts.get(source).and_then(|ctx| {
                if !ctx.current_span_id.is_empty() {
                    Some(ctx.current_span_id.clone())
                } else {
                    None
                }
            })
        } else if event_type.contains("created") || event_type.contains("initialized") {
            // Creation events are always root spans
            None
        } else {
            // For other events, try to link to current span if it exists
            self.trace_contexts.get(source).and_then(|ctx| {
                if !ctx.current_span_id.is_empty() {
                    Some(ctx.current_span_id.clone())
                } else {
                    None
                }
            })
        }
    }

    /// Update the span context after creating a new span.
    ///
    /// This ensures the next event from the same source can reference
    /// this span as its parent.
    fn update_span_context(&mut self, source: &str, span_id: String) {
        if let Some(ctx) = self.trace_contexts.get_mut(source) {
            ctx.current_span_id = span_id;
        }
    }

    /// Extract span attributes from the event data.
    ///
    /// Attributes are key-value pairs that provide additional context
    /// about the span operation.
    fn extract_attributes(&self, event: &Value) -> HashMap<String, String> {
        let mut attributes = HashMap::new();

        // Add event-level attributes
        if let Some(id) = event.get("id").and_then(|v| v.as_str()) {
            attributes.insert("event.id".to_string(), id.to_string());
        }

        // Extract data field attributes
        if let Some(data) = event.get("data").and_then(|v| v.as_object()) {
            for (key, value) in data {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Object(_) | Value::Array(_) => {
                        serde_json::to_string(value).unwrap_or_default()
                    }
                    Value::Null => "null".to_string(),
                };
                attributes.insert(format!("data.{}", key), value_str);
            }
        }

        attributes
    }

    /// Compute a synthetic duration for the span based on event type.
    ///
    /// In a real system, you'd track actual start/end times. For simulation
    /// telemetry, we generate reasonable synthetic durations.
    fn compute_span_duration(&self, event_type: &str) -> Duration {
        if event_type.contains("created") || event_type.contains("destroyed") {
            Duration::milliseconds(10) // Fast lifecycle events
        } else if event_type.contains("workflow") {
            Duration::milliseconds(500) // Workflows take longer
        } else if event_type.contains("ticked") {
            Duration::milliseconds(50) // Regular tick processing
        } else {
            Duration::milliseconds(100) // Default duration
        }
    }

    /// Validate that a trace ID is a valid UUID format.
    #[cfg(test)]
    fn validate_trace_id(&self, trace_id: &str) -> bool {
        Uuid::parse_str(trace_id).is_ok()
    }

    /// Validate that a span ID is a valid UUID format.
    #[cfg(test)]
    fn validate_span_id(&self, span_id: &str) -> bool {
        Uuid::parse_str(span_id).is_ok()
    }
}

impl TelemetryGenerator for EventToTraceGenerator {
    fn generate(&mut self, event: &Value) -> Result<Vec<TelemetryRecord>, TelemetryError> {
        // Validate event structure
        if !event.is_object() {
            return Err(TelemetryError::InvalidEvent(
                "Event must be a JSON object".to_string(),
            ));
        }

        // Extract required event metadata
        let event_type = event
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TelemetryError::InvalidEvent("Missing type field".into()))?;

        let source = event
            .get("source")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TelemetryError::InvalidEvent("Missing source field".into()))?;

        let time_str = event
            .get("time")
            .and_then(|v| v.as_str())
            .ok_or_else(|| TelemetryError::InvalidEvent("Missing time field".into()))?;

        // Parse start time
        let start_time = DateTime::parse_from_rfc3339(time_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        // Get or create trace ID for this source
        let trace_id = self.get_or_create_trace_id(source);

        // Create new span ID
        let span_id = Uuid::new_v4().to_string();

        // Determine parent span based on correlation rules
        let parent_span_id = self.get_parent_span_id(event_type, source);

        // Extract attributes
        let mut attributes = self.extract_attributes(event);
        attributes.insert("event.type".to_string(), event_type.to_string());
        attributes.insert("event.source".to_string(), source.to_string());

        // Compute span duration
        let duration = self.compute_span_duration(event_type);
        let end_time = start_time + duration;

        // Validate timestamps are ordered
        if end_time <= start_time {
            return Err(TelemetryError::InvalidEvent(
                "Span end time must be after start time".into(),
            ));
        }

        // Create trace span
        let trace = TelemetryRecord::Trace {
            trace_id: trace_id.clone(),
            span_id: span_id.clone(),
            parent_span_id,
            name: event_type.to_string(),
            start_time,
            end_time,
            attributes,
        };

        // Update context for next span in this trace
        self.update_span_context(source, span_id);

        Ok(vec![trace])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_event_generates_span() {
        let mut generator = EventToTraceGenerator::new();
        let event = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "agent_id": "agent-001"
            }
        });

        let records = generator.generate(&event).unwrap();
        assert_eq!(records.len(), 1);

        match &records[0] {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                name,
                start_time,
                end_time,
                attributes,
            } => {
                // Validate trace and span IDs are UUIDs
                assert!(generator.validate_trace_id(trace_id));
                assert!(generator.validate_span_id(span_id));

                // Root span should have no parent
                assert!(parent_span_id.is_none());

                // Validate span name
                assert_eq!(name, "com.simulation.agent_created");

                // Validate timestamps are ordered
                assert!(end_time > start_time);

                // Validate attributes
                assert_eq!(
                    attributes.get("event.type").unwrap(),
                    "com.simulation.agent_created"
                );
                assert_eq!(attributes.get("data.agent_id").unwrap(), "agent-001");
            }
            _ => panic!("Expected Trace record"),
        }
    }

    #[test]
    fn test_correlated_events_share_trace_id() {
        let mut generator = EventToTraceGenerator::new();

        // First event
        let event1 = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "agent-001"}
        });

        let records1 = generator.generate(&event1).unwrap();
        let trace_id1 = match &records1[0] {
            TelemetryRecord::Trace { trace_id, .. } => trace_id.clone(),
            _ => panic!("Expected Trace record"),
        };

        // Second event from same source
        let event2 = json!({
            "id": "evt-002",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:01Z",
            "data": {"agent_id": "agent-001", "tick": 1}
        });

        let records2 = generator.generate(&event2).unwrap();
        let trace_id2 = match &records2[0] {
            TelemetryRecord::Trace { trace_id, .. } => trace_id.clone(),
            _ => panic!("Expected Trace record"),
        };

        // Both events should share the same trace_id
        assert_eq!(trace_id1, trace_id2);
    }

    #[test]
    fn test_parent_child_relationship_for_workflow_steps() {
        let mut generator = EventToTraceGenerator::new();

        // Step 1: Create agent (root span)
        let event1 = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "agent-001"}
        });

        let records1 = generator.generate(&event1).unwrap();
        let (trace_id1, span_id1, parent_id1) = match &records1[0] {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                ..
            } => (trace_id.clone(), span_id.clone(), parent_span_id.clone()),
            _ => panic!("Expected Trace record"),
        };

        assert!(parent_id1.is_none(), "First span should have no parent");

        // Step 2: First tick (child of creation)
        let event2 = json!({
            "id": "evt-002",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:01Z",
            "data": {"agent_id": "agent-001", "tick": 1}
        });

        let records2 = generator.generate(&event2).unwrap();
        let (trace_id2, span_id2, parent_id2) = match &records2[0] {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                ..
            } => (trace_id.clone(), span_id.clone(), parent_span_id.clone()),
            _ => panic!("Expected Trace record"),
        };

        // Validate trace correlation
        assert_eq!(trace_id1, trace_id2, "Spans should share trace_id");

        // Validate parent-child relationship
        assert_eq!(
            parent_id2.unwrap(),
            span_id1,
            "Second span's parent should be first span"
        );

        // Step 3: Second tick (child of first tick)
        let event3 = json!({
            "id": "evt-003",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:02Z",
            "data": {"agent_id": "agent-001", "tick": 2}
        });

        let records3 = generator.generate(&event3).unwrap();
        let (trace_id3, _span_id3, parent_id3) = match &records3[0] {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                ..
            } => (trace_id.clone(), span_id.clone(), parent_span_id.clone()),
            _ => panic!("Expected Trace record"),
        };

        // Validate continued correlation
        assert_eq!(trace_id1, trace_id3, "All spans should share trace_id");

        // Third span should be child of second span
        assert_eq!(
            parent_id3.unwrap(),
            span_id2,
            "Third span's parent should be second span"
        );
    }

    #[test]
    fn test_multiple_agents_get_different_trace_ids() {
        let mut generator = EventToTraceGenerator::new();

        // Agent 1 event
        let event1 = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "agent-001"}
        });

        let records1 = generator.generate(&event1).unwrap();
        let trace_id1 = match &records1[0] {
            TelemetryRecord::Trace { trace_id, .. } => trace_id.clone(),
            _ => panic!("Expected Trace record"),
        };

        // Agent 2 event
        let event2 = json!({
            "id": "evt-002",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-002",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "agent-002"}
        });

        let records2 = generator.generate(&event2).unwrap();
        let trace_id2 = match &records2[0] {
            TelemetryRecord::Trace { trace_id, .. } => trace_id.clone(),
            _ => panic!("Expected Trace record"),
        };

        // Different agents should have different trace_ids
        assert_ne!(
            trace_id1, trace_id2,
            "Different agents should have independent traces"
        );
    }

    #[test]
    fn test_missing_required_fields() {
        let mut generator = EventToTraceGenerator::new();

        // Missing type
        let event = json!({
            "id": "evt-001",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z"
        });

        let result = generator.generate(&event);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TelemetryError::InvalidEvent(_)
        ));

        // Missing source
        let event = json!({
            "id": "evt-001",
            "type": "test.event",
            "time": "2026-03-16T10:00:00Z"
        });

        let result = generator.generate(&event);
        assert!(result.is_err());

        // Missing time
        let event = json!({
            "id": "evt-001",
            "type": "test.event",
            "source": "urn:sim:agent:agent-001"
        });

        let result = generator.generate(&event);
        assert!(result.is_err());
    }

    #[test]
    fn test_timestamp_validation() {
        let mut generator = EventToTraceGenerator::new();

        // Valid RFC3339 timestamp
        let event = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T09:17:46.518945Z",
            "data": {}
        });

        let records = generator.generate(&event).unwrap();
        match &records[0] {
            TelemetryRecord::Trace {
                start_time,
                end_time,
                ..
            } => {
                assert!(end_time > start_time, "End time must be after start time");
            }
            _ => panic!("Expected Trace record"),
        }
    }

    #[test]
    fn test_attribute_extraction() {
        let mut generator = EventToTraceGenerator::new();

        let event = json!({
            "id": "evt-123",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {
                "agent_id": "agent-001",
                "tick": 42,
                "state": "active",
                "energy": 75.5,
                "position": {"x": 10, "y": 20}
            }
        });

        let records = generator.generate(&event).unwrap();
        match &records[0] {
            TelemetryRecord::Trace { attributes, .. } => {
                // Event-level attributes
                assert_eq!(attributes.get("event.id").unwrap(), "evt-123");
                assert_eq!(
                    attributes.get("event.type").unwrap(),
                    "com.simulation.agent_ticked"
                );
                assert_eq!(
                    attributes.get("event.source").unwrap(),
                    "urn:sim:agent:agent-001"
                );

                // Data attributes
                assert_eq!(attributes.get("data.agent_id").unwrap(), "agent-001");
                assert_eq!(attributes.get("data.tick").unwrap(), "42");
                assert_eq!(attributes.get("data.state").unwrap(), "active");
                assert_eq!(attributes.get("data.energy").unwrap(), "75.5");
                assert!(attributes.get("data.position").unwrap().contains("\"x\""));
            }
            _ => panic!("Expected Trace record"),
        }
    }

    #[test]
    fn test_span_duration_varies_by_type() {
        let mut generator = EventToTraceGenerator::new();

        // Created event (fast)
        let event1 = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let records1 = generator.generate(&event1).unwrap();
        let duration1 = match &records1[0] {
            TelemetryRecord::Trace {
                start_time,
                end_time,
                ..
            } => (*end_time - *start_time).num_milliseconds(),
            _ => panic!("Expected Trace record"),
        };

        // Workflow event (slower)
        let event2 = json!({
            "id": "evt-002",
            "type": "com.simulation.workflow_executed",
            "source": "urn:sim:workflow:wf-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {}
        });

        let records2 = generator.generate(&event2).unwrap();
        let duration2 = match &records2[0] {
            TelemetryRecord::Trace {
                start_time,
                end_time,
                ..
            } => (*end_time - *start_time).num_milliseconds(),
            _ => panic!("Expected Trace record"),
        };

        // Workflow events should have longer duration than creation events
        assert!(
            duration2 > duration1,
            "Workflow duration ({}) should be > creation duration ({})",
            duration2,
            duration1
        );
    }

    #[test]
    fn test_json_serialization() {
        let mut generator = EventToTraceGenerator::new();

        let event = json!({
            "id": "evt-001",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"tick": 1}
        });

        let records = generator.generate(&event).unwrap();
        let json_output = serde_json::to_string_pretty(&records[0]).unwrap();

        // Validate it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
        assert_eq!(parsed.get("type").and_then(|v| v.as_str()), Some("Trace"));
        assert!(parsed.get("trace_id").is_some());
        assert!(parsed.get("span_id").is_some());
        assert!(parsed.get("name").is_some());
        assert!(parsed.get("start_time").is_some());
        assert!(parsed.get("end_time").is_some());
        assert!(parsed.get("attributes").is_some());
    }

    #[test]
    fn test_integration_multi_step_workflow_span_tree() {
        let mut generator = EventToTraceGenerator::new();

        // Simulate a complete workflow: create -> tick -> tick -> destroy
        let events = vec![
            json!({
                "id": "evt-001",
                "type": "com.simulation.agent_created",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:00Z",
                "data": {"agent_id": "agent-001"}
            }),
            json!({
                "id": "evt-002",
                "type": "com.simulation.agent_ticked",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:01Z",
                "data": {"agent_id": "agent-001", "tick": 1}
            }),
            json!({
                "id": "evt-003",
                "type": "com.simulation.agent_ticked",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:02Z",
                "data": {"agent_id": "agent-001", "tick": 2}
            }),
            json!({
                "id": "evt-004",
                "type": "com.simulation.agent_destroyed",
                "source": "urn:sim:agent:agent-001",
                "time": "2026-03-16T10:00:03Z",
                "data": {"agent_id": "agent-001"}
            }),
        ];

        let mut all_spans = Vec::new();
        for event in events {
            let records = generator.generate(&event).unwrap();
            all_spans.push(records[0].clone());
        }

        // Verify we generated 4 spans
        assert_eq!(all_spans.len(), 4);

        // Extract span information for verification
        let mut span_info = Vec::new();
        for span in &all_spans {
            match span {
                TelemetryRecord::Trace {
                    trace_id,
                    span_id,
                    parent_span_id,
                    name,
                    ..
                } => {
                    span_info.push((
                        trace_id.clone(),
                        span_id.clone(),
                        parent_span_id.clone(),
                        name.clone(),
                    ));
                }
                _ => panic!("Expected Trace record"),
            }
        }

        // All spans should share the same trace_id
        let trace_id = &span_info[0].0;
        for (tid, _, _, _) in &span_info {
            assert_eq!(tid, trace_id, "All spans should share the same trace_id");
        }

        // Verify span hierarchy
        // Span 0 (created): root span, no parent
        assert!(span_info[0].2.is_none(), "First span should have no parent");

        // Span 1 (tick 1): parent is span 0
        assert_eq!(
            span_info[1].2.as_ref().unwrap(),
            &span_info[0].1,
            "Second span's parent should be first span"
        );

        // Span 2 (tick 2): parent is span 1
        assert_eq!(
            span_info[2].2.as_ref().unwrap(),
            &span_info[1].1,
            "Third span's parent should be second span"
        );

        // Span 3 (destroyed): parent is span 2
        assert_eq!(
            span_info[3].2.as_ref().unwrap(),
            &span_info[2].1,
            "Fourth span's parent should be third span"
        );

        // Print span tree for visualization
        println!("\n=== Span Tree Visualization ===");
        println!("Trace ID: {}", trace_id);
        println!("\nSpan Hierarchy:");
        for (i, (_, span_id, parent_id, name)) in span_info.iter().enumerate() {
            let indent = if parent_id.is_none() {
                String::new()
            } else {
                "  ".repeat(i)
            };
            let parent_display = parent_id
                .as_ref()
                .map(|p| format!(" (parent: {})", &p[..8]))
                .unwrap_or_default();
            println!("{}{} - {}{}", indent, &span_id[..8], name, parent_display);
        }
        println!("================================\n");
    }

    #[test]
    fn test_workflow_event_correlation() {
        let mut generator = EventToTraceGenerator::new();

        // Workflow start
        let event1 = json!({
            "id": "evt-001",
            "type": "com.simulation.workflow_started",
            "source": "urn:sim:workflow:wf-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"workflow_id": "wf-001"}
        });

        let records1 = generator.generate(&event1).unwrap();
        let (trace_id1, span_id1) = match &records1[0] {
            TelemetryRecord::Trace {
                trace_id, span_id, ..
            } => (trace_id.clone(), span_id.clone()),
            _ => panic!("Expected Trace record"),
        };

        // Workflow step
        let event2 = json!({
            "id": "evt-002",
            "type": "com.simulation.workflow_step_executed",
            "source": "urn:sim:workflow:wf-001",
            "time": "2026-03-16T10:00:01Z",
            "data": {"workflow_id": "wf-001", "step": "step-1"}
        });

        let records2 = generator.generate(&event2).unwrap();
        let (trace_id2, _span_id2, parent_id2) = match &records2[0] {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                ..
            } => (trace_id.clone(), span_id.clone(), parent_span_id.clone()),
            _ => panic!("Expected Trace record"),
        };

        // Same trace
        assert_eq!(trace_id1, trace_id2);

        // Step should be child of start
        assert_eq!(parent_id2.unwrap(), span_id1);
    }

    #[test]
    fn test_invalid_event_types() {
        let mut generator = EventToTraceGenerator::new();

        // Not an object
        let result = generator.generate(&json!("not an object"));
        assert!(result.is_err());

        // Array
        let result = generator.generate(&json!([1, 2, 3]));
        assert!(result.is_err());

        // Null
        let result = generator.generate(&json!(null));
        assert!(result.is_err());
    }
}
