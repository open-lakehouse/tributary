# Custom Resources Guide

This guide covers implementing the `Resource` trait, task processing patterns, state transitions, and testing strategies for capacity-constrained processors.

## Table of Contents

1. [Resource Trait Overview](#resource-trait-overview)
2. [Task Processing Patterns](#task-processing-patterns)
3. [State Transitions](#state-transitions)
4. [Testing Strategies](#testing-strategies)
5. [Examples from sim-logistics](#examples-from-sim-logistics)

## Resource Trait Overview

The `Resource` trait represents capacity-constrained processors. From `/crates/sim-core/src/resources/traits.rs`:

```rust
pub trait Resource: Send + Sync + fmt::Debug {
    fn id(&self) -> ResourceId;
    fn resource_type(&self) -> &str;
    fn capacity(&self) -> usize;
    fn current_load(&self) -> usize;
    fn is_available(&self) -> bool;
    fn available_capacity(&self) -> usize;
}
```

### Core Concepts

- **Capacity**: Maximum concurrent tasks the resource can handle
- **Load**: Current number of active tasks
- **Availability**: Whether resource can accept more tasks
- **Processing**: Moving tasks from queue to completion

## Task Processing Patterns

### Pattern 1: FIFO Queue Processing

Best for sequential task processing with fixed duration.

**Example: SortingStation**

From `/crates/sim-logistics/src/resources/sorting.rs`:

```rust
#[derive(Debug, Clone)]
pub struct SortingStation {
    pub id: ResourceId,
    pub name: String,
    pub capacity: u32,
    pub processing_rate_per_hour: f64,
    pub current_load: u32,
    pub queue: VecDeque<String>, // Package IDs waiting
    pub processing: Vec<(String, u64)>, // (package_id, completion_tick)
    pub total_processed: u64,
}

impl SortingStation {
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

    pub fn enqueue_package(&mut self, package_id: String) -> Result<(), String> {
        // Check total load (processing + queue)
        let total_load = self.current_load + self.queue.len() as u32;
        if total_load >= self.capacity * 3 {
            return Err("Sorting station queue is full".to_string());
        }

        self.queue.push_back(package_id);
        Ok(())
    }

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
                let seconds_per_package = 3600.0 / self.processing_rate_per_hour;
                let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;
                let completion_tick = current_tick + ticks_per_package;

                self.processing.push((package_id, completion_tick));
                self.current_load += 1;
            }
        }
    }

    pub fn get_completed(&self, current_tick: u64) -> Vec<String> {
        self.processing
            .iter()
            .filter(|(_, completion)| *completion == current_tick)
            .map(|(id, _)| id.clone())
            .collect()
    }

    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current_load as f64 / self.capacity as f64) * 100.0
        }
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
```

**Key Elements:**

- **Queue**: `VecDeque<String>` for FIFO ordering
- **Processing List**: `Vec<(String, u64)>` with completion ticks
- **Capacity Check**: Before starting new work
- **Completion Check**: Each tick, remove finished items
- **Utilization Metric**: Current load / capacity * 100

**Timing Calculation:**

```rust
// Rate: 12 packages/hour
// Time per package: 3600 / 12 = 300 seconds
// Ticks per package (30 sec/tick): 300 / 30 = 10 ticks
let seconds_per_package = 3600.0 / self.processing_rate_per_hour;
let ticks_per_package = (seconds_per_package / 30.0).ceil() as u64;
let completion_tick = current_tick + ticks_per_package;
```

### Pattern 2: Slot-Based Processing

Best for resources with discrete processing slots (e.g., loading docks, checkout lanes).

**Example: LoadingDock**

From `/crates/sim-logistics/src/resources/loading.rs`:

```rust
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

    pub fn has_available_slot(&self) -> bool {
        self.active_vehicles.len() < self.vehicle_slots as usize
    }

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
                // Would need package info stored with waiting vehicles
            }
        }

        results
    }

    pub fn get_vehicle_progress(&self, driver_id: &str) -> Option<(usize, usize)> {
        self.active_vehicles.get(driver_id).map(|state| {
            (state.packages_loaded, state.packages_to_load.len())
        })
    }

    pub fn is_vehicle_loading(&self, driver_id: &str) -> bool {
        self.active_vehicles.contains_key(driver_id)
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
```

**Key Elements:**

- **Slot Map**: `HashMap<String, SlotState>` for active slots
- **Waiting Queue**: `VecDeque<String>` for overflow
- **Per-Slot State**: Track progress of each slot independently
- **Incremental Progress**: Load one item at a time per slot
- **Completion Tracking**: Remove when all items loaded

### Pattern 3: Priority Queue Processing

Best for resources that process tasks based on priority.

```rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;

#[derive(Debug, Clone)]
struct PriorityTask {
    id: String,
    priority: i32,
    completion_tick: u64,
}

impl Ord for PriorityTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority first, then earlier completion
        self.priority.cmp(&other.priority)
            .then_with(|| other.completion_tick.cmp(&self.completion_tick))
    }
}

impl PartialOrd for PriorityTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for PriorityTask {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PriorityTask {}

#[derive(Debug, Clone)]
pub struct PriorityProcessor {
    pub id: ResourceId,
    pub capacity: u32,
    pub current_load: u32,
    pub queue: BinaryHeap<PriorityTask>,
    pub processing: Vec<PriorityTask>,
}

impl PriorityProcessor {
    pub fn enqueue(&mut self, id: String, priority: i32) {
        self.queue.push(PriorityTask {
            id,
            priority,
            completion_tick: 0, // Will be set when processing starts
        });
    }

    pub fn process_tick(&mut self, current_tick: u64) {
        // Complete finished tasks
        self.processing.retain(|task| {
            if task.completion_tick <= current_tick {
                self.current_load -= 1;
                false
            } else {
                true
            }
        });

        // Start highest priority tasks
        while self.current_load < self.capacity && !self.queue.is_empty() {
            if let Some(mut task) = self.queue.pop() {
                task.completion_tick = current_tick + 10; // Fixed duration
                self.processing.push(task);
                self.current_load += 1;
            }
        }
    }
}
```

**Key Elements:**

- **Priority Heap**: `BinaryHeap` for priority ordering
- **Custom Ordering**: Implement `Ord` for priority logic
- **Dual Priority**: Primary and secondary sort keys

### Pattern 4: Batch Processing

Best for resources that process items in batches.

```rust
#[derive(Debug, Clone)]
pub struct BatchProcessor {
    pub id: ResourceId,
    pub batch_size: usize,
    pub processing_time_ticks: u64,
    pub current_batch: Vec<String>,
    pub batch_started_at: Option<u64>,
    pub queue: VecDeque<String>,
}

impl BatchProcessor {
    pub fn enqueue(&mut self, item_id: String) {
        self.queue.push_back(item_id);
    }

    pub fn process_tick(&mut self, current_tick: u64) {
        // Check if current batch is complete
        if let Some(started) = self.batch_started_at {
            if current_tick >= started + self.processing_time_ticks {
                // Batch complete
                self.current_batch.clear();
                self.batch_started_at = None;
            }
        }

        // Start new batch if idle and enough items
        if self.batch_started_at.is_none() && self.queue.len() >= self.batch_size {
            for _ in 0..self.batch_size {
                if let Some(item) = self.queue.pop_front() {
                    self.current_batch.push(item);
                }
            }
            self.batch_started_at = Some(current_tick);
        }
    }

    pub fn is_processing(&self) -> bool {
        self.batch_started_at.is_some()
    }
}
```

**Key Elements:**

- **Batch Collection**: Wait for minimum batch size
- **Atomic Processing**: Process entire batch together
- **Batch Timing**: Start time + duration = completion

## State Transitions

### State Transition Diagram

Resources typically follow this flow:

```
┌──────────────────┐
│  Item Arrives    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Add to Queue   │
└────────┬─────────┘
         │
         ▼
    ┌───────┐
    │ Queue │◄───────┐
    └───┬───┘        │
        │            │
        │ Has        │ No
        │ Capacity?  │ Capacity
        │            │
        ▼            │
┌──────────────────┐│
│ Start Processing ││
└────────┬─────────┘│
         │          │
         ▼          │
┌──────────────────┐│
│   Processing     ││
└────────┬─────────┘│
         │          │
         │ Complete │
         ▼          │
┌──────────────────┐│
│  Remove & Free   │┘
│    Capacity      │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Item Exits     │
└──────────────────┘
```

### Implementation Pattern

```rust
pub fn process_tick(&mut self, current_tick: u64) {
    // STEP 1: Complete finished items
    self.processing.retain(|(item_id, completion)| {
        if *completion <= current_tick {
            self.current_load -= 1;
            self.total_processed += 1;
            self.emit_completion_event(item_id);
            false // Remove
        } else {
            true // Keep
        }
    });

    // STEP 2: Start new items from queue
    while self.has_capacity() && !self.queue.is_empty() {
        let item = self.queue.pop_front().unwrap();
        let completion = self.calculate_completion_time(current_tick);
        self.processing.push((item, completion));
        self.current_load += 1;
        self.emit_start_event(&item);
    }

    // STEP 3: Update metrics
    self.update_utilization_metrics();
}
```

## Testing Strategies

### Unit Test Structure

From `/crates/sim-logistics/src/resources/sorting.rs`:

```rust
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
```

### Test Categories

**1. Creation Tests**

- Verify initial state
- Check default values
- Test constructor variants

**2. Queue Management Tests**

- Test enqueue operations
- Verify queue limits
- Check FIFO ordering

**3. Processing Tests**

- Test process_tick() behavior
- Verify capacity constraints
- Check completion logic

**4. Timing Tests**

- Verify processing durations
- Test tick-based timing
- Check completion times

**5. Utilization Tests**

- Test metrics calculation
- Verify percentage values
- Check edge cases (0%, 100%)

**6. Trait Implementation Tests**

- Verify Resource trait methods
- Check is_available() logic
- Test available_capacity()

## Examples from sim-logistics

### SortingStation (FIFO Processing)

**Location:** `/crates/sim-logistics/src/resources/sorting.rs`

**Characteristics:**

- FIFO queue processing
- Fixed processing rate (12 packages/hour)
- Capacity limit (10 concurrent)
- Queue overflow protection (3x capacity)

**Key Features:**

- `process_tick()` - Main processing loop
- `get_completed()` - Query completion status
- `utilization()` - Performance metric

### LoadingDock (Slot-Based Processing)

**Location:** `/crates/sim-logistics/src/resources/loading.rs`

**Characteristics:**

- Slot-based allocation (2 vehicle slots)
- Per-slot progress tracking
- Incremental loading (5 packages/hour per vehicle)
- Waiting queue for overflow

**Key Features:**

- `assign_vehicle()` - Slot allocation
- `get_vehicle_progress()` - Progress query
- `process_tick()` - Per-slot processing

## Best Practices

1. **Separation of Concerns**: Keep queue, processing, and completion logic separate
2. **Timing**: Use tick counts, convert rates to ticks
3. **Capacity**: Enforce strict capacity limits
4. **Metrics**: Track utilization, throughput, wait times
5. **Testing**: Test at 0%, 50%, 100% capacity
6. **Documentation**: Document processing algorithm
7. **Error Handling**: Return Result for operations that can fail
8. **State Queries**: Provide methods to inspect resource state

## Next Steps

- Read [Custom Workflows Guide](./custom-workflows.md) for workflow integration
- Read [Testing Strategies](./testing-strategies.md) for comprehensive testing
- Read [Performance Tuning](./performance-tuning.md) for optimization
