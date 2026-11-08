//! Performance benchmarks for sim-geo routing and geospatial operations
//!
//! This benchmark suite validates that routing queries meet performance targets:
//! - Single route query: <1ms (target: <100µs)
//! - Batch 1000 queries: <1s (target: <100ms)
//! - Network loading: <100ms (target: <50ms)
//! - Velocity calculation: <1µs (target: <100ns)
//!
//! Run with: cargo bench -p sim-geo --bench routing_performance

use chrono::Utc;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use geo::Point;
use sim_geo::graph::{RoadNetwork, RoadType};
use sim_geo::movement::velocity::TimeOfDayVelocity;
use std::hint::black_box;

/// Load a medium-sized test network (1024 nodes)
fn load_network() -> RoadNetwork {
    RoadNetwork::load_from_file("test_data/medium_city_graph.json")
        .expect("Failed to load test network")
}

/// Benchmark: Single routing query performance
///
/// Target: <1ms (preferred: <100µs)
///
/// This is the core operation that must be extremely fast for real-time simulation.
fn benchmark_routing_single_query(c: &mut Criterion) {
    let network = load_network();

    // Find valid node IDs from the network
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let start = node_ids[0];
    let goal = node_ids[node_ids.len() / 2]; // Middle node

    c.bench_function("route_single_query", |b| {
        b.iter(|| network.find_shortest_path(black_box(start), black_box(goal)));
    });
}

/// Benchmark: Batch routing queries
///
/// Target for 1000 queries: <1s (preferred: <100ms = 100µs per query)
///
/// Tests throughput for scenarios where many agents plan routes simultaneously.
fn benchmark_routing_batch_queries(c: &mut Criterion) {
    let network = load_network();
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let node_count = node_ids.len();

    for batch_size in [10, 100, 1000] {
        c.bench_with_input(
            BenchmarkId::new("route_batch", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    for i in 0..size {
                        let start = node_ids[i % node_count];
                        let goal = node_ids[(i + node_count / 2) % node_count];
                        black_box(network.find_shortest_path(start, goal));
                    }
                });
            },
        );
    }
}

/// Benchmark: Network loading time
///
/// Target: <100ms (preferred: <50ms)
///
/// Loading time affects simulation startup and scenario switching.
fn benchmark_network_loading(c: &mut Criterion) {
    c.bench_function("network_load_medium", |b| {
        b.iter(|| {
            black_box(RoadNetwork::load_from_file(
                "test_data/medium_city_graph.json",
            ))
        });
    });

    c.bench_function("network_load_small", |b| {
        b.iter(|| {
            black_box(RoadNetwork::load_from_file(
                "test_data/small_city_graph.json",
            ))
        });
    });
}

/// Benchmark: Velocity calculation overhead
///
/// Target: <1µs (preferred: <100ns)
///
/// Velocity calculations happen on every simulation step for moving agents.
fn benchmark_velocity_calculation(c: &mut Criterion) {
    let velocity_model = TimeOfDayVelocity::default();
    let road_type = RoadType::Residential;
    let time = Utc::now();

    c.bench_function("velocity_calculation", |b| {
        b.iter(|| {
            black_box(velocity_model.calculate_velocity(black_box(&road_type), black_box(time)))
        });
    });
}

/// Benchmark: Point-to-node mapping (nearest node finding)
///
/// Target: <1ms (preferred: <100µs)
///
/// This operation is needed when agents start journeys from arbitrary locations.
fn benchmark_point_to_node_mapping(c: &mut Criterion) {
    let network = load_network();
    let test_point = Point::new(-122.4194, 37.7749);

    c.bench_function("find_nearest_node", |b| {
        b.iter(|| network.find_nearest_node(black_box(test_point)));
    });
}

/// Benchmark: routing graph preprocessing
///
/// This is a one-time cost when loading a network, but important to measure
/// for understanding total initialization time.
fn benchmark_fast_graph_preprocessing(c: &mut Criterion) {
    let network = load_network();

    c.bench_function("build_fast_graph", |b| {
        b.iter(|| black_box(network.build_fast_graph()));
    });
}

/// Benchmark: Route calculation with graph reuse
///
/// This simulates the recommended usage pattern where the routing graph is
/// built once and reused for multiple queries.
fn benchmark_routing_with_graph_reuse(c: &mut Criterion) {
    let network = load_network();
    let routing_graph = network.build_fast_graph();
    let mut path_calculator = routing_graph.create_calculator();

    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let start = node_ids[0];
    let goal = node_ids[node_ids.len() / 2];

    c.bench_function("route_with_graph_reuse", |b| {
        b.iter(|| {
            black_box(network.find_shortest_path_with_cache(
                &mut path_calculator,
                &routing_graph,
                black_box(start),
                black_box(goal),
            ))
        });
    });
}

/// Benchmark: Batch routing with optimal graph reuse
///
/// This demonstrates best-case performance when the routing graph is properly cached.
fn benchmark_batch_routing_optimized(c: &mut Criterion) {
    let network = load_network();
    let routing_graph = network.build_fast_graph();
    let node_ids: Vec<_> = network.nodes.keys().copied().collect();
    let node_count = node_ids.len();

    for batch_size in [10, 100, 1000] {
        c.bench_with_input(
            BenchmarkId::new("route_batch_optimized", batch_size),
            &batch_size,
            |b, &size| {
                let mut path_calculator = routing_graph.create_calculator();
                b.iter(|| {
                    for i in 0..size {
                        let start = node_ids[i % node_count];
                        let goal = node_ids[(i + node_count / 2) % node_count];
                        black_box(network.find_shortest_path_with_cache(
                            &mut path_calculator,
                            &routing_graph,
                            start,
                            goal,
                        ));
                    }
                });
            },
        );
    }
}

criterion_group!(
    benches,
    benchmark_routing_single_query,
    benchmark_routing_batch_queries,
    benchmark_network_loading,
    benchmark_velocity_calculation,
    benchmark_point_to_node_mapping,
    benchmark_fast_graph_preprocessing,
    benchmark_routing_with_graph_reuse,
    benchmark_batch_routing_optimized,
);
criterion_main!(benches);
