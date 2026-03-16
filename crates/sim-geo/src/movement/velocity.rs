//! Time-of-day dependent velocity models for journey simulation
//!
//! This module provides velocity models that adjust travel speeds based on the time of day,
//! including rush hour patterns and nighttime adjustments.

use super::{JourneyState, VelocityModel};
use crate::graph::RoadType;
use chrono::{DateTime, Timelike, Utc};

/// Velocity model that adjusts speeds based on time of day
///
/// This model applies different speed multipliers depending on whether it's rush hour,
/// off-peak, or nighttime. Rush hours are typically congested with slower travel speeds,
/// while nighttime often allows faster travel.
///
/// # Example
///
/// ```
/// use sim_geo::movement::velocity::TimeOfDayVelocity;
/// use sim_geo::graph::RoadType;
/// use chrono::{Utc, TimeZone};
///
/// let velocity_model = TimeOfDayVelocity::default();
///
/// // During rush hour (8am): 50% speed reduction
/// let rush_hour = Utc.with_ymd_and_hms(2024, 1, 15, 8, 30, 0).unwrap();
/// let speed = velocity_model.calculate_velocity(&RoadType::Arterial, rush_hour);
/// assert!(speed < RoadType::Arterial.default_speed_mps());
///
/// // During off-peak (2pm): normal speed
/// let off_peak = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();
/// let speed = velocity_model.calculate_velocity(&RoadType::Arterial, off_peak);
/// assert_eq!(speed, RoadType::Arterial.default_speed_mps());
///
/// // During night (2am): 20% speed increase
/// let night = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();
/// let speed = velocity_model.calculate_velocity(&RoadType::Arterial, night);
/// assert!(speed > RoadType::Arterial.default_speed_mps());
/// ```
#[derive(Debug, Clone)]
pub struct TimeOfDayVelocity {
    /// Speed multiplier during rush hour (0.0 to 1.0+)
    /// Default: 0.5 (50% speed reduction due to congestion)
    pub rush_hour_multiplier: f64,

    /// Speed multiplier during off-peak hours (typically 1.0)
    /// Default: 1.0 (normal speed)
    pub off_peak_multiplier: f64,

    /// Speed multiplier during nighttime (1.0+)
    /// Default: 1.2 (20% speed increase due to less traffic)
    pub night_multiplier: f64,
}

impl TimeOfDayVelocity {
    /// Create a new time-of-day velocity model with custom multipliers
    ///
    /// # Arguments
    ///
    /// * `rush_hour_multiplier` - Speed multiplier during rush hours (typically < 1.0)
    /// * `off_peak_multiplier` - Speed multiplier during normal hours (typically 1.0)
    /// * `night_multiplier` - Speed multiplier during nighttime (typically > 1.0)
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::movement::velocity::TimeOfDayVelocity;
    ///
    /// // More severe rush hour congestion (70% reduction)
    /// let model = TimeOfDayVelocity::new(0.3, 1.0, 1.2);
    /// ```
    pub fn new(rush_hour_multiplier: f64, off_peak_multiplier: f64, night_multiplier: f64) -> Self {
        Self {
            rush_hour_multiplier,
            off_peak_multiplier,
            night_multiplier,
        }
    }

    /// Check if the given time is during rush hour
    ///
    /// Rush hours are defined as:
    /// - Morning: 7:00 AM - 9:00 AM (UTC)
    /// - Evening: 5:00 PM - 7:00 PM (UTC, 17:00 - 19:00)
    ///
    /// Note: This uses UTC time. For production use, consider converting to local timezone.
    ///
    /// # Arguments
    ///
    /// * `time` - The time to check
    ///
    /// # Returns
    ///
    /// `true` if the time falls within rush hour periods, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::movement::velocity::TimeOfDayVelocity;
    /// use chrono::{Utc, TimeZone};
    ///
    /// let morning_rush = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    /// assert!(TimeOfDayVelocity::is_rush_hour(morning_rush));
    ///
    /// let midday = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
    /// assert!(!TimeOfDayVelocity::is_rush_hour(midday));
    ///
    /// let evening_rush = Utc.with_ymd_and_hms(2024, 1, 15, 18, 0, 0).unwrap();
    /// assert!(TimeOfDayVelocity::is_rush_hour(evening_rush));
    /// ```
    pub fn is_rush_hour(time: DateTime<Utc>) -> bool {
        let hour = time.hour();
        // Morning rush: 7-9 AM (7:00 - 8:59)
        // Evening rush: 5-7 PM (17:00 - 18:59)
        (7..9).contains(&hour) || (17..19).contains(&hour)
    }

