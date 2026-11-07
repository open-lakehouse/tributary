//! Logistics simulation
//!
//! [`LogisticsSimulation`] ships packages from warehouses to delivery
//! addresses on the sim-core framework:
//!
//! - A Poisson [`DemandGenerator`](sim_core::DemandGenerator) creates packages
//!   (10 per hour); [`LogisticsSimulation::create_package`] adds more.
//! - Each package is a [`PackageShipmentWorkflow`] instance whose five steps
//!   are [`ShipmentTask`]s allocated to [`Station`]s: receivers, sorting
//!   stations, loading docks, drivers (one package at a time each) and
//!   delivery confirmers.
//! - Each package is also a passive [`Package`] agent whose stage follows its
//!   workflow, so the population shows where every package is.
//!
//! # Events
//!
//! ```text
//! receive step done    → io.logistics.package.received
//! sort step done       → io.logistics.package.sorted
//! driver allocated     → io.logistics.package.loaded, io.logistics.package.in_transit
//! confirm step done    → io.logistics.package.delivered
//! ```

use crate::locations::{DeliveryAddress, Warehouse};
use crate::resources::{Station, StationKind};
use crate::workflows::{PackageShipmentWorkflow, ShipmentTask, ShipmentTaskFactory};
use crate::{Package, PackagePriority, PackageStage, events::*};
use chrono::{DateTime, Duration, Utc};
use geo::Point;
use rand::{RngExt, SeedableRng, rngs::StdRng};
use sim_core::{
    AgentId, PoissonDemandGenerator, PopulationHandler, ResourceAllocator, ResourceId,
    SimulationConfig, SimulationContext, SimulationHooks, SimulationRunner, Workflow,
    WorkflowRegistry,
    population::Demand,
    resources::Assignment,
    utils::idents::WorkflowInstanceId,
    workflows::{WorkflowEngine, WorkflowStepEvent},
};
use sim_events::{Event, EventDispatcher};
use std::collections::{HashMap, HashSet};

/// The runner type behind [`LogisticsSimulation`]
pub type LogisticsRunner =
    SimulationRunner<Package, Station, ShipmentTask, PackageShipmentWorkflow>;

/// Name of the workflow engine holding [`PackageShipmentWorkflow`]
pub const SHIPMENT_ENGINE: &str = "package_shipment";

/// Simulation step length
const TIME_STEP_SECS: i64 = 30;

/// Packages created per hour by the demand generator
const PACKAGES_PER_HOUR: f64 = 10.0;

/// Size of the driver fleet
const NUM_DRIVERS: usize = 20;

/// Logistics simulation metrics
#[derive(Debug, Clone, Default)]
pub struct LogisticsMetrics {
    pub packages_created: u64,
    pub packages_delivered: u64,
    /// Distinct drivers that have carried at least one package
    pub drivers_used: u64,
    /// Mean time from package creation to delivery
    pub average_delivery_time_secs: f64,
    /// Mean share of drivers carrying a package, over all steps
    pub driver_utilization_percent: f64,
}

/// A package's shipment while its workflow runs
#[derive(Debug)]
struct Shipment {
    package: AgentId,
    tracking_number: String,
    warehouse: String,
    destination: Point<f64>,
    created_at: DateTime<Utc>,
    /// Station that handled each step, filled in as steps are allocated
    stations: Vec<ResourceId>,
}

/// A package created this tick, waiting for its workflow instance
#[derive(Debug)]
struct NewPackage {
    package: Package,
    created_at: DateTime<Utc>,
}

/// Hooks that track shipments, emit logistics events and collect metrics
///
/// Hook state is not part of runner snapshots; after a restore, shipments the
/// hooks don't know about complete without events.
pub struct LogisticsSimulationHooks {
    dispatcher: EventDispatcher,
    rng: StdRng,
    warehouses: Vec<(String, Point<f64>)>,
    addresses: Vec<Point<f64>>,
    station_names: HashMap<ResourceId, String>,
    shipments: HashMap<WorkflowInstanceId, Shipment>,
    new_packages: Vec<NewPackage>,
    stage_updates: Vec<(AgentId, PackageStage)>,
    finished: Vec<WorkflowInstanceId>,
    package_counter: u64,
    packages_created: u64,
    packages_delivered: u64,
    delivery_secs: Vec<f64>,
    total_drivers: usize,
    busy_drivers: usize,
    drivers_used: HashSet<ResourceId>,
    utilization_samples: Vec<f64>,
}

