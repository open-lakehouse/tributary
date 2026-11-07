# sim-geo

Geospatial building blocks: road networks with routing, location storage with H3
proximity queries, and journeys along routes.

No domain crate uses sim-geo yet. `sim-core` uses it only in integration tests, so the
journey API has no production caller.

## Road networks

`RoadNetwork` is a directed graph of `RoadNode`s and `RoadEdge`s. `add_edge` adds one
direction only. `RoadType` is `Highway`, `Arterial`, `Residential` or `ServiceRoad`.

```rust
use geo_types::{LineString, Point};
use sim_geo::graph::{RoadEdge, RoadNetwork, RoadNode, RoadType};

let mut network = RoadNetwork::new();
network.add_node(RoadNode { id: 1, position: Point::new(-122.42, 37.77) });
network.add_node(RoadNode { id: 2, position: Point::new(-122.41, 37.77) });
network.add_edge(RoadEdge {
    id: 10,
    from_node: 1,
    to_node: 2,
    distance_m: 900.0,
    geometry: LineString::from(vec![(-122.42, 37.77), (-122.41, 37.77)]),
    road_type: RoadType::Arterial,
    speed_limit_mps: 15.0,
});
let route = network.find_shortest_path(1, 2); // Option<Route>, via fast_paths
```

The routing graph is built on first use and dropped by `add_node` and `add_edge`.
`find_nearest_node` snaps a point to the nearest node by great-circle distance. Networks
round-trip through JSON: `from_json`, `to_json`, `load_from_file`, `save_to_file`.

## Locations

Implement `Location` (`id`, `location_type`, `position`; `capacity`, `properties` and
`h3_cell` have defaults). `LocationHandler<L>` keeps locations in an Arrow `RecordBatch`
and indexes them by H3 cell at the resolution passed to `new`.

```rust
use h3o::Resolution;
use sim_geo::LocationHandler;

let mut handler = LocationHandler::new(Resolution::Nine);
handler.add_location(warehouse)?; // any Location implementation
let nearest = handler.nearest(point)?;                  // Option<LocationId>
let within = handler.within_radius(point, 5_000.0)?;    // metres
```

`nearest` searches the query cell and its neighbours, then falls back to a full scan.
`within_radius` searches the H3 rings covering the radius, then filters by exact distance.
For items without a `Location` type, `H3Index<T>` offers the same cell lookup: `insert`,
`get_cell`, and `nearby(point, resolution)`, which returns items in the point's cell and its
ring of neighbours with no distance filter.

## Journeys

`MovementHandler` plans and advances journeys. Routes follow the network when one is
loaded, otherwise they are straight lines. Progress is a fraction of route length.

```rust
use sim_geo::{MovementHandler, TimeOfDayVelocity};

let mut movement = MovementHandler::new()
    .with_network(network)
    .with_velocity_model(Box::new(TimeOfDayVelocity::default().with_utc_offset(-8)));
let journey = movement.plan_journey(agent_id, origin, destination, start_time)?;
movement.add_journey(journey);
movement.advance_journeys(now, Duration::seconds(60))?; // now = start of this step
let arrived = movement.remove_completed();              // progress >= 1.0
```

The default velocity model is `ConstantVelocity` (5 m/s). `TimeOfDayVelocity` defaults to
multipliers of 0.5 during 07:00-08:59 and 17:00-18:59, 1.2 during 22:00-04:59, and 1.0
otherwise, in the local time given by `with_utc_offset`. The multipliers are public fields.
Implement `VelocityModel` for other models.

Examples: `cargo run -p sim-geo --example road_network_demo` and
`--example network_routing_comparison`. Benchmark: `cargo bench -p sim-geo`.
API reference: `cargo doc -p sim-geo --open`.