    /// Check if the given time is during nighttime
    ///
    /// Nighttime is defined as: 10:00 PM - 5:00 AM (UTC, 22:00 - 4:59)
    ///
    /// Note: This uses UTC time. For production use, consider converting to local timezone.
    ///
    /// # Arguments
    ///
    /// * `time` - The time to check
    ///
    /// # Returns
    ///
    /// `true` if the time falls within nighttime hours, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::movement::velocity::TimeOfDayVelocity;
    /// use chrono::{Utc, TimeZone};
    ///
    /// let midnight = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
    /// assert!(TimeOfDayVelocity::is_night_time(midnight));
    ///
    /// let early_morning = Utc.with_ymd_and_hms(2024, 1, 15, 4, 0, 0).unwrap();
    /// assert!(TimeOfDayVelocity::is_night_time(early_morning));
    ///
    /// let daytime = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
    /// assert!(!TimeOfDayVelocity::is_night_time(daytime));
    /// ```
    pub fn is_night_time(time: DateTime<Utc>) -> bool {
        let hour = time.hour();
        // Night: 10 PM - 5 AM (22:00 - 4:59)
        !(5..22).contains(&hour)
    }

    /// Calculate velocity based on road type and current time
    ///
    /// This is the core method that combines the base speed limit of the road type
    /// with the time-of-day multiplier to produce the effective travel speed.
    ///
    /// # Arguments
    ///
    /// * `road_type` - The type of road being traveled
    /// * `current_time` - The current simulation time
    ///
    /// # Returns
    ///
    /// The effective velocity in meters per second
    ///
    /// # Example
    ///
    /// ```
    /// use sim_geo::movement::velocity::TimeOfDayVelocity;
    /// use sim_geo::graph::RoadType;
    /// use chrono::{Utc, TimeZone};
    ///
    /// let model = TimeOfDayVelocity::default();
    ///
    /// // Rush hour on arterial: 15 m/s * 0.5 = 7.5 m/s
    /// let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
    /// let speed = model.calculate_velocity(&RoadType::Arterial, rush_time);
    /// assert!((speed - 7.5).abs() < 0.01);
    /// ```
    pub fn calculate_velocity(&self, road_type: &RoadType, current_time: DateTime<Utc>) -> f64 {
        let base_speed = road_type.default_speed_mps();

        let multiplier = if Self::is_rush_hour(current_time) {
            self.rush_hour_multiplier
        } else if Self::is_night_time(current_time) {
            self.night_multiplier
        } else {
            self.off_peak_multiplier
        };

        base_speed * multiplier
    }
}

impl Default for TimeOfDayVelocity {
    /// Create a time-of-day velocity model with default multipliers
    ///
    /// Default values:
    /// - Rush hour: 0.5 (50% speed reduction)
    /// - Off-peak: 1.0 (normal speed)
    /// - Night: 1.2 (20% speed increase)
    fn default() -> Self {
        Self {
            rush_hour_multiplier: 0.5,
            off_peak_multiplier: 1.0,
            night_multiplier: 1.2,
        }
    }
}

impl VelocityModel for TimeOfDayVelocity {
    fn velocity_m_s(&self, journey: &JourneyState, _progress: f64) -> f64 {
        // Extract road type from journey metadata or use default
        // For now, assume Residential as default (can be enhanced later)
        let road_type = RoadType::Residential;

        // Use journey start time as reference point
        // In a real simulation, this would use the current simulation time
        self.calculate_velocity(&road_type, journey.start_time)
    }

