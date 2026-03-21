//! Simulation context and configuration
//!
//! This module provides the simulation runtime context including time management,
//! configuration, and catalogs.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Controls how the simulation clock advances each tick.
///
/// `TimeMode` is runtime state on [`SimulationContext`] — it is deliberately
/// kept separate from [`SimulationConfig`] so that `SimulationConfig` remains
/// `serde`-serializable without dragging in runtime concerns.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeMode {
    /// Advance `current_time` by `time_step` each tick.
    ///
    /// When `end_time` is `Some`, the runner will stop once
    /// `current_time >= end_time`. When `None`, the simulation runs until
    /// explicitly stopped or the step limit is reached.
    Range { end_time: Option<DateTime<Utc>> },

    /// `current_time` tracks the wall clock (`Utc::now()`) each tick.
    ///
    /// The runner sleeps between ticks to maintain real-time pacing.
    Live,

    /// Fast-forward from `start_time` until `current_time >= Utc::now()`,
    /// then automatically transition to [`TimeMode::Live`].
    Catchup,
}

impl Default for TimeMode {
    fn default() -> Self {
        TimeMode::Range { end_time: None }
    }
}

/// Result returned by [`SimulationContext::advance`] (and by
/// [`crate::runner::SimulationRunner::tick`]).
///
/// Callers should break out of their tick loop on any variant other than
/// `Continue`.
#[derive(Debug, Clone, PartialEq)]
pub enum TickResult {
    /// The tick completed normally; continue simulation.
    Continue,
    /// `Range` mode with a defined `end_time` — the simulation reached it.
    ReachedEnd,
    /// `Catchup` mode just crossed `Utc::now()` and transitioned to `Live`.
    SwitchedToLive,
}

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
    /// How the simulation clock advances each tick
    time_mode: TimeMode,
    /// Reference point for Live/Catchup sleep calculations:
    /// `(wall_clock_at_live_start, step_at_live_start)`
    live_reference: Option<(DateTime<Utc>, usize)>,
    /// Speed multiplier for Live mode (1.0 = real-time, 2.0 = 2x speed).
    /// Clamped to [0.125, 16.0] when set.
    pub live_speed: f32,
}

