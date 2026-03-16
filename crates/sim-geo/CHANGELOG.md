# Changelog

All notable changes to `sim-geo` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.0.1] - 2026-03-16

### Added

#### Location Management
- **LocationHandler**: Generic location storage with H3 spatial indexing
- **Location trait**: Abstraction for position-based entities
- H3 resolution configuration (supports resolutions 0-15)
- Spatial queries:
  - Nearest location by distance
  - Radius search (all locations within distance)
  - H3 cell-based proximity queries
- Location type filtering and categorization
- Capacity tracking for locations

#### Road Network and Routing
- **RoadNetwork**: OSM-based road graph representation
- **RoadNode**: Junction points with coordinates
- **RoadEdge**: Road segments with properties:
  - Length (meters)
  - Road type (Motorway, Primary, Residential, etc.)
  - One-way support
  - Speed limits
- **Fast path routing**: Contraction Hierarchies for O(log n) queries
- Bidirectional Dijkstra fallback
- OSM PBF file parsing and ingestion
- Road network serialization to Parquet
- Node and edge lookups by ID

#### Movement and Journey Tracking
- **MovementHandler**: Journey lifecycle management
- **Journey**: Movement from origin to destination via waypoints
- **JourneyState**: State machine (NotStarted, InProgress, Completed, Cancelled)
- Progress tracking:
  - Distance covered vs total
  - Estimated vs actual duration
  - Current position interpolation
- **VelocityModel trait**: Pluggable speed calculation
  - **ConstantVelocity**: Fixed speed model
  - **TimeOfDayVelocity**: Speed varies by hour (traffic patterns)
- Journey completion detection and callbacks

#### H3 Spatial Indexing
- **H3Index**: Generic spatial index using Uber H3
- Hierarchical hexagonal grid support
- Cell-based indexing for O(1) lookups
- Neighbor cell queries
- Multi-resolution support
- Integration with geo-types Point

#### Arrow/GeoArrow Integration
- GeoArrow array support for locations
- WKB (Well-Known Binary) encoding/decoding
- Point geometry serialization
- Efficient columnar geospatial data

### Performance
- Contraction Hierarchies routing: Sub-millisecond queries on city-scale networks
- H3 index: O(1) cell lookup, O(k) neighbor queries
- Memory-efficient edge storage with compact representations

### Dependencies
- `sim-core 0.0.1` - Core simulation abstractions
- `arrow 57.0` - Columnar data
- `geoarrow 0.7.0` - Geospatial Arrow extensions
- `geo 0.31` - Geospatial algorithms
- `h3o 0.9.3` - H3 hexagonal indexing
- `fast_paths 1.0` - Contraction Hierarchies routing
- `osmpbfreader 0.16` - OSM file parsing
- `dashmap 6.0` - Concurrent hash maps

### Testing
- Unit tests for all components
- Integration tests for routing workflows
- Benchmark suite for routing performance

[0.0.1]: https://github.com/yourusername/repo/releases/tag/v0.0.1
