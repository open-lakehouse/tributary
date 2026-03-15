//! Journey planning and tracking
//!
//! This module will provide journey planning with road network routing.
//! Full implementation will be completed with graph integration.

use chrono::{DateTime, Duration, Utc};
use geo_types::LineString;
use serde::{Deserialize, Serialize};
use sim_core::utils::idents::{AgentId, JourneyId};
use std::collections::HashMap;

/// State of a journey
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JourneyState {
    /// Unique identifier
    pub id: JourneyId,
    /// Agent making the journey
    pub agent_id: AgentId,
    /// Route geometry
    pub route: LineString<f64>,
    /// Total distance in meters
    pub distance_m: f64,
    /// Current progress (0.0 to 1.0)
    pub progress: f64,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

/// A journey (alias for compatibility)
pub type Journey = JourneyState;

/// Model for journey velocity
pub trait VelocityModel: Send + Sync {
    /// Get velocity in meters per second
    ///
    /// # Arguments
    ///
    /// * `journey` - The journey being traveled
    /// * `progress` - Current progress (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Velocity in m/s
    fn velocity_m_s(&self, journey: &JourneyState, progress: f64) -> f64;
}

/// Constant velocity model
pub struct ConstantVelocity {
    pub velocity_m_s: f64,
}

impl ConstantVelocity {
    pub fn new(velocity_m_s: f64) -> Self {
        Self { velocity_m_s }
    }
}

impl Default for ConstantVelocity {
    fn default() -> Self {
        Self::new(5.0) // 5 m/s (~18 km/h, walking/cycling speed)
    }
}

impl VelocityModel for ConstantVelocity {
    fn velocity_m_s(&self, _journey: &JourneyState, _progress: f64) -> f64 {
        self.velocity_m_s
    }
}

/// Movement handler (simplified version for Phase 2)
///
/// This is a placeholder. Full implementation with road network routing
/// will be added when we integrate the graph module.
pub struct MovementHandler {
    active_journeys: HashMap<JourneyId, JourneyState>,
    velocity_model: Box<dyn VelocityModel>,
}

impl MovementHandler {
    /// Create a new movement handler
    pub fn new() -> Self {
        Self {
            active_journeys: HashMap::new(),
            velocity_model: Box::new(ConstantVelocity::default()),
        }
    }

    /// Set the velocity model
    pub fn with_velocity_model(mut self, model: Box<dyn VelocityModel>) -> Self {
        self.velocity_model = model;
        self
    }

    /// Add a journey (simplified - no routing yet)
    pub fn add_journey(&mut self, journey: JourneyState) {
        let id = journey.id;
        self.active_journeys.insert(id, journey);
    }

    /// Advance all journeys by time step
    pub fn advance_journeys(&mut self, time_step: Duration) -> crate::Result<()> {
        let time_step_secs = time_step.num_seconds() as f64;

        for journey in self.active_journeys.values_mut() {
            let velocity = self.velocity_model.velocity_m_s(journey, journey.progress);
            let distance_traveled = velocity * time_step_secs;
            let progress_delta = distance_traveled / journey.distance_m;

            journey.progress = (journey.progress + progress_delta).min(1.0);
        }

        Ok(())
    }

    /// Get completed journeys (progress >= 1.0)
    pub fn get_completed_journeys(&self) -> Vec<JourneyState> {
        self.active_journeys
            .values()
            .filter(|j| j.progress >= 1.0)
            .cloned()
            .collect()
    }

    /// Remove completed journeys
    pub fn remove_completed(&mut self) -> Vec<JourneyState> {
        let completed_ids: Vec<_> = self
            .active_journeys
            .iter()
            .filter(|(_, j)| j.progress >= 1.0)
            .map(|(id, _)| *id)
            .collect();

        completed_ids
            .into_iter()
            .filter_map(|id| self.active_journeys.remove(&id))
            .collect()
    }

    /// Get active journey count
    pub fn active_count(&self) -> usize {
        self.active_journeys.len()
    }
}

impl Default for MovementHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::Point;

    #[test]
    fn test_journey_progression() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(1.0, 1.0).into(),
            ]),
            distance_m: 1000.0,
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // Advance by 100 seconds at 5 m/s = 500m
        handler.advance_journeys(Duration::seconds(100)).unwrap();

        let journeys: Vec<_> = handler.active_journeys.values().collect();
        assert_eq!(journeys.len(), 1);
        assert!(journeys[0].progress > 0.4 && journeys[0].progress < 0.6);
    }

    #[test]
    fn test_journey_completion() {
        let mut handler = MovementHandler::new();

        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::new(vec![
                Point::new(0.0, 0.0).into(),
                Point::new(1.0, 1.0).into(),
            ]),
            distance_m: 500.0,
            progress: 0.0,
            start_time: Utc::now(),
            metadata: HashMap::new(),
        };

        handler.add_journey(journey);

        // Advance by 200 seconds at 5 m/s = 1000m (more than distance)
        handler.advance_journeys(Duration::seconds(200)).unwrap();

        let completed = handler.get_completed_journeys();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].progress, 1.0);
    }
}
