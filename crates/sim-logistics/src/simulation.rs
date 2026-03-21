//! Logistics simulation orchestration
//!
//! This module provides the main simulation runner and metrics collection,
//! wired up using `SimulationRunner`, `WorkflowRegistry`, and `SimulationHooks`
//! from `sim-core` — the same pattern used by `sim-kitchen`.
//!
//! # Architecture
//!
//! 1. **SimulationRunner**: Core orchestrator (ticks agents, processes workflows,
//!    manages time)
//! 2. **PopulationHandler**: Manages the Driver agent population
//! 3. **ResourceAllocator**: Manages SortingStation resources and ShipmentTask
//!    assignment
//! 4. **WorkflowRegistry**: Registers the `PackageShipmentWorkflow` engine
//! 5. **LogisticsSimulationHooks**: Domain callbacks that track package lifecycle
//!    and metrics
//!
//! # Lifecycle Flow
//!
//! Each simulation step follows this sequence:
//!
//! ```text
//! 1. on_before_tick()
//!    ↓
//! 2. Tick all driver agents
//!    ↓
//! 3. on_agents_ticked()
//!    ↓
//! 4. Tick workflow registry (advance time-based steps)
//!    ↓
//! 5. Allocate tasks to resources
//!    ↓
//! 6. on_resources_allocated() — track assignments
//!    ↓
//! 7. on_after_tick()
//! ```

use crate::locations::Warehouse;
use crate::resources::SortingStation;
use crate::workflows::{PackageShipmentWorkflow, ShipmentTask, ShipmentTaskFactory};
use crate::{DeliveryAddress, Driver, Package, PackagePriority};
use chrono::Duration;
use geo::Point;
use rand::Rng;
use sim_core::{
    PopulationHandler, ResourceAllocator, SimulationConfig, SimulationHooks, SimulationRunner,
    Workflow, WorkflowRegistry,
    population::AgentTickResult,
    resources::{Assignment, LeastLoaded},
    workflows::WorkflowEngine,
};

/// Logistics simulation metrics
#[derive(Debug, Clone, Default)]
pub struct LogisticsMetrics {
    pub packages_created: u64,
    pub packages_delivered: u64,
    pub drivers_used: u64,
    pub average_delivery_time_secs: f64,
    pub driver_utilization_percent: f64,
}

/// Simulation hooks for the logistics domain
///
/// Receives lifecycle callbacks from `SimulationRunner` and tracks package
/// creation, workflow progression, and delivery metrics.
pub struct LogisticsSimulationHooks {
    /// Cumulative metrics
    pub metrics: LogisticsMetrics,
    /// Packages waiting to be registered in the workflow registry.
    /// Drained each step and added as workflow instances by `LogisticsSimulation`.
    pub pending_shipments: Vec<PackageShipmentWorkflow>,
    /// Number of resources allocated (used to track driver utilization)
    allocations_this_run: u64,
    /// Total allocation opportunities
    allocation_opportunities: u64,
}

impl LogisticsSimulationHooks {
    pub fn new() -> Self {
        Self {
            metrics: LogisticsMetrics::default(),
            pending_shipments: Vec::new(),
            allocations_this_run: 0,
            allocation_opportunities: 0,
        }
    }

    /// Drain pending shipment workflows so the simulation loop can register them.
    pub fn drain_pending_shipments(&mut self) -> Vec<PackageShipmentWorkflow> {
        std::mem::take(&mut self.pending_shipments)
    }

    /// Record a new package to be shipped.
    pub fn enqueue_shipment(&mut self, workflow: PackageShipmentWorkflow) {
        self.metrics.packages_created += 1;
        self.pending_shipments.push(workflow);
    }
}

impl Default for LogisticsSimulationHooks {
    fn default() -> Self {
        Self::new()
    }
}

