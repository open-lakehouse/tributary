//! Loading Dock Resource
//!
//! Loading docks handle package loading onto delivery vehicles.
//! They have limited vehicle slots and processing capacity.

use sim_core::{Resource, ResourceId};
use std::collections::{HashMap, VecDeque};

/// Loading Dock Resource
///
/// Manages package loading operations with vehicle slot management.
///
/// # Characteristics
///
/// - **Vehicle Slots**: Max 2 vehicles loading concurrently
/// - **Processing Rate**: 5 packages/hour per vehicle
/// - **Per-Vehicle Queues**: Each vehicle has its own loading queue
#[derive(Debug, Clone)]
pub struct LoadingDock {
    pub id: ResourceId,
    pub name: String,
    pub vehicle_slots: u32,
    pub packages_per_hour: f64,
    pub active_vehicles: HashMap<String, VehicleLoadingState>,
    pub waiting_vehicles: VecDeque<String>,
    pub total_vehicles_serviced: u64,
}

#[derive(Debug, Clone)]
pub struct VehicleLoadingState {
    pub driver_id: String,
    pub packages_to_load: Vec<String>,
    pub packages_loaded: usize,
    pub loading_started_tick: u64,
    pub next_package_ready_tick: u64,
}

impl LoadingDock {
    /// Create a new loading dock
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the dock
    /// * `vehicle_slots` - Maximum concurrent vehicles
    /// * `packages_per_hour` - Loading rate per vehicle
    pub fn new(name: String, vehicle_slots: u32, packages_per_hour: f64) -> Self {
        Self {
            id: ResourceId::new(),
            name,
            vehicle_slots,
            packages_per_hour,
            active_vehicles: HashMap::new(),
            waiting_vehicles: VecDeque::new(),
            total_vehicles_serviced: 0,
        }
    }

    /// Create a standard loading dock (2 slots, 5 packages/hour)
    pub fn standard(name: String) -> Self {
        Self::new(name, 2, 5.0)
    }

    /// Check if dock has available vehicle slots
    pub fn has_available_slot(&self) -> bool {
        self.active_vehicles.len() < self.vehicle_slots as usize
    }

    /// Get number of available slots
    pub fn available_slots(&self) -> u32 {
        self.vehicle_slots.saturating_sub(self.active_vehicles.len() as u32)
    }

    /// Assign a vehicle for loading
    ///
    /// # Arguments
    ///
    /// * `driver_id` - Driver identifier
    /// * `package_ids` - List of packages to load onto vehicle
    /// * `current_tick` - Current simulation tick
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Vehicle assigned successfully
    /// * `Err(String)` - No available slots or other error
    pub fn assign_vehicle(
        &mut self,
        driver_id: String,
        package_ids: Vec<String>,
        current_tick: u64,
    ) -> Result<(), String> {
        if !self.has_available_slot() {
            self.waiting_vehicles.push_back(driver_id);
            return Err("No available loading slots".to_string());
        }

        // Calculate when first package will be ready
        // packages_per_hour -> seconds_per_package = 3600 / rate
        // ticks_per_package = seconds_per_package / 30
        let seconds_per_package = 3600.0 / self.packages_per_hour;
        let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;

        let state = VehicleLoadingState {
            driver_id: driver_id.clone(),
            packages_to_load: package_ids,
            packages_loaded: 0,
            loading_started_tick: current_tick,
            next_package_ready_tick: current_tick + ticks_per_package,
        };

        self.active_vehicles.insert(driver_id, state);
        Ok(())
    }

