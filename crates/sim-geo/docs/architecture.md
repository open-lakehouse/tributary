# sim-geo Architecture

## Design Principles

### 1. Routing Performance First
Fast routing is critical for large-scale simulations. The crate uses Contraction Hierarchies (CH) for sub-millisecond queries on city-scale networks.

### 2. Spatial Indexing via H3
Uber's H3 hexagonal grid system provides efficient proximity queries without complex tree structures.

### 3. Pluggable Velocity Models
Speed calculations are abstracted behind the `VelocityModel` trait, enabling time-of-day traffic patterns, weather effects, and more.

### 4. Arrow-Native Geospatial Data
Integration with GeoArrow enables efficient storage and querying of geospatial data.

## Module Organization

```
sim-geo/
├── graph/             Road network representation
│   ├── network.rs     RoadNetwork, nodes, edges
│   ├── routing.rs     Fast path routing (CH)
│   └── osm.rs         OSM PBF parsing
│
├── locations/         Location management
│   ├── location.rs    Location trait
│   ├── handler.rs     LocationHandler with H3 index
│   └── geoarrow.rs    Arrow integration
│
├── movement/          Journey tracking
│   ├── journey.rs     Journey definition and state
│   ├── handler.rs     MovementHandler
│   └── velocity/      Velocity models
│       ├── constant.rs
│       └── time_of_day.rs
│
└── spatial/           H3 indexing
    └── h3_index.rs    Generic H3 spatial index
```

## Key Abstractions

### Location
Represents any position-based entity in the simulation.

```rust
pub trait Location: Send + Sync + Debug {
    fn id(&self) -> LocationId;
    fn location_type(&self) -> &str;
    fn position(&self) -> Point<f64>;
    fn capacity(&self) -> Option<usize> { None }
}
```

**Design Decision**: Minimal required interface. Extensions via optional methods.

**Trade-off**: Flexibility vs type safety.

### RoadNetwork
Graph representation of a road network with routing capabilities.

**Key Features**:
- Fast path routing via Contraction Hierarchies
- Bidirectional Dijkstra fallback
- OSM PBF ingestion
- Node/edge serialization

**Design Decision**: Separate preparation (CH construction) from querying.

**Trade-off**: Upfront cost for query performance.

### MovementHandler
Manages active journeys with time-based progression.

**Responsibilities**:
- Start/cancel journeys
- Advance journeys by time delta
- Track completion
- Calculate current positions

**Design Decision**: Handler manages all journeys centrally.

**Trade-off**: Centralized vs distributed journey state.

## Routing Performance

### Contraction Hierarchies

**Preparation Phase** (one-time):
1. Contract nodes in importance order
2. Add shortcut edges
3. Build query graph
Complexity: O(n log n + m)

**Query Phase** (per route):
1. Bidirectional search on contracted graph
2. Unpack shortcuts to recover path
Complexity: O(log n)

**Results**: Sub-millisecond queries on 100k+ node networks.

### Memory Optimization

**Edge Storage**:
- Compact representation: node IDs + distance
- Shared road type enum (4 bytes)
- No redundant coordinate storage

**Typical Memory**:
- 10k edges: ~1 MB
- 100k edges: ~10 MB
- 1M edges: ~100 MB

## H3 Spatial Indexing

### Why H3?

1. **Uniform distribution**: Hexagons have consistent neighbor counts
2. **Hierarchical**: Multi-resolution support (0-15)
3. **Fast lookups**: O(1) cell assignment
4. **No tree structure**: Simpler than R-trees or KD-trees

### Resolution Selection

| Resolution | Cell Area | Use Case |
|-----------|-----------|----------|
| 5 | ~252 km² | Country-level regions |
| 7 | ~5.16 km² | Neighborhoods |
| 9 | ~0.105 km² | City blocks (default) |
| 11 | ~4,250 m² | Building-level precision |
| 13 | ~43 m² | High-precision tracking |

**Guidance**: Use resolution 9 for urban simulations, 7 for large areas.

### H3Index Operations

```rust
pub struct H3Index<T> {
    cells: HashMap<H3Cell, Vec<T>>,
    resolution: Resolution,
}

// O(1) insertion
index.insert(cell, item);

// O(1) retrieval
let items = index.get(cell);

// O(k) neighbor query (k = neighbor count)
let nearby = index.nearby(point, resolution);
```

## Velocity Models

### ConstantVelocity
Fixed speed regardless of time or conditions.

**Use case**: Baseline, simple scenarios.
**Complexity**: O(1)

