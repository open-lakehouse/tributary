//! Shared movement utilities for agent-based simulation
//!
//! Provides a common `move_towards` function used by agents that need to
//! navigate toward geographic targets over discrete simulation time steps.

use geo::Point;

/// Move a position toward a target, limited by speed and step duration.
///
/// Returns `true` if the position reached the target this step, `false` if
/// only partial progress was made.
///
/// # Arguments
///
/// * `location` - Current position (mutated in place)
/// * `target` - Destination to move toward
/// * `movement_speed` - Agent speed in meters per second
/// * `step_duration_secs` - Duration of the simulation time step in seconds
///
/// # Notes
///
/// Uses simplified Euclidean distance in coordinate units, not geodesic
/// distance. Suitable for small areas or simulations where coordinate-space
/// distance is an acceptable approximation.
pub fn move_towards(
    location: &mut Point<f64>,
    target: Point<f64>,
    movement_speed: f64,
    step_duration_secs: f64,
) -> bool {
    let dx = target.x() - location.x();
    let dy = target.y() - location.y();
    let distance = (dx * dx + dy * dy).sqrt();

    let max_travel = movement_speed * step_duration_secs;

    if distance <= max_travel {
        *location = target;
        true
    } else {
        let ratio = max_travel / distance;
        let new_x = location.x() + dx * ratio;
        let new_y = location.y() + dy * ratio;
        *location = Point::new(new_x, new_y);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arrives_when_within_range() {
        let mut loc = Point::new(0.0, 0.0);
        let target = Point::new(100.0, 0.0);
        // speed=12 m/s, step=30s → max_travel=360 > 100 → arrives
        let arrived = move_towards(&mut loc, target, 12.0, 30.0);
        assert!(arrived);
        assert_eq!(loc, target);
    }

    #[test]
    fn test_partial_progress_when_far() {
        let mut loc = Point::new(0.0, 0.0);
        let target = Point::new(1000.0, 0.0);
        // speed=12 m/s, step=30s → max_travel=360 < 1000 → partial
        let arrived = move_towards(&mut loc, target, 12.0, 30.0);
        assert!(!arrived);
        assert!(loc.x() > 0.0);
        assert!(loc.x() < 1000.0);
    }

    #[test]
    fn test_at_target_returns_true() {
        let mut loc = Point::new(5.0, 5.0);
        let target = Point::new(5.0, 5.0);
        let arrived = move_towards(&mut loc, target, 10.0, 30.0);
        assert!(arrived);
        assert_eq!(loc, target);
    }
}
