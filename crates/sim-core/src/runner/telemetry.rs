//! Telemetry integration for SimulationRunner.
//!
//! This module provides hooks that integrate telemetry generation with the
//! simulation lifecycle. Events are converted to CloudEvents-compatible JSON
//! and processed through the telemetry collector.
//!
//! # Architecture
//!
//! The integration uses the existing `SimulationHooks` trait to intercept
//! simulation events and convert them to telemetry records:
//!
//! 1. Simulation events (agents ticked, resources allocated, etc.)
//! 2. Convert to CloudEvents JSON format
//! 3. Process through TelemetryCollector
//! 4. Generate logs, traces, and metrics
//! 5. Export to OTLP, JSON, or Prometheus
//!
//! # Performance
//!
//! Telemetry generation is designed to have minimal impact on simulation
//! performance. Overhead should be <5% of total runtime. The collector
//! buffers records in memory for batch export.
//!
//! # Example
//!
//! ```rust,ignore
//! use sim_core::{SimulationRunner, SimulationConfig};
//! use sim_core::runner::telemetry::SimulationTelemetry;
//!
//! let config = SimulationConfig::default();
//! let mut runner = SimulationRunner::new(config);
//!
//! // Attach telemetry collection
//! let telemetry = SimulationTelemetry::new();
//! runner.set_hooks(Box::new(telemetry));
//!
//! // Run simulation with telemetry
//! runner.run_for_steps(100)?;
//!
//! // Extract and export telemetry
//! let hooks = runner.hooks_mut();
//! let records = hooks.flush_telemetry();
//! ```

use crate::context::SimulationContext;
use crate::population::AgentTickResult;
use crate::resources::Assignment;
use crate::workflows::WorkflowStepEvent;
use crate::{Error, Result};

use serde_json::json;
use sim_telemetry::{
    EventToLogGenerator, EventToMetricGenerator, EventToTraceGenerator, TelemetryCollector,
    TelemetryRecord,
};

/// Telemetry hooks for SimulationRunner.
///
/// This trait extends `SimulationHooks` with methods for flushing collected
/// telemetry data. Implementations should convert simulation events to
/// CloudEvents JSON and process them through generators.
pub trait TelemetryHooks: super::SimulationHooks {
    /// Flush all collected telemetry records.
    ///
    /// This drains the internal buffer and returns all records collected
    /// since the last flush.
    fn flush_telemetry(&mut self) -> Vec<TelemetryRecord>;

    /// Get the current count of collected records without flushing.
    fn telemetry_count(&self) -> usize;
}

/// Standard telemetry implementation for simulations.
///
/// Collects logs, traces, and metrics from simulation events using the
/// standard telemetry generators.
///
/// # Thread Safety
///
/// This struct is Send + Sync and can be used across threads.
///
/// # Example
///
/// ```rust,ignore
/// let mut telemetry = SimulationTelemetry::new();
///
/// // Attach to runner
/// runner.set_hooks(Box::new(telemetry));
///
/// // Run simulation
/// runner.run_for_steps(100)?;
///
/// // Get telemetry
/// let records = telemetry.flush_telemetry();
/// println!("Collected {} telemetry records", records.len());
/// ```
pub struct SimulationTelemetry {
    collector: TelemetryCollector,
    event_counter: u64,
}

impl SimulationTelemetry {
    /// Create a new telemetry collector with standard generators.
    ///
    /// This initializes the collector with:
    /// - EventToLogGenerator (for logs)
    /// - EventToTraceGenerator (for distributed traces)
    /// - EventToMetricGenerator (for metrics)
    pub fn new() -> Self {
        let mut collector = TelemetryCollector::new();
        collector.add_generator(Box::new(EventToLogGenerator::new()));
        collector.add_generator(Box::new(EventToTraceGenerator::new()));
        collector.add_generator(Box::new(EventToMetricGenerator::new()));

        Self {
            collector,
            event_counter: 0,
        }
    }

    /// Create a collector with custom generators.
    pub fn with_collector(collector: TelemetryCollector) -> Self {
        Self {
            collector,
            event_counter: 0,
        }
    }

    /// Process an event through the telemetry collectors.
    fn process_event(&mut self, event: serde_json::Value) -> Result<()> {
        self.collector
            .process_event(&event)
            .map_err(|e| Error::InvalidState(format!("Telemetry error: {}", e)))?;
        self.event_counter += 1;
        Ok(())
    }

    /// Generate a unique event ID.
    fn next_event_id(&mut self) -> String {
        let id = self.event_counter;
        self.event_counter += 1;
        format!("sim-evt-{:08}", id)
    }
}