### TimeOfDayVelocity
Speed multipliers based on hour of day.

**Use case**: Traffic patterns (rush hour, off-peak).
**Configuration**: HashMap<hour, multiplier>
**Complexity**: O(1) with hash lookup

### Custom Models
Implement `VelocityModel` trait:

```rust
pub trait VelocityModel: Send + Sync {
    fn speed(&self, time: DateTime<Utc>, road_type: RoadType) -> f64;
}
```

**Example**: Weather-dependent speed
```rust
struct WeatherVelocity {
    base_speed: f64,
    weather_data: WeatherService,
}

impl VelocityModel for WeatherVelocity {
    fn speed(&self, time: DateTime<Utc>, road_type: RoadType) -> f64 {
        let weather = self.weather_data.get(time);
        let penalty = match weather {
            Weather::Rain => 0.8,   // 20% slower
            Weather::Snow => 0.5,   // 50% slower
            _ => 1.0,
        };
        self.base_speed * penalty
    }
}
```

## Journey State Machine

```
NotStarted -> InProgress -> Completed
                  |
                  v
              Cancelled
```

**State Transitions**:
- `NotStarted` -> `InProgress`: Journey starts
- `InProgress` -> `Completed`: Distance covered >= total distance
- `InProgress` -> `Cancelled`: Explicit cancellation
- `Completed`/`Cancelled`: Terminal states

**Progress Tracking**:
```rust
pub struct Journey {
    pub origin: Point<f64>,
    pub destination: Point<f64>,
    pub waypoints: Vec<Point<f64>>,  // Route path
    pub distance_covered: f64,       // Meters traveled
    pub total_distance: f64,         // Total journey distance
    pub state: JourneyState,
}
```

## Integration with sim-core

### Pattern: Journeys from Agent Ticks

```rust
impl Agent for MyAgent {
    fn tick(&mut self, ctx: &SimulationContext) -> Result<AgentTickResult> {
        let mut result = AgentTickResult::default();

        if self.should_move() {
            result.journeys.push(Journey {
                origin: self.position,
                destination: self.destination,
                ..Default::default()
            });
        }

        Ok(result)
    }
}

// In hooks
impl SimulationHooks for GeoHooks {
    fn on_agents_ticked(
        &mut self,
        ctx: &SimulationContext,
        results: &[AgentTickResult],
    ) -> Result<()> {
        for result in results {
            for journey in &result.journeys {
                self.movement_handler.start_journey(journey.clone())?;
            }
        }

        self.movement_handler.advance_journeys(ctx.time_step())?;
        Ok(())
    }
}
```

## Performance Considerations

### Routing at Scale

**Small networks (<10k nodes)**:
- Dijkstra is sufficient
- No preprocessing needed
- Queries: ~10ms

**Medium networks (10k-100k nodes)**:
- Use Contraction Hierarchies
- Preprocessing: ~1-10 seconds
- Queries: <1ms

**Large networks (>100k nodes)**:
- CH essential for performance
- Preprocessing: ~10-60 seconds
- Queries: <1ms

### H3 Resolution Trade-offs

**Lower resolution (5-7)**:
- Fewer cells, less memory
- Coarser proximity queries
- Faster cell assignment

**Higher resolution (11-13)**:
- More cells, more memory
- Finer proximity queries
- Slightly slower cell assignment

**Recommendation**: Start with resolution 9, profile, and adjust.

### Movement Handler Scalability

**Up to 10k active journeys**: No special optimization needed
**10k-100k journeys**: Consider spatial partitioning
**100k+ journeys**: Parallel processing (future work)

## Design Decisions Summary

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| Contraction Hierarchies | Sub-ms queries at scale | Preprocessing overhead |
| H3 indexing | Simple, fast proximity queries | Fixed hexagonal grid |
| Pluggable velocity models | Domain flexibility | Requires implementation |
| Central MovementHandler | Simplified lifecycle | Single point of coordination |
| GeoArrow integration | Industry-standard geospatial | Conversion overhead |
| OSM as input format | Widely available data | Requires parsing |

## Extension Points

### 1. Custom Location Types
Implement `Location` trait with domain-specific metadata.

### 2. Custom Velocity Models
Implement `VelocityModel` for traffic, weather, vehicle types.

### 3. Custom Routing Metrics
Extend `RoadNetwork` with custom edge weights (time, cost, fuel).

### 4. Multi-Modal Routing
Combine road, rail, pedestrian networks.

### 5. Dynamic Networks
Update edge weights in real-time for traffic incidents.
