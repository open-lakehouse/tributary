//! Sorting Station Resource
//!
//! Sorting stations process incoming packages and route them for delivery.
//! They have limited capacity and process packages at a fixed rate.

use sim_core::{Resource, ResourceId};
use std::collections::VecDeque;

/// Sorting Station Resource
///
/// Processes packages with limited concurrent capacity and fixed throughput rate.
///
/// # Characteristics
///
/// - **Capacity**: Max 10 packages being sorted concurrently
/// - **Processing Rate**: 12 packages/hour (5 minutes per package)
/// - **FIFO Queue**: Packages processed in arrival order
#[derive(Debug, Clone)]
pub struct SortingStation {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u32,
    pub processing_rate_per_hour: f64,
    pub current_load: u32,
    pub queue: VecDeque<String>,        // Package IDs waiting
    pub processing: Vec<(String, u64)>, // (package_id, completion_tick)
    pub total_processed: u64,
}

impl SortingStation {
    /// Create a new sorting station
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the station
    /// * `capacity` - Maximum concurrent packages being sorted
    /// * `processing_rate_per_hour` - Throughput rate (packages/hour)
    pub fn new(name: String, capacity: u32, processing_rate_per_hour: f64) -> Self {
        Self {
            id: ResourceId::new(),
            name,
            capacity,
            processing_rate_per_hour,
            current_load: 0,
            queue: VecDeque::new(),
            processing: Vec::new(),
            total_processed: 0,
        }
    }

    /// Create a standard sorting station (capacity=10, rate=12/hr)
    pub fn standard(name: String) -> Self {
        Self::new(name, 10, 12.0)
    }

    /// Check if station can accept more packages
    pub fn has_capacity(&self) -> bool {
        self.current_load < self.capacity
    }

    /// Get available capacity
    pub fn available_capacity(&self) -> u32 {
        self.capacity.saturating_sub(self.current_load)
    }

    /// Add a package to the sorting queue
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Package queued successfully
    /// * `Err(String)` - Queue is full or other error
    pub fn enqueue_package(&mut self, package_id: String) -> Result<(), String> {
        // Check total load (processing + queue)
        let total_load = self.current_load + self.queue.len() as u32;
        if total_load >= self.capacity * 3 {
            return Err("Sorting station queue is full".to_string());
        }

        self.queue.push_back(package_id);
        Ok(())
    }

    /// Process queued packages (call this each simulation tick)
    ///
    /// # Arguments
    ///
    /// * `current_tick` - Current simulation tick count
    pub fn process_tick(&mut self, current_tick: u64) {
        // Check for completed packages
        self.processing.retain(|(_, completion_tick)| {
            if *completion_tick <= current_tick {
                self.current_load = self.current_load.saturating_sub(1);
                self.total_processed += 1;
                false // Remove from processing
            } else {
                true // Keep processing
            }
        });

        // Start processing queued packages if capacity available
        while self.has_capacity() && !self.queue.is_empty() {
            if let Some(package_id) = self.queue.pop_front() {
                // Calculate completion time
                // Processing rate is packages/hour, so time per package = 3600 / rate seconds
                // At 30 sec/tick: ticks_per_package = (3600 / rate) / 30
                let seconds_per_package = 3600.0 / self.processing_rate_per_hour;
                let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;
                let completion_tick = current_tick + ticks_per_package;

                self.processing.push((package_id, completion_tick));
                self.current_load += 1;
            }
        }
    }

    /// Get list of packages that completed this tick
    pub fn get_completed(&self, current_tick: u64) -> Vec<String> {
        self.processing
            .iter()
            .filter(|(_, completion)| *completion == current_tick)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get utilization percentage (0-100)
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current_load as f64 / self.capacity as f64) * 100.0
        }
    }

    /// Get queue length
    pub fn queue_length(&self) -> usize {
        self.queue.len()
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.total_processed = 0;
    }
}

impl Resource for SortingStation {
    fn id(&self) -> ResourceId {
        self.id
    }

    fn resource_type(&self) -> &str {
        "sorting_station"
    }

    fn capacity(&self) -> usize {
        self.capacity as usize
    }

    fn current_load(&self) -> usize {
        self.current_load as usize
    }