    fn velocity_m_s_at_time(
        &self,
        _journey: &JourneyState,
        _progress: f64,
        current_time: DateTime<Utc>,
    ) -> f64 {
        // Extract road type from journey metadata or use default
        // For now, assume Residential as default (can be enhanced later)
        let road_type = RoadType::Residential;

        // Use the provided current time for time-of-day adjustments
        self.calculate_velocity(&road_type, current_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_is_rush_hour() {
        // Morning rush hour (7-9 AM)
        let morning_start = Utc.with_ymd_and_hms(2024, 1, 15, 7, 0, 0).unwrap();
        let morning_mid = Utc.with_ymd_and_hms(2024, 1, 15, 8, 30, 0).unwrap();
        let morning_end = Utc.with_ymd_and_hms(2024, 1, 15, 8, 59, 59).unwrap();

        assert!(TimeOfDayVelocity::is_rush_hour(morning_start));
        assert!(TimeOfDayVelocity::is_rush_hour(morning_mid));
        assert!(TimeOfDayVelocity::is_rush_hour(morning_end));

        // Evening rush hour (5-7 PM, 17:00-19:00)
        let evening_start = Utc.with_ymd_and_hms(2024, 1, 15, 17, 0, 0).unwrap();
        let evening_mid = Utc.with_ymd_and_hms(2024, 1, 15, 18, 30, 0).unwrap();
        let evening_end = Utc.with_ymd_and_hms(2024, 1, 15, 18, 59, 59).unwrap();

        assert!(TimeOfDayVelocity::is_rush_hour(evening_start));
        assert!(TimeOfDayVelocity::is_rush_hour(evening_mid));
        assert!(TimeOfDayVelocity::is_rush_hour(evening_end));

        // Not rush hour
        let before_morning = Utc.with_ymd_and_hms(2024, 1, 15, 6, 59, 59).unwrap();
        let midday = Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let after_evening = Utc.with_ymd_and_hms(2024, 1, 15, 19, 0, 0).unwrap();

        assert!(!TimeOfDayVelocity::is_rush_hour(before_morning));
        assert!(!TimeOfDayVelocity::is_rush_hour(midday));
        assert!(!TimeOfDayVelocity::is_rush_hour(after_evening));
    }

    #[test]
    fn test_is_night_time() {
        // Night time (10 PM - 5 AM, 22:00 - 4:59)
        let late_evening = Utc.with_ymd_and_hms(2024, 1, 15, 22, 0, 0).unwrap();
        let midnight = Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap();
        let early_morning = Utc.with_ymd_and_hms(2024, 1, 15, 4, 30, 0).unwrap();
        let before_dawn = Utc.with_ymd_and_hms(2024, 1, 15, 4, 59, 59).unwrap();

        assert!(TimeOfDayVelocity::is_night_time(late_evening));
        assert!(TimeOfDayVelocity::is_night_time(midnight));
        assert!(TimeOfDayVelocity::is_night_time(early_morning));
        assert!(TimeOfDayVelocity::is_night_time(before_dawn));

        // Not night time
        let dawn = Utc.with_ymd_and_hms(2024, 1, 15, 5, 0, 0).unwrap();
        let morning = Utc.with_ymd_and_hms(2024, 1, 15, 10, 0, 0).unwrap();
        let evening = Utc.with_ymd_and_hms(2024, 1, 15, 21, 59, 59).unwrap();

        assert!(!TimeOfDayVelocity::is_night_time(dawn));
        assert!(!TimeOfDayVelocity::is_night_time(morning));
        assert!(!TimeOfDayVelocity::is_night_time(evening));
    }

    #[test]
    fn test_calculate_velocity_rush_hour() {
        let model = TimeOfDayVelocity::default();
        let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();

        // Test all road types during rush hour
        let highway_speed = model.calculate_velocity(&RoadType::Highway, rush_time);
        assert_eq!(highway_speed, 30.0 * 0.5); // 15 m/s

        let arterial_speed = model.calculate_velocity(&RoadType::Arterial, rush_time);
        assert_eq!(arterial_speed, 15.0 * 0.5); // 7.5 m/s

        let residential_speed = model.calculate_velocity(&RoadType::Residential, rush_time);
        assert_eq!(residential_speed, 8.0 * 0.5); // 4 m/s

        let service_speed = model.calculate_velocity(&RoadType::ServiceRoad, rush_time);
        assert_eq!(service_speed, 5.0 * 0.5); // 2.5 m/s
    }

    #[test]
    fn test_calculate_velocity_off_peak() {
        let model = TimeOfDayVelocity::default();
        let off_peak_time = Utc.with_ymd_and_hms(2024, 1, 15, 14, 0, 0).unwrap();

        // Test all road types during off-peak
        let highway_speed = model.calculate_velocity(&RoadType::Highway, off_peak_time);
        assert_eq!(highway_speed, 30.0); // Normal speed

        let arterial_speed = model.calculate_velocity(&RoadType::Arterial, off_peak_time);
        assert_eq!(arterial_speed, 15.0);

        let residential_speed = model.calculate_velocity(&RoadType::Residential, off_peak_time);
        assert_eq!(residential_speed, 8.0);

        let service_speed = model.calculate_velocity(&RoadType::ServiceRoad, off_peak_time);
        assert_eq!(service_speed, 5.0);
    }

    #[test]
    fn test_calculate_velocity_night() {
        let model = TimeOfDayVelocity::default();
        let night_time = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();

        // Test all road types during night
        let highway_speed = model.calculate_velocity(&RoadType::Highway, night_time);
        assert_eq!(highway_speed, 30.0 * 1.2); // 36 m/s

        let arterial_speed = model.calculate_velocity(&RoadType::Arterial, night_time);
        assert_eq!(arterial_speed, 15.0 * 1.2); // 18 m/s

        let residential_speed = model.calculate_velocity(&RoadType::Residential, night_time);
        assert_eq!(residential_speed, 8.0 * 1.2); // 9.6 m/s

        let service_speed = model.calculate_velocity(&RoadType::ServiceRoad, night_time);
        assert_eq!(service_speed, 5.0 * 1.2); // 6 m/s
    }

    #[test]
    fn test_custom_multipliers() {
        let model = TimeOfDayVelocity::new(0.3, 1.0, 1.5);

        let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
        let rush_speed = model.calculate_velocity(&RoadType::Highway, rush_time);
        assert_eq!(rush_speed, 30.0 * 0.3); // 9 m/s

        let night_time = Utc.with_ymd_and_hms(2024, 1, 15, 2, 0, 0).unwrap();
        let night_speed = model.calculate_velocity(&RoadType::Highway, night_time);
        assert_eq!(night_speed, 30.0 * 1.5); // 45 m/s
    }

    #[test]
    fn test_velocity_model_trait_implementation() {
        use crate::movement::JourneyState;
        use geo_types::{LineString, Point};
        use sim_core::utils::idents::{AgentId, JourneyId};
        use std::collections::HashMap;

        let model = TimeOfDayVelocity::default();

        // Create a journey starting during rush hour
        let rush_time = Utc.with_ymd_and_hms(2024, 1, 15, 8, 0, 0).unwrap();
        let journey = JourneyState {
            id: JourneyId::new(),
            agent_id: AgentId::new(),
            route: LineString::from(vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 1.0),
            ]),
            distance_m: 1000.0,
            progress: 0.5,
            start_time: rush_time,
            metadata: HashMap::new(),
        };

        // Test VelocityModel trait implementation
        let velocity = model.velocity_m_s(&journey, 0.5);

        // Should apply rush hour multiplier to residential roads (default)
        assert_eq!(velocity, 8.0 * 0.5); // 4 m/s
    }

    #[test]
    fn test_time_boundaries() {
        // Test exact boundaries between time periods
        let model = TimeOfDayVelocity::default();

        // 6:59:59 -> not rush hour (off-peak)
        let before_rush = Utc.with_ymd_and_hms(2024, 1, 15, 6, 59, 59).unwrap();
        let speed = model.calculate_velocity(&RoadType::Arterial, before_rush);
        assert_eq!(speed, 15.0); // Off-peak speed

        // 7:00:00 -> rush hour starts
        let rush_start = Utc.with_ymd_and_hms(2024, 1, 15, 7, 0, 0).unwrap();
        let speed = model.calculate_velocity(&RoadType::Arterial, rush_start);
        assert_eq!(speed, 7.5); // Rush hour speed

        // 9:00:00 -> rush hour ends (off-peak)
        let rush_end = Utc.with_ymd_and_hms(2024, 1, 15, 9, 0, 0).unwrap();
        let speed = model.calculate_velocity(&RoadType::Arterial, rush_end);
        assert_eq!(speed, 15.0); // Off-peak speed
    }
}
