//! Logistics simulation orchestration
//!
//! This module provides the main simulation runner and metrics collection.

use crate::{DeliveryAddress, Driver, Package, PackagePriority, Warehouse};
use chrono::Duration;
use geo::Point;
use rand::Rng;
use sim_core::{PopulationHandler, SimulationConfig, SimulationContext};

/// Logistics simulation metrics
#[derive(Debug, Clone, Default)]
pub struct LogisticsMetrics {
    pub packages_created: u64,
    pub packages_delivered: u64,
    pub drivers_used: u64,
    pub average_delivery_time_secs: f64,
    pub driver_utilization_percent: f64,
}

/// Main logistics simulation
///
/// Orchestrates package delivery simulation with warehouses, drivers, and packages.
pub struct LogisticsSimulation {
    context: SimulationContext,
    warehouses: Vec<Warehouse>,
    delivery_addresses: Vec<DeliveryAddress>,
    drivers: PopulationHandler<Driver>,
    packages: PopulationHandler<Package>,
    metrics: LogisticsMetrics,
}

impl LogisticsSimulation {
    /// Create a new logistics simulation with default setup
    ///
    /// Initializes:
    /// - 2 warehouses
    /// - 1000 delivery addresses
    /// - 20 drivers
    pub fn new() -> Self {
        let config = SimulationConfig {
            name: "logistics_simulation".to_string(),
            time_step: Duration::seconds(30),
            verbose: false,
            custom: serde_json::Map::new(),
        };
        let context = SimulationContext::from_now(config);

        // Create warehouses
        let warehouses = vec![
            Warehouse::new(
                "Warehouse North".to_string(),
                Point::new(-122.4, 37.8),
                1000,
            )
            .unwrap(),
            Warehouse::new(
                "Warehouse South".to_string(),
                Point::new(-122.3, 37.7),
                1000,
            )
            .unwrap(),
        ];

        // Create delivery addresses (1000 addresses in Bay Area)
        let mut rng = rand::thread_rng();
        let delivery_addresses: Vec<DeliveryAddress> = (0..1000)
            .map(|i| {
                let lat = 37.7 + rng.random::<f64>() * 0.2;
                let lon = -122.5 + rng.random::<f64>() * 0.3;
                DeliveryAddress::new(Point::new(lon, lat), format!("Customer-{}", i)).unwrap()
            })
            .collect();

        // Create drivers (20 drivers at Warehouse North)
        let mut drivers = PopulationHandler::new();
        for _ in 0..20 {
            drivers.add_agent(Driver::new(Point::new(-122.4, 37.8)));
        }

        // Start with empty package population
        let packages = PopulationHandler::new();

        Self {
            context,
            warehouses,
            delivery_addresses,
            drivers,
            packages,
            metrics: LogisticsMetrics::default(),
        }
    }

    /// Run simulation for specified number of hours
    ///
    /// # Arguments
    ///
    /// * `hours` - Number of simulation hours to run
    pub fn run_for_hours(&mut self, hours: u64) -> crate::Result<()> {
        let steps = hours * 120; // 30 sec/step = 120 steps/hour

        for _ in 0..steps {
            self.step()?;
        }

        Ok(())
    }

    /// Execute one simulation step
    fn step(&mut self) -> crate::Result<()> {
        // Tick all agents
        self.drivers.tick_agents(&self.context)?;
        self.packages.tick_agents(&self.context)?;

        // Advance simulation time
        self.context.advance_time();

        Ok(())
    }

    /// Create a new package in the simulation
    pub fn create_package(
        &mut self,
        warehouse_idx: usize,
        destination_idx: usize,
        weight_kg: f64,
        priority: PackagePriority,
    ) -> crate::Result<()> {
        if warehouse_idx >= self.warehouses.len() {
            return Err(crate::Error::Logistics(
                "Invalid warehouse index".to_string(),
            ));
        }
        if destination_idx >= self.delivery_addresses.len() {
            return Err(crate::Error::Logistics(
                "Invalid destination index".to_string(),
            ));
        }

        let warehouse = &self.warehouses[warehouse_idx];
        let destination = &self.delivery_addresses[destination_idx];

        let package = Package::new(
            warehouse.location,
            destination.location,
            weight_kg,
            priority,
        );

        self.packages.add_agent(package);
        self.metrics.packages_created += 1;

        Ok(())
    }