    /// Process loading operations for current tick
    ///
    /// # Arguments
    ///
    /// * `current_tick` - Current simulation tick
    ///
    /// # Returns
    ///
    /// List of (driver_id, completed) tuples where completed=true means all packages loaded
    pub fn process_tick(&mut self, current_tick: u64) -> Vec<(String, bool)> {
        let mut results = Vec::new();
        let mut completed_vehicles = Vec::new();

        // Update each active vehicle
        for (driver_id, state) in self.active_vehicles.iter_mut() {
            let mut packages_loaded_this_tick = 0;

            // Load packages that are ready
            while state.next_package_ready_tick <= current_tick
                && state.packages_loaded < state.packages_to_load.len()
            {
                state.packages_loaded += 1;
                packages_loaded_this_tick += 1;

                // Calculate next package ready time
                let seconds_per_package = 3600.0 / self.packages_per_hour;
                let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;
                state.next_package_ready_tick = current_tick + ticks_per_package;
            }

            // Check if all packages loaded
            let all_loaded = state.packages_loaded >= state.packages_to_load.len();
            if all_loaded {
                completed_vehicles.push(driver_id.clone());
                results.push((driver_id.clone(), true));
            } else if packages_loaded_this_tick > 0 {
                results.push((driver_id.clone(), false));
            }
        }

        // Remove completed vehicles
        for driver_id in &completed_vehicles {
            self.active_vehicles.remove(driver_id);
            self.total_vehicles_serviced += 1;
        }

        // Assign waiting vehicles to available slots
        while self.has_available_slot() && !self.waiting_vehicles.is_empty() {
            if let Some(_driver_id) = self.waiting_vehicles.pop_front() {
                // Need to know package count - for now, skip
                // In real implementation, would store package info with waiting vehicles
            }
        }

        results
    }

    /// Get loading progress for a vehicle
    ///
    /// # Returns
    ///
    /// * `Some((loaded, total))` - Progress tuple
    /// * `None` - Vehicle not found
    pub fn get_vehicle_progress(&self, driver_id: &str) -> Option<(usize, usize)> {
        self.active_vehicles.get(driver_id).map(|state| {
            (state.packages_loaded, state.packages_to_load.len())
        })
    }

    /// Check if a vehicle is loading
    pub fn is_vehicle_loading(&self, driver_id: &str) -> bool {
        self.active_vehicles.contains_key(driver_id)
    }

    /// Get utilization percentage (0-100)
    pub fn utilization(&self) -> f64 {
        if self.vehicle_slots == 0 {
            0.0
        } else {
            (self.active_vehicles.len() as f64 / self.vehicle_slots as f64) * 100.0
        }
    }

    /// Get number of waiting vehicles
    pub fn waiting_count(&self) -> usize {
        self.waiting_vehicles.len()
    }
}