impl LogisticsSimulationHooks {
    fn new(
        warehouses: Vec<(String, Point<f64>)>,
        addresses: Vec<Point<f64>>,
        stations: &[Station],
        rng: StdRng,
    ) -> Self {
        Self {
            dispatcher: EventDispatcher::new(),
            rng,
            warehouses,
            addresses,
            station_names: stations.iter().map(|s| (s.id, s.name.clone())).collect(),
            shipments: HashMap::new(),
            new_packages: Vec::new(),
            stage_updates: Vec::new(),
            finished: Vec::new(),
            package_counter: 0,
            packages_created: 0,
            packages_delivered: 0,
            delivery_secs: Vec::new(),
            total_drivers: stations
                .iter()
                .filter(|s| s.kind == StationKind::Driver)
                .count(),
            busy_drivers: 0,
            drivers_used: HashSet::new(),
            utilization_samples: Vec::new(),
        }
    }

    /// All events emitted so far
    pub fn dispatcher(&self) -> &EventDispatcher {
        &self.dispatcher
    }

    /// Metrics so far
    pub fn metrics(&self) -> LogisticsMetrics {
        let mean = |v: &[f64]| {
            if v.is_empty() {
                0.0
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };
        LogisticsMetrics {
            packages_created: self.packages_created,
            packages_delivered: self.packages_delivered,
            drivers_used: self.drivers_used.len() as u64,
            average_delivery_time_secs: mean(&self.delivery_secs),
            driver_utilization_percent: mean(&self.utilization_samples) * 100.0,
        }
    }

    fn enqueue(&mut self, package: Package, created_at: DateTime<Utc>) {
        self.packages_created += 1;
        self.new_packages.push(NewPackage {
            package,
            created_at,
        });
    }

    fn next_tracking_number(&mut self) -> String {
        self.package_counter += 1;
        format!("PKG-{:06}", self.package_counter)
    }

    fn station_name(&self, id: ResourceId) -> String {
        self.station_names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| id.to_string())
    }
}

impl SimulationHooks for LogisticsSimulationHooks {
    fn on_demands_generated(
        &mut self,
        ctx: &SimulationContext,
        demands: &[Demand],
    ) -> sim_core::Result<()> {
        if self.warehouses.is_empty() || self.addresses.is_empty() {
            return Ok(());
        }
        for _ in demands {
            let (warehouse, origin) =
                self.warehouses[self.rng.random_range(0..self.warehouses.len())].clone();
            let destination = self.addresses[self.rng.random_range(0..self.addresses.len())];
            let tracking = self.next_tracking_number();
            let package = Package::new(
                tracking,
                warehouse,
                origin,
                destination,
                1.0,
                PackagePriority::Standard,
            );
            self.enqueue(package, ctx.current_time());
        }
        Ok(())
    }

    fn on_resources_allocated(
        &mut self,
        ctx: &SimulationContext,
        assignments: &[Assignment],
    ) -> sim_core::Result<()> {
        let now = ctx.current_time();
        for a in assignments {
            let Some(instance_id) = a.workflow_instance_id else {
                continue;
            };
            let Some(shipment) = self.shipments.get_mut(&instance_id) else {
                continue;
            };
            // Steps run in order, so this is the step being allocated.
            let step = shipment.stations.len();
            shipment.stations.push(a.resource_id);
            if step != PackageShipmentWorkflow::TRANSIT_STEP {
                continue;
            }

            self.busy_drivers += 1;
            self.drivers_used.insert(a.resource_id);
            let package_id = shipment.tracking_number.clone();
            let dock = shipment.stations[PackageShipmentWorkflow::TRANSIT_STEP - 1];
            let package = shipment.package;
            let (driver, dock) = (self.station_name(a.resource_id), self.station_name(dock));
            self.dispatcher.emit(Event::new(
                Box::new(PackageLoadedEvent {
                    package_id: package_id.clone(),
                    driver_id: driver.clone(),
                    loading_dock_id: dock,
                }),
                now,
            ));
            self.dispatcher.emit(Event::new(
                Box::new(PackageInTransitEvent {
                    package_id,
                    driver_id: driver,
                    estimated_arrival: now
                        + PackageShipmentWorkflow::step_duration(
                            PackageShipmentWorkflow::TRANSIT_STEP,
                        ),
                }),
                now,
            ));
            self.stage_updates.push((package, PackageStage::InTransit));
        }
        Ok(())
    }