    /// Get current simulation metrics
    pub fn get_metrics(&self) -> &LogisticsMetrics {
        &self.metrics
    }

    /// Get number of active drivers
    pub fn driver_count(&self) -> usize {
        self.drivers.count()
    }

    /// Get number of packages
    pub fn package_count(&self) -> usize {
        self.packages.count()
    }

    /// Get number of warehouses
    pub fn warehouse_count(&self) -> usize {
        self.warehouses.len()
    }

    /// Get number of delivery addresses
    pub fn delivery_address_count(&self) -> usize {
        self.delivery_addresses.len()
    }
}

impl Default for LogisticsSimulation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulation_creation() {
        let sim = LogisticsSimulation::new();

        assert_eq!(sim.warehouse_count(), 2);
        assert_eq!(sim.delivery_address_count(), 1000);
        assert_eq!(sim.driver_count(), 20);
        assert_eq!(sim.package_count(), 0);
    }

    #[test]
    fn test_create_package() {
        let mut sim = LogisticsSimulation::new();

        sim.create_package(0, 0, 2.5, PackagePriority::Standard)
            .unwrap();
        assert_eq!(sim.package_count(), 1);
        assert_eq!(sim.get_metrics().packages_created, 1);

        sim.create_package(1, 5, 1.0, PackagePriority::Express)
            .unwrap();
        assert_eq!(sim.package_count(), 2);
    }

    #[test]
    fn test_invalid_warehouse_index() {
        let mut sim = LogisticsSimulation::new();

        let result = sim.create_package(10, 0, 1.0, PackagePriority::Standard);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_destination_index() {
        let mut sim = LogisticsSimulation::new();

        let result = sim.create_package(0, 2000, 1.0, PackagePriority::Standard);
        assert!(result.is_err());
    }

    #[test]
    fn test_run_simulation_steps() {
        let mut sim = LogisticsSimulation::new();

        // Add some packages
        sim.create_package(0, 0, 1.0, PackagePriority::Standard)
            .unwrap();
        sim.create_package(0, 1, 1.5, PackagePriority::Express)
            .unwrap();

        // Run for 1 hour (120 steps)
        let result = sim.run_for_hours(1);
        assert!(result.is_ok());
    }

    #[test]
    fn test_metrics_tracking() {
        let mut sim = LogisticsSimulation::new();

        assert_eq!(sim.get_metrics().packages_created, 0);

        sim.create_package(0, 0, 1.0, PackagePriority::Standard)
            .unwrap();
        assert_eq!(sim.get_metrics().packages_created, 1);

        sim.create_package(0, 1, 2.0, PackagePriority::Express)
            .unwrap();
        assert_eq!(sim.get_metrics().packages_created, 2);
    }

    #[test]
    fn test_default_construction() {
        let sim = LogisticsSimulation::default();
        assert_eq!(sim.warehouse_count(), 2);
        assert_eq!(sim.driver_count(), 20);
    }

    #[test]
    fn test_multiple_simulation_steps() {
        let mut sim = LogisticsSimulation::new();

        for _ in 0..10 {
            let result = sim.step();
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_simulation_with_multiple_packages() {
        let mut sim = LogisticsSimulation::new();

        // Create 10 packages
        for i in 0..10 {
            sim.create_package(
                i % 2,
                i * 10,
                1.0 + i as f64 * 0.1,
                PackagePriority::Standard,
            )
            .unwrap();
        }

        assert_eq!(sim.package_count(), 10);
        assert_eq!(sim.get_metrics().packages_created, 10);

        // Run simulation
        sim.run_for_hours(1).unwrap();
    }

    #[test]
    fn test_warehouse_locations() {
        let sim = LogisticsSimulation::new();

        assert_eq!(sim.warehouses[0].name, "Warehouse North");
        assert_eq!(sim.warehouses[1].name, "Warehouse South");
    }
}