    fn is_available(&self) -> bool {
        self.has_capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorting_station_creation() {
        let station = SortingStation::standard("Station-A".to_string());

        assert_eq!(station.name, "Station-A");
        assert_eq!(station.capacity, 10);
        assert_eq!(station.processing_rate_per_hour, 12.0);
        assert_eq!(station.current_load, 0);
        assert!(station.has_capacity());
    }

    #[test]
    fn test_custom_sorting_station() {
        let station = SortingStation::new("Custom".to_string(), 20, 24.0);

        assert_eq!(station.capacity, 20);
        assert_eq!(station.processing_rate_per_hour, 24.0);
    }

    #[test]
    fn test_enqueue_package() {
        let mut station = SortingStation::standard("S1".to_string());

        assert!(station.enqueue_package("pkg-1".to_string()).is_ok());
        assert_eq!(station.queue_length(), 1);

        assert!(station.enqueue_package("pkg-2".to_string()).is_ok());
        assert_eq!(station.queue_length(), 2);
    }

    #[test]
    fn test_enqueue_limit() {
        let mut station = SortingStation::standard("S1".to_string());

        // Can queue up to capacity * 3 = 30 packages
        for i in 0..30 {
            assert!(station.enqueue_package(format!("pkg-{}", i)).is_ok());
        }

        // 31st should fail
        assert!(station.enqueue_package("pkg-31".to_string()).is_err());
    }

    #[test]
    fn test_process_tick_starts_processing() {
        let mut station = SortingStation::standard("S1".to_string());

        station.enqueue_package("pkg-1".to_string()).unwrap();
        station.enqueue_package("pkg-2".to_string()).unwrap();

        assert_eq!(station.current_load, 0);
        assert_eq!(station.queue_length(), 2);

        // First tick should start processing both packages
        station.process_tick(1);

        assert_eq!(station.current_load, 2);
        assert_eq!(station.queue_length(), 0);
        assert_eq!(station.processing.len(), 2);
    }

    #[test]
    fn test_processing_completion() {
        let mut station = SortingStation::standard("S1".to_string());

        station.enqueue_package("pkg-1".to_string()).unwrap();
        station.process_tick(1);

        // Package should be processing
        assert_eq!(station.current_load, 1);

        // Processing takes (3600 / 12) / 30 = 10 ticks
        for tick in 2..11 {
            station.process_tick(tick);
            assert_eq!(station.current_load, 1); // Still processing
        }

        // At tick 11, should be complete
        station.process_tick(11);
        assert_eq!(station.current_load, 0);
        assert_eq!(station.total_processed, 1);
    }

    #[test]
    fn test_capacity_limits() {
        let mut station = SortingStation::standard("S1".to_string());

        // Queue 15 packages
        for i in 0..15 {
            station.enqueue_package(format!("pkg-{}", i)).unwrap();
        }

        // First tick processes up to capacity (10)
        station.process_tick(1);

        assert_eq!(station.current_load, 10);
        assert_eq!(station.queue_length(), 5);
        assert!(!station.has_capacity());
    }

    #[test]
    fn test_get_completed() {
        let mut station = SortingStation::standard("S1".to_string());

        station.enqueue_package("pkg-1".to_string()).unwrap();
        station.process_tick(1);

        // No completions at early ticks
        assert_eq!(station.get_completed(5).len(), 0);

        // Should complete at tick 11 - check before processing
        let completed_before = station.get_completed(11);
        assert_eq!(completed_before.len(), 1); // Package is scheduled to complete at tick 11

        // Process to completion
        station.process_tick(11);
        // After processing, package is removed from processing array
        assert_eq!(station.total_processed, 1);
    }

    #[test]
    fn test_utilization() {
        let mut station = SortingStation::standard("S1".to_string());

        assert_eq!(station.utilization(), 0.0);

        // Add 5 packages (50% utilization)
        for i in 0..5 {
            station.enqueue_package(format!("pkg-{}", i)).unwrap();
        }
        station.process_tick(1);

        assert_eq!(station.utilization(), 50.0);

        // Fill to capacity
        for i in 5..10 {
            station.enqueue_package(format!("pkg-{}", i)).unwrap();
        }
        station.process_tick(2);

        assert_eq!(station.utilization(), 100.0);
    }

    #[test]
    fn test_available_capacity() {
        let mut station = SortingStation::standard("S1".to_string());

        assert_eq!(station.available_capacity(), 10);

        for i in 0..3 {
            station.enqueue_package(format!("pkg-{}", i)).unwrap();
        }
        station.process_tick(1);

        assert_eq!(station.available_capacity(), 7);
    }

    #[test]
    fn test_fifo_ordering() {
        let mut station = SortingStation::new("S1".to_string(), 2, 12.0);

        station.enqueue_package("pkg-1".to_string()).unwrap();
        station.enqueue_package("pkg-2".to_string()).unwrap();
        station.enqueue_package("pkg-3".to_string()).unwrap();

        station.process_tick(1);

        // First two should be processing
        assert_eq!(station.processing.len(), 2);
        assert!(station.processing.iter().any(|(id, _)| id == "pkg-1"));
        assert!(station.processing.iter().any(|(id, _)| id == "pkg-2"));

        // Third should still be queued
        assert_eq!(station.queue_length(), 1);
    }

    #[test]
    fn test_resource_trait_implementation() {
        let station = SortingStation::standard("S1".to_string());

        assert_eq!(station.resource_type(), "sorting_station");
        assert_eq!(station.capacity(), 10);
        assert!(station.is_available());
    }
}