impl SimulationContext {
    /// Create a new simulation context
    pub fn new(start_time: DateTime<Utc>, config: SimulationConfig) -> Self {
        Self {
            start_time,
            current_time: start_time,
            config,
            step: 0,
            time_mode: TimeMode::default(),
            live_reference: None,
            live_speed: 1.0,
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

    /// Return a reference to the current [`TimeMode`].
    pub fn time_mode(&self) -> &TimeMode {
        &self.time_mode
    }

    /// Replace the current [`TimeMode`].
    ///
    /// Calling this mid-simulation will take effect on the *next* call to
    /// [`Self::advance`].  Switching to `Live` or `Catchup` resets the
    /// `live_reference` so sleep calculations start fresh.
    pub fn set_time_mode(&mut self, mode: TimeMode) {
        self.time_mode = mode;
        self.live_reference = None;
    }

    /// Advance the simulation clock by one tick according to the current
    /// [`TimeMode`] and return a [`TickResult`] so the runner knows whether
    /// to continue or stop.
    ///
    /// This is the canonical time-advancement path. [`Self::advance_time`]
    /// delegates here for backward compatibility.
    pub fn advance(&mut self) -> TickResult {
        match self.time_mode.clone() {
            TimeMode::Range { end_time: None } => {
                self.current_time += self.config.time_step;
                self.step += 1;
                TickResult::Continue
            }
            TimeMode::Range {
                end_time: Some(end),
            } => {
                self.current_time += self.config.time_step;
                self.step += 1;
                if self.current_time >= end {
                    TickResult::ReachedEnd
                } else {
                    TickResult::Continue
                }
            }
            TimeMode::Live => {
                // Ensure live_reference is set before sleeping calculations
                if self.live_reference.is_none() {
                    self.live_reference = Some((Utc::now(), self.step));
                }
                self.current_time = Utc::now();
                self.step += 1;
                TickResult::Continue
            }
            TimeMode::Catchup => {
                self.current_time += self.config.time_step;
                self.step += 1;
                if self.current_time >= Utc::now() {
                    // Transition to Live
                    self.time_mode = TimeMode::Live;
                    self.live_reference = Some((Utc::now(), self.step));
                    TickResult::SwitchedToLive
                } else {
                    TickResult::Continue
                }
            }
        }
    }

    /// Returns how long the runner should sleep after this tick to stay
    /// aligned with wall time, or `None` if no sleep is needed.
    ///
    /// Only meaningful in `Live` mode (or `Catchup` after transition).
    /// Returns `None` in `Range` / `Catchup` (pre-transition) modes.
    pub fn sleep_duration(&self) -> Option<std::time::Duration> {
        let (ref_wall, ref_step) = self.live_reference?;
        // Only sleep in Live mode (live_reference is set when mode is Live)
        if !matches!(self.time_mode, TimeMode::Live) {
            return None;
        }
        let steps_since = self.step.saturating_sub(ref_step);
        let step_nanos = self.config.time_step.num_nanoseconds().unwrap_or(0);
        // Divide sim nanos by speed to get wall-clock nanos per step.
        let speed = self.live_speed.max(0.001) as f64;
        let wall_nanos_per_step = (step_nanos as f64 / speed) as i64;
        let elapsed_wall_ns = steps_since as i64 * wall_nanos_per_step;
        let next_wall = ref_wall + Duration::nanoseconds(elapsed_wall_ns);
        let now = Utc::now();
        if now < next_wall {
            (next_wall - now).to_std().ok()
        } else {
            None
        }
    }

    /// Advance the simulation time by one step.
    ///
    /// This is a backward-compatible alias for [`Self::advance`]; the
    /// returned [`TickResult`] is discarded. Prefer `advance()` in new code
    /// so callers can react to `ReachedEnd` / `SwitchedToLive`.
    pub fn advance_time(&mut self) {
        self.advance();
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

    #[test]
    fn test_default_mode_is_range_forever() {
        let ctx = SimulationContext::from_now(SimulationConfig::default());
        assert_eq!(*ctx.time_mode(), TimeMode::Range { end_time: None });
    }

    #[test]
    fn test_range_end_time() {
        let start = Utc::now();
        let config = SimulationConfig {
            time_step: Duration::seconds(60),
            ..Default::default()
        };
        let mut ctx = SimulationContext::new(start, config);
        let end = start + Duration::seconds(3 * 60); // 3 steps
        ctx.set_time_mode(TimeMode::Range {
            end_time: Some(end),
        });

        assert_eq!(ctx.advance(), TickResult::Continue);
        assert_eq!(ctx.advance(), TickResult::Continue);
        // Third advance pushes current_time to exactly end
        assert_eq!(ctx.advance(), TickResult::ReachedEnd);
        assert_eq!(ctx.step(), 3);
    }

    #[test]
    fn test_range_no_end_always_continues() {
        let mut ctx = SimulationContext::from_now(SimulationConfig::default());
        for _ in 0..100 {
            assert_eq!(ctx.advance(), TickResult::Continue);
        }
    }

    #[test]
    fn test_catchup_transitions_to_live() {
        // Use a start time well in the past so catchup triggers within a few ticks
        let past = Utc::now() - Duration::seconds(10);
        let config = SimulationConfig {
            time_step: Duration::seconds(5),
            ..Default::default()
        };
        let mut ctx = SimulationContext::new(past, config);
        ctx.set_time_mode(TimeMode::Catchup);

        // Should transition within a small number of ticks (past + N*5s crosses now)
        let mut result = TickResult::Continue;
        for _ in 0..20 {
            result = ctx.advance();
            if result == TickResult::SwitchedToLive {
                break;
            }
        }
        assert_eq!(result, TickResult::SwitchedToLive);
        assert_eq!(*ctx.time_mode(), TimeMode::Live);
    }

    #[test]
    fn test_live_sleep_duration() {
        // In Live mode, sleep_duration should return Some when we haven't yet
        // reached the next scheduled wall-clock tick, or None if we're behind.
        // We can't deterministically test the exact value, but we can verify
        // it returns a non-negative duration (i.e. doesn't panic or go negative).
        let config = SimulationConfig {
            time_step: Duration::seconds(60),
            ..Default::default()
        };
        let mut ctx = SimulationContext::from_now(config);
        ctx.set_time_mode(TimeMode::Live);
        ctx.advance(); // sets live_reference on first call

        // After advancing once, sleep_duration() must return Some or None
        // without panicking. If the machine is fast, we should get Some.
        let dur = ctx.sleep_duration();
        if let Some(d) = dur {
            assert!(d <= std::time::Duration::from_secs(61));
        }
    }
}