impl Resource for LoadingDock {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "loading_dock"
    }

    fn capacity(&self) -> usize {
        self.vehicle_slots as usize
    }

    fn current_load(&self) -> usize {
        self.active_vehicles.len()
    }

    fn is_available(&self) -> bool {
        self.has_available_slot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loading_dock_creation() {
        let dock = LoadingDock::standard("Dock-A".to_string());

        assert_eq!(dock.name, "Dock-A");
        assert_eq!(dock.vehicle_slots, 2);
        assert_eq!(dock.packages_per_hour, 5.0);
        assert!(dock.has_available_slot());
    }

    #[test]
    fn test_custom_loading_dock() {
        let dock = LoadingDock::new("Custom".to_string(), 4, 10.0);

        assert_eq!(dock.vehicle_slots, 4);
        assert_eq!(dock.packages_per_hour, 10.0);
    }

    #[test]
    fn test_assign_vehicle() {
        let mut dock = LoadingDock::standard("D1".to_string());

        let packages = vec!["pkg-1".to_string(), "pkg-2".to_string()];
        let result = dock.assign_vehicle("driver-1".to_string(), packages, 1);

        assert!(result.is_ok());
        assert_eq!(dock.active_vehicles.len(), 1);
        assert!(dock.is_vehicle_loading("driver-1"));
    }

    #[test]
    fn test_slot_capacity() {
        let mut dock = LoadingDock::standard("D1".to_string());

        // Assign 2 vehicles (fills capacity)
        dock.assign_vehicle("driver-1".to_string(), vec!["p1".to_string()], 1).unwrap();
        dock.assign_vehicle("driver-2".to_string(), vec!["p2".to_string()], 1).unwrap();

        assert_eq!(dock.available_slots(), 0);
        assert!(!dock.has_available_slot());

        // Third vehicle should be queued
        let result = dock.assign_vehicle("driver-3".to_string(), vec!["p3".to_string()], 1);
        assert!(result.is_err());
        assert_eq!(dock.waiting_count(), 1);
    }

    #[test]
    fn test_loading_progress() {
        let mut dock = LoadingDock::standard("D1".to_string());

        let packages = vec!["p1".to_string(), "p2".to_string(), "p3".to_string()];
        dock.assign_vehicle("driver-1".to_string(), packages, 1).unwrap();

        // Initial progress
        let progress = dock.get_vehicle_progress("driver-1");
        assert_eq!(progress, Some((0, 3)));

        // Process some ticks
        // Loading rate: 5 packages/hour = 720 seconds/package = 24 ticks/package
        for tick in 2..25 {
            dock.process_tick(tick);
        }

        // First package should be loaded at tick 25
        dock.process_tick(25);
        let progress = dock.get_vehicle_progress("driver-1");
        assert_eq!(progress, Some((1, 3)));
    }

    #[test]
    fn test_complete_loading() {
        let mut dock = LoadingDock::standard("D1".to_string());

        let packages = vec!["p1".to_string()];
        dock.assign_vehicle("driver-1".to_string(), packages, 1).unwrap();

        // Process until completion
        // 1 package at 5/hour = 720 sec = 24 ticks
        for tick in 2..25 {
            let results = dock.process_tick(tick);
            assert_eq!(results.len(), 0); // Not complete yet
        }

        let results = dock.process_tick(25);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "driver-1");
        assert!(results[0].1); // Complete

        // Vehicle should be removed from active
        assert!(!dock.is_vehicle_loading("driver-1"));
        assert_eq!(dock.total_vehicles_serviced, 1);
    }

    #[test]
    fn test_utilization() {
        let mut dock = LoadingDock::standard("D1".to_string());

        assert_eq!(dock.utilization(), 0.0);

        dock.assign_vehicle("driver-1".to_string(), vec!["p1".to_string()], 1).unwrap();
        assert_eq!(dock.utilization(), 50.0);

        dock.assign_vehicle("driver-2".to_string(), vec!["p2".to_string()], 1).unwrap();
        assert_eq!(dock.utilization(), 100.0);
    }

    #[test]
    fn test_vehicle_not_found() {
        let dock = LoadingDock::standard("D1".to_string());

        assert!(!dock.is_vehicle_loading("driver-999"));
        assert_eq!(dock.get_vehicle_progress("driver-999"), None);
    }

    #[test]
    fn test_resource_trait_implementation() {
        let dock = LoadingDock::standard("D1".to_string());

        assert_eq!(dock.resource_type(), "loading_dock");
        assert_eq!(dock.capacity(), 2);
        assert!(dock.is_available());
    }

    #[test]
    fn test_multiple_packages_loading() {
        let mut dock = LoadingDock::standard("D1".to_string());

        let packages = vec![
            "p1".to_string(),
            "p2".to_string(),
            "p3".to_string(),
            "p4".to_string(),
            "p5".to_string(),
        ];
        dock.assign_vehicle("driver-1".to_string(), packages, 1).unwrap();

        // Process through all package loading
        // 5 packages * 24 ticks = 120 ticks total
        let mut last_progress = (0, 5);
        for tick in 2..150 {
            dock.process_tick(tick);
            if let Some(progress) = dock.get_vehicle_progress("driver-1") {
                if progress.0 > last_progress.0 {
                    last_progress = progress;
                }
            }
        }

        // All packages should be loaded
        assert!(!dock.is_vehicle_loading("driver-1"));
        assert_eq!(dock.total_vehicles_serviced, 1);
    }
}