    fn on_workflow_step_completed(
        &mut self,
        _ctx: &SimulationContext,
        event: &WorkflowStepEvent,
    ) -> sim_core::Result<()> {
        match event {
            WorkflowStepEvent::StepCompleted {
                instance_id,
                step_index,
                completion_time,
            } => {
                let Some(shipment) = self.shipments.get(instance_id) else {
                    return Ok(());
                };
                let package = shipment.package;
                match *step_index {
                    0 => {
                        let evt = PackageReceivedEvent {
                            package_id: shipment.tracking_number.clone(),
                            warehouse_id: shipment.warehouse.clone(),
                            destination: shipment.destination,
                            timestamp: *completion_time,
                        };
                        self.dispatcher
                            .emit(Event::new(Box::new(evt), *completion_time));
                        self.stage_updates.push((package, PackageStage::Received));
                    }
                    1 => {
                        let evt = PackageSortedEvent {
                            package_id: shipment.tracking_number.clone(),
                            sorting_station_id: self.station_name(shipment.stations[1]),
                            warehouse_id: shipment.warehouse.clone(),
                        };
                        self.dispatcher
                            .emit(Event::new(Box::new(evt), *completion_time));
                        self.stage_updates.push((package, PackageStage::Sorted));
                    }
                    PackageShipmentWorkflow::TRANSIT_STEP => {
                        self.busy_drivers = self.busy_drivers.saturating_sub(1);
                    }
                    _ => {}
                }
            }
            WorkflowStepEvent::WorkflowCompleted {
                instance_id,
                completion_time,
                ..
            } => {
                let Some(shipment) = self.shipments.remove(instance_id) else {
                    return Ok(());
                };
                self.packages_delivered += 1;
                self.delivery_secs
                    .push((*completion_time - shipment.created_at).num_seconds() as f64);
                let driver =
                    self.station_name(shipment.stations[PackageShipmentWorkflow::TRANSIT_STEP]);
                self.dispatcher.emit(Event::new(
                    Box::new(PackageDeliveredEvent {
                        package_id: shipment.tracking_number,
                        driver_id: driver,
                        delivery_time: *completion_time,
                        destination: shipment.destination,
                    }),
                    *completion_time,
                ));
                self.stage_updates
                    .push((shipment.package, PackageStage::Delivered));
                self.finished.push(*instance_id);
            }
        }
        Ok(())
    }