impl Default for SimulationTelemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl super::SimulationHooks for SimulationTelemetry {
    fn on_start(&mut self, ctx: &SimulationContext) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner",
            "type": "com.simulation.started",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "step": ctx.step(),
                "config": {
                    "time_step_seconds": ctx.config().time_step.num_seconds(),
                }
            }
        });

        self.process_event(event)
    }

    fn on_before_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner",
            "type": "com.simulation.tick_started",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "step": ctx.step(),
                "simulation_time": ctx.current_time().to_rfc3339(),
            }
        });

        self.process_event(event)
    }

    fn on_after_tick(&mut self, ctx: &SimulationContext) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner",
            "type": "com.simulation.tick_completed",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "step": ctx.step(),
                "simulation_time": ctx.current_time().to_rfc3339(),
            }
        });

        self.process_event(event)
    }

    fn on_end(&mut self, ctx: &SimulationContext) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner",
            "type": "com.simulation.completed",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "total_steps": ctx.step(),
                "final_time": ctx.current_time().to_rfc3339(),
            }
        });

        self.process_event(event)
    }

    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner:population",
            "type": "com.simulation.agents_ticked",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "step": ctx.step(),
                "agent_count": results.len(),
                "simulation_time": ctx.current_time().to_rfc3339(),
            }
        });

        self.process_event(event)
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment],
    ) -> Result<()> {
        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner:resources",
            "type": "com.simulation.resources_allocated",
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": {
                "step": ctx.step(),
                "assignment_count": assignments.len(),
                "simulation_time": ctx.current_time().to_rfc3339(),
            }
        });

        self.process_event(event)
    }

    fn on_workflow_step_completed(
        &mut self,
        ctx: &SimulationContext,
        workflow_event: &WorkflowStepEvent,
    ) -> Result<()> {
        let (event_type, event_data) = match workflow_event {
            WorkflowStepEvent::StepCompleted {
                instance_id,
                step_index,
                completion_time,
            } => (
                "com.simulation.workflow_step_completed",
                json!({
                    "instance_id": instance_id.to_string(),
                    "step_index": step_index,
                    "completion_time": completion_time.to_rfc3339(),
                }),
            ),
            WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                workflow_id,
                completion_time,
            } => (
                "com.simulation.workflow_completed",
                json!({
                    "instance_id": instance_id.to_string(),
                    "workflow_id": workflow_id.to_string(),
                    "completion_time": completion_time.to_rfc3339(),
                }),
            ),
        };

        let event = json!({
            "specversion": "1.0",
            "id": self.next_event_id(),
            "source": "urn:sim:runner:workflows",
            "type": event_type,
            "time": ctx.current_time().to_rfc3339(),
            "datacontenttype": "application/json",
            "data": event_data
        });

        self.process_event(event)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl TelemetryHooks for SimulationTelemetry {
    fn flush_telemetry(&mut self) -> Vec<TelemetryRecord> {
        self.collector.flush()
    }

    fn telemetry_count(&self) -> usize {
        self.collector.record_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::SimulationConfig;
    use crate::runner::SimulationHooks;
    use crate::utils::idents::{TaskId, WorkflowInstanceId};

    #[test]
    fn test_telemetry_creation() {
        let telemetry = SimulationTelemetry::new();
        assert_eq!(telemetry.telemetry_count(), 0);
    }

    #[test]
    fn test_on_start_hook() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        telemetry.on_start(&ctx).unwrap();
        assert!(telemetry.telemetry_count() > 0);
    }

    #[test]
    fn test_on_agents_ticked() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let results = vec![AgentTickResult::default(), AgentTickResult::default()];
        telemetry.on_agents_ticked(&ctx, &results).unwrap();

        assert!(telemetry.telemetry_count() > 0);
    }

    #[test]
    fn test_on_resources_allocated() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let assignments = vec![
            Assignment {
                task_id: TaskId::new(),
                resource_id: crate::utils::idents::ResourceId::new(),
            },
            Assignment {
                task_id: TaskId::new(),
                resource_id: crate::utils::idents::ResourceId::new(),
            },
        ];

        telemetry
            .on_resources_allocated(&ctx, &assignments)
            .unwrap();
        assert!(telemetry.telemetry_count() > 0);
    }

    #[test]
    fn test_on_workflow_step_completed() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        let event = WorkflowStepEvent::StepCompleted {
            instance_id: WorkflowInstanceId::new(),
            step_index: 0,
            completion_time: ctx.current_time(),
        };

        telemetry.on_workflow_step_completed(&ctx, &event).unwrap();
        assert!(telemetry.telemetry_count() > 0);
    }

    #[test]
    fn test_flush_telemetry() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        telemetry.on_start(&ctx).unwrap();
        telemetry.on_before_tick(&ctx).unwrap();
        telemetry.on_after_tick(&ctx).unwrap();

        let count_before = telemetry.telemetry_count();
        assert!(count_before > 0);

        let records = telemetry.flush_telemetry();
        assert_eq!(records.len(), count_before);
        assert_eq!(telemetry.telemetry_count(), 0);
    }

    #[test]
    fn test_event_id_generation() {
        let mut telemetry = SimulationTelemetry::new();

        let id1 = telemetry.next_event_id();
        let id2 = telemetry.next_event_id();
        let id3 = telemetry.next_event_id();

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert!(id1.starts_with("sim-evt-"));
    }

    #[test]
    fn test_full_simulation_lifecycle() {
        let mut telemetry = SimulationTelemetry::new();
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        // Simulate a complete lifecycle
        telemetry.on_start(&ctx).unwrap();
        telemetry.on_before_tick(&ctx).unwrap();
        telemetry
            .on_agents_ticked(&ctx, &[AgentTickResult::default()])
            .unwrap();
        telemetry.on_resources_allocated(&ctx, &Vec::new()).unwrap();
        telemetry.on_after_tick(&ctx).unwrap();
        telemetry.on_end(&ctx).unwrap();

        let records = telemetry.flush_telemetry();
        assert!(records.len() >= 6); // At least one record per event
    }
}
