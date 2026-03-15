//! Simulation context and configuration
//!
//! This module provides the simulation runtime context including time management,
//! configuration, and catalogs.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Configuration for the simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    /// Name of this simulation
    pub name: String,
    /// Time step duration (how much time advances per step)
    pub time_step: Duration,
    /// Whether to enable detailed logging
    pub verbose: bool,
    /// Custom configuration values
    #[serde(default)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            name: "simulation".to_string(),
            time_step: Duration::seconds(60), // 1 minute default
            verbose: false,
            custom: serde_json::Map::new(),
        }
    }
}

/// Simulation context
///
/// Provides access to the current simulation time, configuration, and other runtime state.
#[derive(Debug, Clone)]
pub struct SimulationContext {
    /// Simulation start time
    start_time: DateTime<Utc>,
    /// Current simulation time
    current_time: DateTime<Utc>,
    /// Configuration
    config: SimulationConfig,
    /// Current step number
    step: usize,
}

impl SimulationContext {
    /// Create a new simulation context
    pub fn new(start_time: DateTime<Utc>, config: SimulationConfig) -> Self {
        Self {
            start_time,
            current_time: start_time,
            config,
            step: 0,
        }
    }

    /// Create a context starting from the current time
    pub fn from_now(config: SimulationConfig) -> Self {
        Self::new(Utc::now(), config)
    }

    /// Get the simulation start time
    pub fn start_time(&self) -> DateTime<Utc> {
        self.start_time
    }

    /// Get the current simulation time
    pub fn current_time(&self) -> DateTime<Utc> {
        self.current_time
    }

    /// Get the time step duration
    pub fn time_step(&self) -> Duration {
        self.config.time_step
    }

    /// Get the current step number
    pub fn step(&self) -> usize {
        self.step
    }

    /// Get the configuration
    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }

    /// Advance the simulation time by one step
    pub fn advance_time(&mut self) {
        self.current_time = self.current_time + self.config.time_step;
        self.step += 1;
    }

    /// Set the current time (for snapshot restoration)
    pub fn set_current_time(&mut self, time: DateTime<Utc>) {
        self.current_time = time;
    }

    /// Calculate elapsed time since start
    pub fn elapsed(&self) -> Duration {
        self.current_time - self.start_time
    }

    /// Check if verbose logging is enabled
    pub fn is_verbose(&self) -> bool {
        self.config.verbose
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let config = SimulationConfig::default();
        let ctx = SimulationContext::from_now(config);

        assert_eq!(ctx.step(), 0);
        assert_eq!(ctx.elapsed(), Duration::zero());
    }

    #[test]
    fn test_time_advancement() {
        let config = SimulationConfig::default();
        let mut ctx = SimulationContext::from_now(config);

        let initial_time = ctx.current_time();
        ctx.advance_time();

        assert_eq!(ctx.step(), 1);
        assert_eq!(ctx.current_time(), initial_time + Duration::seconds(60));
        assert_eq!(ctx.elapsed(), Duration::seconds(60));
    }

    #[test]
    fn test_custom_config() {
        let mut config = SimulationConfig::default();
        config.custom.insert(
            "max_agents".to_string(),
            serde_json::Value::Number(1000.into()),
        );

        let ctx = SimulationContext::from_now(config);
        assert_eq!(
            ctx.config().custom.get("max_agents"),
            Some(&serde_json::Value::Number(1000.into()))
        );
    }
}