impl SimulationHooks for LogisticsSimulationHooks {
    fn on_agents_ticked(
        &mut self,
        _ctx: &sim_core::SimulationContext,
        _results: &[AgentTickResult],
    ) -> sim_core::Result<()> {
        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        _ctx: &sim_core::SimulationContext,
        assignments: &[Assignment],
    ) -> sim_core::Result<()> {
        self.allocations_this_run += assignments.len() as u64;
        self.allocation_opportunities += 1;
        Ok(())
    }

    fn on_workflow_step_completed(
        &mut self,
        _ctx: &sim_core::SimulationContext,
        event: &sim_core::workflows::WorkflowStepEvent,
    ) -> sim_core::Result<()> {
        use sim_core::workflows::WorkflowStepEvent;
        if let WorkflowStepEvent::WorkflowCompleted { .. } = event {
            self.metrics.packages_delivered += 1;
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Build a `SimulationRunner` wired up for the logistics domain.
///
/// Sets up:
/// - 2 SortingStation resources in a `ResourceAllocator`
/// - A `WorkflowRegistry` with a `PackageShipmentWorkflow` engine
/// - A `ShipmentTaskFactory` as the task factory
/// - `LogisticsSimulationHooks`
///
/// The driver population and delivery addresses are managed by
/// `LogisticsSimulation` directly.
pub fn build_logistics_runner(
    verbose: bool,
) -> SimulationRunner<Driver, SortingStation, ShipmentTask, PackageShipmentWorkflow> {
    let config = SimulationConfig {
        name: "logistics_simulation".to_string(),
        time_step: Duration::seconds(30),
        verbose,
        custom: serde_json::Map::new(),
    };

    let mut runner = SimulationRunner::new(config);

    // Resource allocator — sorting stations handle ShipmentTasks
    let mut allocator: ResourceAllocator<SortingStation, ShipmentTask> = ResourceAllocator::new();
    allocator
        .add_resource(SortingStation::standard("Sorting-A".to_string()))
        .expect("add resource");
    allocator
        .add_resource(SortingStation::standard("Sorting-B".to_string()))
        .expect("add resource");
    runner.set_resource_allocator(allocator);

    // Workflow registry for package shipment workflows
    let mut registry: WorkflowRegistry<PackageShipmentWorkflow> = WorkflowRegistry::new();
    let engine: WorkflowEngine<PackageShipmentWorkflow> = WorkflowEngine::new();
    registry
        .register("package_shipment".to_string(), engine)
        .expect("register workflow engine");
    runner.set_workflow_registry(registry);

    // Task factory — uses a placeholder; real metadata comes per-shipment
    let task_factory =
        ShipmentTaskFactory::new(String::new(), 0.0, String::new(), Point::new(0.0, 0.0));
    runner.set_task_factory(Box::new(task_factory));

    // Domain hooks
    runner.set_hooks(Box::new(LogisticsSimulationHooks::new()));

    runner
}

/// Main logistics simulation
///
/// Orchestrates package delivery simulation with warehouses, drivers, and packages.
/// Uses `SimulationRunner` + `WorkflowRegistry` for consistent simulation lifecycle.
pub struct LogisticsSimulation {
    runner: SimulationRunner<Driver, SortingStation, ShipmentTask, PackageShipmentWorkflow>,
    warehouses: Vec<Warehouse>,
    delivery_addresses: Vec<DeliveryAddress>,
    /// Local package tracking (separate from workflow state)
    packages: PopulationHandler<Package>,
}

impl LogisticsSimulation {
    /// Create a new logistics simulation with default setup
    ///
    /// Initializes:
    /// - 2 warehouses
    /// - 1000 delivery addresses
    /// - 20 drivers
    pub fn new() -> Self {
        let runner = build_logistics_runner(false);

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
        let mut rng = rand::rng();
        let delivery_addresses: Vec<DeliveryAddress> = (0..1000)
            .map(|i| {
                let lat = 37.7 + rng.random::<f64>() * 0.2;
                let lon = -122.5 + rng.random::<f64>() * 0.3;
                DeliveryAddress::new(Point::new(lon, lat), format!("Customer-{}", i)).unwrap()
            })
            .collect();

        // Create drivers (20 drivers at Warehouse North) and attach to runner
        let mut drivers = PopulationHandler::new();
        for _ in 0..20 {
            drivers.add_agent(Driver::new(Point::new(-122.4, 37.8)));
        }

        let mut sim = Self {
            runner,
            warehouses,
            delivery_addresses,
            packages: PopulationHandler::new(),
        };
        sim.runner.set_population_handler(drivers);
        sim
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

    /// Execute one simulation step via `SimulationRunner`.
    ///
    /// 1. Ticks the runner (agents, workflows, time advancement)
    /// 2. Drains pending shipment workflows from hooks and registers them
    /// 3. Allocates tasks to resources
    pub fn step(&mut self) -> crate::Result<()> {
        // Tick agents + workflow registry + advance time (TickResult handled by caller if needed)
        let _ = self.runner.tick()?;

        // Drain pending shipments enqueued via create_package(), register as instances
        let pending = if let Some(hooks) = self.runner.hooks_as_mut::<LogisticsSimulationHooks>() {
            hooks.drain_pending_shipments()
        } else {
            Vec::new()
        };

        if !pending.is_empty()
            && let Some(registry) = self.runner.workflow_registry_mut()
            && let Some(engine) = registry.get_mut("package_shipment")
        {
            for workflow in pending {
                let workflow_id = workflow.id();
                engine.register_workflow(workflow);
                let _ = engine.start_instance(workflow_id);
            }
        }

        // Allocate tasks to sorting stations
        let _ = self.runner.allocate_resources(&LeastLoaded);

        Ok(())
    }

    /// Create a new package and enqueue it for shipment via the workflow.
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

        // Add to local package population for tracking
        let package = Package::new(
            warehouse.location,
            destination.location,
            weight_kg,
            priority,
        );
        self.packages.add_agent(package);

        // Enqueue a workflow for this shipment
        let workflow = PackageShipmentWorkflow::new(
            format!("PKG-{}", self.packages.count()),
            warehouse.id.to_string(),
            destination.location,
            weight_kg,
        );

        if let Some(hooks) = self.runner.hooks_as_mut::<LogisticsSimulationHooks>() {
            hooks.enqueue_shipment(workflow);
        }

        Ok(())
    }

    /// Get current simulation metrics
    pub fn get_metrics(&self) -> &LogisticsMetrics {
        if let Some(hooks) = self.runner.hooks_as::<LogisticsSimulationHooks>() {
            &hooks.metrics
        } else {
            // Should not happen; fall back to a static default
            static DEFAULT: LogisticsMetrics = LogisticsMetrics {
                packages_created: 0,
                packages_delivered: 0,
                drivers_used: 0,
                average_delivery_time_secs: 0.0,
                driver_utilization_percent: 0.0,
            };
            &DEFAULT
        }
    }

    /// Get number of active drivers
    pub fn driver_count(&self) -> usize {
        self.runner.population().map(|p| p.count()).unwrap_or(0)
    }

    /// Get number of packages (local tracking)
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

    /// Get simulation runner stats
    pub fn runner_stats(&self) -> &sim_core::SimulationStats {
        self.runner.stats()
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

    #[test]
    fn test_workflow_registry_wired_up() {
        let mut sim = LogisticsSimulation::new();

        // Create a package — its workflow should be enqueued
        sim.create_package(0, 0, 1.0, PackagePriority::Standard)
            .unwrap();

        // After one step, the pending shipment should be registered and started
        sim.step().unwrap();

        // Runner stats should show at least one step executed
        assert!(sim.runner_stats().total_steps >= 1);
    }

    #[test]
    fn test_runner_stats_advance_with_steps() {
        let mut sim = LogisticsSimulation::new();

        sim.create_package(0, 5, 2.0, PackagePriority::Express)
            .unwrap();

        sim.run_for_hours(1).unwrap();

        // 1 hour = 120 steps
        assert_eq!(sim.runner_stats().total_steps, 120);
    }
}