    fn on_after_tick(&mut self, _ctx: &SimulationContext) -> sim_core::Result<()> {
        if self.total_drivers > 0 {
            self.utilization_samples
                .push(self.busy_drivers as f64 / self.total_drivers as f64);
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

/// Package delivery simulation
///
/// Two warehouses, 1000 delivery addresses in the Bay Area and a fleet of 20
/// drivers. Time advances 30 s per step. Drive it with
/// [`step`](Self::step) or [`run_for_hours`](Self::run_for_hours): package
/// workflows start and package stages update in `step`, around the
/// framework tick.
pub struct LogisticsSimulation {
    runner: LogisticsRunner,
    factory: ShipmentTaskFactory,
    warehouses: Vec<Warehouse>,
    delivery_addresses: Vec<DeliveryAddress>,
}

impl LogisticsSimulation {
    /// Default setup, unseeded
    pub fn new() -> Self {
        Self::new_seeded(None)
    }

    /// Default setup with an optional global seed
    ///
    /// With a seed, the address layout, package demand and package routing
    /// are reproducible.
    pub fn new_seeded(seed: Option<u64>) -> Self {
        let rng = |label: &str| match seed {
            Some(global) => StdRng::seed_from_u64(sim_core::derive_seed(global, label)),
            None => StdRng::from_seed(rand::random()),
        };

        let config = SimulationConfig {
            name: "logistics_simulation".to_string(),
            time_step: Duration::seconds(TIME_STEP_SECS),
            seed,
            ..Default::default()
        };
        let mut runner = LogisticsRunner::new(config);

        let warehouses = vec![
            Warehouse::new(
                "Warehouse North".to_string(),
                Point::new(-122.4, 37.8),
                1000,
            )
            .expect("valid coordinates"),
            Warehouse::new(
                "Warehouse South".to_string(),
                Point::new(-122.3, 37.7),
                1000,
            )
            .expect("valid coordinates"),
        ];
        let mut address_rng = rng("logistics-addresses");
        let delivery_addresses: Vec<DeliveryAddress> = (0..1000)
            .map(|i| {
                let lat = 37.7 + address_rng.random::<f64>() * 0.2;
                let lon = -122.5 + address_rng.random::<f64>() * 0.3;
                DeliveryAddress::new(Point::new(lon, lat), format!("Customer-{i}"))
                    .expect("valid coordinates")
            })
            .collect();

        let mut stations = vec![
            Station::new("Receiving", StationKind::Receiver, 10),
            Station::new("Sorting-A", StationKind::Sorting, 5),
            Station::new("Sorting-B", StationKind::Sorting, 5),
            Station::new("Dock-1", StationKind::LoadingDock, 4),
            Station::new("Dock-2", StationKind::LoadingDock, 4),
            Station::new("Confirmation", StationKind::DeliveryConfirmer, 10),
        ];
        stations.extend(
            (1..=NUM_DRIVERS).map(|i| Station::new(format!("Driver-{i}"), StationKind::Driver, 1)),
        );

        let hooks = LogisticsSimulationHooks::new(
            warehouses
                .iter()
                .map(|w| (w.name.clone(), w.location))
                .collect(),
            delivery_addresses.iter().map(|a| a.location).collect(),
            &stations,
            rng("logistics-hooks"),
        );
        runner.set_hooks(Box::new(hooks));

        let mut allocator = ResourceAllocator::new();
        allocator
            .add_resources(stations)
            .expect("station names are unique");
        runner.set_resource_allocator(allocator);

        let mut engine = WorkflowEngine::new();
        engine.register_workflow(PackageShipmentWorkflow::new());
        let mut registry = WorkflowRegistry::new();
        registry
            .register(SHIPMENT_ENGINE.to_string(), engine)
            .expect("fresh registry");
        runner.set_workflow_registry(registry);

        let factory = ShipmentTaskFactory::new();
        runner.set_task_factory(Box::new(factory.clone()));
        let demand = match seed {
            Some(global) => PoissonDemandGenerator::with_seed(
                PACKAGES_PER_HOUR,
                "package_shipment",
                sim_core::derive_seed(global, "logistics-demand"),
            ),
            None => PoissonDemandGenerator::new(PACKAGES_PER_HOUR, "package_shipment"),
        };
        runner.set_demand_generator(Box::new(demand));
        runner.set_population_handler(PopulationHandler::new());

        Self {
            runner,
            factory,
            warehouses,
            delivery_addresses,
        }
    }

    /// Run for `hours` of simulated time
    pub fn run_for_hours(&mut self, hours: u64) -> crate::Result<()> {
        let steps = hours * 3600 / TIME_STEP_SECS as u64;
        for _ in 0..steps {
            self.step()?;
        }
        Ok(())
    }

    /// Execute one simulation step
    ///
    /// Ticks the runner, then starts shipments for packages created this
    /// tick and moves packages to their new stages.
    pub fn step(&mut self) -> crate::Result<sim_core::TickResult> {
        let tick = self.runner.tick()?;
        self.start_new_shipments()?;
        self.apply_stage_updates()?;
        Ok(tick)
    }

    /// Ship a package from warehouse `warehouse_idx` to address `destination_idx`
    ///
    /// The shipment starts on the next step.
    pub fn create_package(
        &mut self,
        warehouse_idx: usize,
        destination_idx: usize,
        weight_kg: f64,
        priority: PackagePriority,
    ) -> crate::Result<()> {
        let warehouse = self
            .warehouses
            .get(warehouse_idx)
            .ok_or_else(|| crate::Error::Logistics("Invalid warehouse index".to_string()))?;
        let destination = self
            .delivery_addresses
            .get(destination_idx)
            .ok_or_else(|| crate::Error::Logistics("Invalid destination index".to_string()))?;
        let (name, origin, dest) = (
            warehouse.name.clone(),
            warehouse.location,
            destination.location,
        );
        let now = self.runner.context().current_time();
        let hooks = self.hooks_mut();
        let tracking = hooks.next_tracking_number();
        hooks.enqueue(
            Package::new(tracking, name, origin, dest, weight_kg, priority),
            now,
        );
        Ok(())
    }

    /// Current metrics
    pub fn get_metrics(&self) -> LogisticsMetrics {
        self.hooks().metrics()
    }

    /// Size of the driver fleet
    pub fn driver_count(&self) -> usize {
        self.hooks().total_drivers
    }

    /// Packages created so far, including delivered ones and those whose
    /// shipment starts on the next step
    pub fn package_count(&self) -> usize {
        self.runner.population().map_or(0, |p| p.count()) + self.hooks().new_packages.len()
    }

    /// Number of warehouses
    pub fn warehouse_count(&self) -> usize {
        self.warehouses.len()
    }

    /// Number of delivery addresses
    pub fn delivery_address_count(&self) -> usize {
        self.delivery_addresses.len()
    }

    /// The underlying runner
    pub fn runner(&self) -> &LogisticsRunner {
        &self.runner
    }

    /// The underlying runner, mutably
    pub fn runner_mut(&mut self) -> &mut LogisticsRunner {
        &mut self.runner
    }

    /// Runner statistics
    pub fn runner_stats(&self) -> &sim_core::SimulationStats {
        self.runner.stats()
    }

    /// All events emitted so far, in emission order
    pub fn event_records(&self) -> Vec<sim_events::EventRecord> {
        self.hooks().dispatcher.to_records()
    }

    /// Write all events emitted so far to a Parquet file and clear them
    pub fn save_events(
        &mut self,
        path: impl Into<std::path::PathBuf>,
    ) -> std::result::Result<usize, Box<dyn std::error::Error>> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let records = self.event_records();
        self.hooks_mut().dispatcher.clear();
        let mut writer = sim_events::storage::ParquetEventWriter::new(path)?;
        writer.write_events(&records)?;
        writer.close()?;
        Ok(records.len())
    }

    fn hooks(&self) -> &LogisticsSimulationHooks {
        self.runner
            .hooks_as::<LogisticsSimulationHooks>()
            .expect("LogisticsSimulation installs LogisticsSimulationHooks")
    }

    fn hooks_mut(&mut self) -> &mut LogisticsSimulationHooks {
        self.runner
            .hooks_as_mut::<LogisticsSimulationHooks>()
            .expect("LogisticsSimulation installs LogisticsSimulationHooks")
    }

    fn start_new_shipments(&mut self) -> crate::Result<()> {
        let new = std::mem::take(&mut self.hooks_mut().new_packages);
        if new.is_empty() {
            return Ok(());
        }
        let engine = self
            .runner
            .workflow_registry_mut()
            .and_then(|r| r.get_mut(SHIPMENT_ENGINE))
            .ok_or_else(|| crate::Error::Logistics("shipment engine missing".into()))?;
        let workflow_id = PackageShipmentWorkflow::new().id();
        let mut started = Vec::with_capacity(new.len());
        for NewPackage {
            package,
            created_at,
        } in new
        {
            let instance = engine.start_instance(workflow_id)?;
            self.factory
                .set_priority(instance.id, package.priority.rank());
            started.push((instance.id, package, created_at));
        }

        let population = self
            .runner
            .population_mut()
            .ok_or_else(|| crate::Error::Logistics("no package population".into()))?;
        let mut shipments = Vec::with_capacity(started.len());
        for (instance_id, package, created_at) in started {
            shipments.push((
                instance_id,
                Shipment {
                    package: package.id,
                    tracking_number: package.tracking_number.clone(),
                    warehouse: package.warehouse.clone(),
                    destination: package.destination,
                    created_at,
                    stations: Vec::with_capacity(PackageShipmentWorkflow::STEPS.len()),
                },
            ));
            population.add_agent(package);
        }
        self.hooks_mut().shipments.extend(shipments);
        Ok(())
    }

    fn apply_stage_updates(&mut self) -> crate::Result<()> {
        let hooks = self.hooks_mut();
        let updates = std::mem::take(&mut hooks.stage_updates);
        let finished = std::mem::take(&mut hooks.finished);
        for instance_id in finished {
            self.factory.forget(instance_id);
        }
        if updates.is_empty() {
            return Ok(());
        }
        let population = self
            .runner
            .population_mut()
            .ok_or_else(|| crate::Error::Logistics("no package population".into()))?;
        let mut latest: HashMap<AgentId, PackageStage> = HashMap::new();
        for (id, stage) in updates {
            latest.insert(id, stage);
        }
        let changed: Vec<Package> = latest
            .into_iter()
            .filter_map(|(id, stage)| {
                population.get_agent(id).map(|mut p| {
                    p.stage = stage;
                    p
                })
            })
            .collect();
        population.update_agents(changed)?;
        Ok(())
    }
}

impl Default for LogisticsSimulation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "simulation_tests.rs"]
mod tests;
