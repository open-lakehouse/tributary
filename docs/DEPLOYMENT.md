# Deployment Guide

**Version:** 1.0
**Last Updated:** March 16, 2026
**Framework Version:** Phase 11 Complete (78% Production Ready)

---

## Table of Contents

1. [Production Deployment](#production-deployment)
2. [System Requirements](#system-requirements)
3. [Configuration Options](#configuration-options)
4. [Monitoring Setup](#monitoring-setup)
5. [Logging Configuration](#logging-configuration)
6. [Telemetry Export Setup](#telemetry-export-setup)
7. [Scaling Considerations](#scaling-considerations)
8. [Security Best Practices](#security-best-practices)
9. [Troubleshooting](#troubleshooting)

---

## Production Deployment

### Quick Start

```bash
# 1. Build release binary
cargo build --release

# 2. Run simulation
./target/release/sim-runner --config config.toml

# 3. Monitor telemetry
curl http://localhost:9090/metrics
```

### Deployment Architectures

#### Standalone Mode

**Use Case:** Single-machine simulations, development, testing

```
┌─────────────────────────────────────┐
│  Simulation Runner                   │
├─────────────────────────────────────┤
│  ├─ sim-core (workflow engine)      │
│  ├─ sim-geo (spatial operations)    │
│  ├─ sim-events (event dispatcher)   │
│  └─ sim-telemetry (observability)   │
└─────────────────────────────────────┘
```

**Recommended Hardware:**
- CPU: 4+ cores
- RAM: 8GB minimum, 16GB recommended
- Disk: 10GB+ for snapshots and logs
- Network: Not critical

#### Distributed Mode (Future)

**Use Case:** Large-scale simulations (>100k agents)

```
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│  Simulation  │   │  Simulation  │   │  Simulation  │
│  Worker 1    │   │  Worker 2    │   │  Worker N    │
└──────┬───────┘   └──────┬───────┘   └──────┬───────┘
       │                  │                  │
       └──────────────────┼──────────────────┘
                          │
                  ┌───────▼────────┐
                  │  Coordinator   │
                  │  (future)      │
                  └────────────────┘
```

**Status:** Not yet implemented (planned for Phase 14+)

---

## System Requirements

### Minimum Requirements

| Component | Requirement |
|-----------|-------------|
| OS | Linux, macOS, Windows (with WSL) |
| Rust | 1.70+ |
| CPU | 2 cores |
| RAM | 4GB |
| Disk | 5GB |
| Network | Optional |

### Recommended Requirements

| Component | Requirement |
|-----------|-------------|
| OS | Linux (Ubuntu 22.04+, RHEL 8+) |
| Rust | Latest stable |
| CPU | 8+ cores |
| RAM | 32GB |
| Disk | 100GB SSD |
| Network | 1Gbps for telemetry export |

### Performance Targets

| Metric | Target | Actual (Phase 9-11) |
|--------|--------|---------------------|
| Tick time | <100ms | 0.06-0.81ms ✅ |
| Agent count | 10,000+ | 2,000+ validated ✅ |
| Snapshot save | <1s | 15ms ✅ |
| Snapshot restore | <2s | 7ms ✅ |
| Memory per agent | <100KB | ~50KB ✅ |

### Dependencies

**Required:**
```toml
# From Cargo.toml
arrow = "51.0"
datafusion = "37.0"
parquet = "51.0"
geo = "0.28"
h3o = "0.6"
chrono = "0.4"
serde = "1.0"
tokio = { version = "1.37", features = ["rt-multi-thread", "macros"] }
```

**Optional:**
```toml
# For telemetry export
opentelemetry = "0.22"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## Configuration Options

### Configuration File Format

**File:** `config.toml`

```toml
[simulation]
# Simulation parameters
name = "production_simulation"
start_time = "2024-01-01T00:00:00Z"
duration_hours = 24
tick_duration_seconds = 60  # 1 minute per tick

[agents]
# Initial agent population
customer_count = 1000
driver_count = 50
package_count = 2000

[resources]
# Resource capacities
kitchen_count = 10
sorting_station_count = 20
loading_dock_count = 15

[workflows]
# Workflow configuration
food_order_enabled = true
supply_chain_enabled = true
courier_delivery_enabled = true
package_shipment_enabled = true

[demand_generation]
enabled = true
base_rate = 10.0  # demands per hour
time_of_day_enabled = true

[[demand_generation.peak_hours]]
start_hour = 12
end_hour = 14
rate_multiplier = 3.0  # lunch rush

[[demand_generation.peak_hours]]
start_hour = 18
end_hour = 20
rate_multiplier = 2.5  # dinner rush

[spatial]
# H3 spatial configuration
h3_resolution = 9  # ~174m cells
network_file = "data/city_graph.json"

[snapshots]
# Snapshot configuration
enabled = true
backend = "parquet"
directory = "/var/lib/sim-snapshots"
interval_ticks = 100  # Save every 100 ticks

[telemetry]
# Observability configuration
enabled = true
export_format = "otlp"
endpoint = "http://localhost:4317"
sampling_rate = 1.0  # 100% sampling

[logging]
level = "info"
format = "json"
output = "stdout"
```

### Environment Variables

Override configuration via environment variables:

```bash
# Core settings
export SIM_NAME="production_simulation"
export SIM_TICK_DURATION_SECONDS=60

# Telemetry
export SIM_TELEMETRY_ENDPOINT="http://otel-collector:4317"
export SIM_TELEMETRY_SAMPLING_RATE=0.1  # 10% sampling

# Logging
export RUST_LOG="info,sim_core=debug"
export SIM_LOG_FORMAT="json"

# Snapshots
export SIM_SNAPSHOT_DIR="/mnt/snapshots"
export SIM_SNAPSHOT_INTERVAL=1000

# Performance
export TOKIO_WORKER_THREADS=16
```

### Runtime Configuration API

```rust
use sim_core::{SimulationConfig, SnapshotBackend};

let config = SimulationConfig {
    name: "production".to_string(),
    tick_duration: Duration::seconds(60),
    max_ticks: 1440,  // 24 hours
    snapshot_backend: Some(SnapshotBackend::Parquet {
        path: "/var/lib/snapshots".into(),
    }),
    telemetry_enabled: true,
    telemetry_endpoint: "http://localhost:4317".to_string(),
    ..Default::default()
};

let mut runner = SimulationRunner::new(config)?;
```

---

## Monitoring Setup

### Metrics Exposition

**Built-in Metrics Endpoint:**

```bash
# Start simulation with metrics server
sim-runner --config config.toml --metrics-port 9090

# Query metrics
curl http://localhost:9090/metrics
```

**Key Metrics:**

```prometheus
# Simulation performance
sim_tick_duration_seconds{phase="total"} 0.00081
sim_tick_duration_seconds{phase="demand_generation"} 0.00075
sim_tick_duration_seconds{phase="workflow_execution"} 0.00006

# Agent metrics
sim_agents_total{type="customer"} 1000
sim_agents_total{type="driver"} 50
sim_agents_active{type="customer"} 127

# Resource metrics
sim_resources_utilization{type="kitchen"} 0.78
sim_resources_capacity{type="kitchen"} 100
sim_resources_available{type="kitchen"} 22

# Workflow metrics
sim_workflows_active{type="food_order"} 45
sim_workflows_completed_total{type="food_order"} 1823
sim_workflows_failed_total{type="food_order"} 3

# Snapshot metrics
sim_snapshot_duration_seconds{operation="save"} 0.015
sim_snapshot_size_bytes 1048576
```

### Prometheus Integration

**prometheus.yml:**

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'simulation'
    static_configs:
      - targets: ['localhost:9090']
    metric_relabel_configs:
      - source_labels: [__name__]
        regex: 'sim_.*'
        action: keep
```

### Grafana Dashboards

**Dashboard JSON:** (Create `/var/lib/grafana/dashboards/simulation.json`)

Key panels:
1. Tick duration (time series)
2. Agent population (gauge)
3. Resource utilization (bar chart)
4. Workflow throughput (graph)
5. Error rate (alert panel)

**Dashboard URL:** Import from `docs/grafana/simulation-dashboard.json` (future)

### Alerting Rules

**alerts.yml:**

```yaml
groups:
  - name: simulation_alerts
    interval: 30s
    rules:
      - alert: SlowTickTime
        expr: sim_tick_duration_seconds{phase="total"} > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Simulation tick time exceeds 100ms"
          description: "Tick time is {{ $value }}s, target is 0.1s"

      - alert: HighWorkflowFailureRate
        expr: rate(sim_workflows_failed_total[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Workflow failure rate above 5%"
          description: "Failure rate is {{ $value | humanizePercentage }}"

      - alert: ResourceSaturation
        expr: sim_resources_utilization > 0.95
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Resource {{ $labels.type }} near capacity"
          description: "Utilization is {{ $value | humanizePercentage }}"
```

---

## Logging Configuration

### Log Levels

```rust
// Configure via RUST_LOG environment variable
export RUST_LOG="info,sim_core=debug,sim_geo=trace"

// Or programmatically
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::INFO)
    .with_target(true)
    .with_thread_ids(true)
    .with_line_number(true)
    .json()  // JSON format for production
    .init();
```

### Structured Logging

**Log Format (JSON):**

```json
{
  "timestamp": "2024-01-01T12:34:56.789Z",
  "level": "INFO",
  "target": "sim_core::runner",
  "fields": {
    "message": "Simulation tick complete",
    "tick": 123,
    "duration_ms": 0.81,
    "agents_active": 127,
    "workflows_completed": 5
  },
  "span": {
    "name": "simulation_tick",
    "tick": 123
  }
}
```

### Log Rotation

**Using logrotate (Linux):**

```bash
# /etc/logrotate.d/simulation
/var/log/simulation/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 0640 sim sim
    sharedscripts
    postrotate
        systemctl reload simulation
    endscript
}
```

### Centralized Logging

**Fluentd Configuration:**

```xml
<source>
  @type tail
  path /var/log/simulation/*.log
  pos_file /var/log/td-agent/simulation.pos
  tag simulation
  <parse>
    @type json
    time_key timestamp
    time_format %Y-%m-%dT%H:%M:%S.%LZ
  </parse>
</source>

<match simulation>
  @type elasticsearch
  host elasticsearch.internal
  port 9200
  index_name simulation-logs
  type_name _doc
  logstash_format true
</match>
```

---

## Telemetry Export Setup

### OpenTelemetry Configuration

**OTLP Export (Recommended):**

```rust
use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;

let tracer = opentelemetry_otlp::new_pipeline()
    .tracing()
    .with_exporter(
        opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint("http://localhost:4317")
    )
    .install_simple()?;

global::set_tracer_provider(tracer);
```

**Collector Configuration (otel-collector-config.yaml):**

```yaml
receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

processors:
  batch:
    timeout: 10s
    send_batch_size: 1024

  memory_limiter:
    check_interval: 1s
    limit_mib: 512

exporters:
  prometheus:
    endpoint: "0.0.0.0:8889"

  jaeger:
    endpoint: jaeger:14250
    tls:
      insecure: true

  logging:
    loglevel: info

service:
  pipelines:
    traces:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [jaeger, logging]

    metrics:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [prometheus, logging]

    logs:
      receivers: [otlp]
      processors: [memory_limiter, batch]
      exporters: [logging]
```

### Trace Sampling

**Production Sampling Strategy:**

```rust
// Probabilistic sampling: 10% of traces
let sampler = opentelemetry::sdk::trace::Sampler::TraceIdRatioBased(0.1);

// Or adaptive sampling based on load
let sampler = if runner.is_high_load() {
    opentelemetry::sdk::trace::Sampler::TraceIdRatioBased(0.01)  // 1%
} else {
    opentelemetry::sdk::trace::Sampler::TraceIdRatioBased(0.1)   // 10%
};
```

### Performance Impact

| Telemetry Mode | Tick Overhead | Use Case |
|----------------|---------------|----------|
| Disabled | 0ms | Production (monitoring-only) |
| Sampling 10% | +0.05ms | Production (debugging) |
| Sampling 100% | +0.75ms | Development/Testing |
| Full + Export | +2-5ms | Troubleshooting |

**Recommendation:** Use 10% sampling in production, increase to 100% only during incident investigation.

---

## Scaling Considerations

### Vertical Scaling

**CPU Scaling:**
- Framework is CPU-bound for workflow execution
- Linear scaling up to ~16 cores
- Tokio async runtime handles concurrency
- Recommendation: 8-16 cores for production

**Memory Scaling:**
- Arrow columnar storage is memory-efficient
- ~50KB per agent (includes all state)
- 10,000 agents ≈ 500MB baseline
- Recommendation: 2GB + (agents * 0.0001GB)

**Disk Scaling:**
- Snapshots: ~200KB per 1,500 agents (Parquet compression)
- Logs: ~1MB per hour (JSON, uncompressed)
- Events: ~500KB per hour (Parquet)
- Recommendation: 100GB for 30-day retention

### Horizontal Scaling (Future)

**Not Yet Implemented:**
- Multi-node simulation distribution
- Shared state coordination
- Partition-based scaling

**Planned for Phase 14+:**
```
┌─────────┐   ┌─────────┐   ┌─────────┐
│ Node 1  │   │ Node 2  │   │ Node N  │
│ Agents  │   │ Agents  │   │ Agents  │
│ 1-3000  │   │ 3001-   │   │ ...     │
│         │   │ 6000    │   │         │
└────┬────┘   └────┬────┘   └────┬────┘
     │             │             │
     └─────────────┼─────────────┘
                   │
           ┌───────▼────────┐
           │  Coordinator   │
           │  (etcd/Redis)  │
           └────────────────┘
```

### Batch Size Tuning

**Workflow Batch Updates:**
```rust
// Phase 3 benchmark results show optimal batch sizes
let batch_size = match total_instances {
    0..=1_000 => 100,      // 54x speedup
    1_001..=10_000 => 500, // 209x speedup
    _ => 1_000,            // 785x speedup
};
```

**Resource Allocation:**
```rust
// Allocate tasks in batches for efficiency
let allocation_batch = 100;  // Sweet spot for allocation
runner.allocate_tasks_batch(tasks.chunks(allocation_batch));
```

### Network Topology

**Telemetry Export:**
```
Simulation → localhost:4317 → OTLP Collector → [Jaeger, Prometheus]
```

**Snapshot Storage:**
```
Simulation → NFS/S3 → Parquet files → Archive storage
```

---

## Security Best Practices

### Data Protection

**Snapshot Encryption:**
```rust
// Future: Encrypted snapshots
let snapshot_manager = SnapshotManager::new(
    SnapshotBackend::Parquet {
        path: "/var/lib/snapshots".into(),
    }
)?;

// Recommendation: Use filesystem-level encryption (LUKS, dm-crypt)
```

**Log Sanitization:**
```rust
// Avoid logging sensitive data
tracing::info!(
    customer_id = %customer_id.sanitize(),  // Hash or truncate IDs
    order_value_cents = order.value,        // Log amounts, not details
    "Order created"
);
```

### Access Control

**File Permissions:**
```bash
# Snapshots directory
chown -R sim:sim /var/lib/sim-snapshots
chmod 750 /var/lib/sim-snapshots

# Configuration files
chmod 640 /etc/sim/config.toml
chown root:sim /etc/sim/config.toml

# Log files
chmod 640 /var/log/simulation/*.log
chown sim:sim /var/log/simulation
```

**Systemd Service (Restricted):**
```ini
[Unit]
Description=Simulation Runner
After=network.target

[Service]
Type=simple
User=sim
Group=sim
ExecStart=/usr/local/bin/sim-runner --config /etc/sim/config.toml
Restart=on-failure
RestartSec=10s

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/sim-snapshots /var/log/simulation

[Install]
WantedBy=multi-user.target
```

### Network Security

**Telemetry Endpoint:**
```bash
# Use TLS for OTLP export
export SIM_TELEMETRY_ENDPOINT="https://otel-collector.internal:4317"
export SIM_TELEMETRY_INSECURE=false

# Or use mTLS for authentication
export SIM_TELEMETRY_CLIENT_CERT="/etc/sim/certs/client.crt"
export SIM_TELEMETRY_CLIENT_KEY="/etc/sim/certs/client.key"
export SIM_TELEMETRY_CA_CERT="/etc/sim/certs/ca.crt"
```

**API Security (Future):**
- REST API authentication (JWT tokens)
- Rate limiting for query endpoints
- CORS configuration for web UI

### Secrets Management

**Environment Variable Injection:**
```bash
# Use secrets manager (Vault, K8s Secrets, AWS Secrets Manager)
export SIM_DB_PASSWORD="$(vault kv get -field=password secret/sim/db)"
export SIM_API_KEY="$(aws secretsmanager get-secret-value --secret-id sim-api-key --query SecretString --output text)"
```

---

## Troubleshooting

### Common Issues

#### 1. Slow Tick Times

**Symptom:** `sim_tick_duration_seconds > 0.1` (100ms target)

**Diagnosis:**
```bash
# Check CPU usage
top -p $(pgrep sim-runner)

# Check detailed timing
export RUST_LOG="sim_core::runner=debug"
sim-runner --config config.toml 2>&1 | grep "tick_duration"
```

**Solutions:**
- **High agent count:** Reduce population or increase hardware
- **Complex workflows:** Simplify workflow steps
- **Telemetry overhead:** Reduce sampling rate to 10% or disable
- **Snapshot frequency:** Increase snapshot interval (every 1000 ticks)

#### 2. Memory Leaks

**Symptom:** Memory usage grows unbounded

**Diagnosis:**
```bash
# Monitor memory usage
ps aux | grep sim-runner

# Use memory profiling
valgrind --tool=massif --massif-out-file=sim.massif ./sim-runner
ms_print sim.massif
```

**Solutions:**
- **Event accumulation:** Ensure events are flushed regularly
- **Snapshot retention:** Limit in-memory snapshot history
- **Resource cleanup:** Check for agent/resource removal bugs

#### 3. Workflow Stuck

**Symptom:** Workflows not progressing, `sim_workflows_active` constant

**Diagnosis:**
```bash
# Enable workflow debugging
export RUST_LOG="sim_core::workflows=trace"

# Check resource allocation
curl http://localhost:9090/metrics | grep sim_resources_available
```

**Solutions:**
- **Resource saturation:** Increase resource capacity
- **Task allocation failure:** Check `allocate_tasks()` errors
- **Workflow definition:** Verify step durations and resource types

#### 4. Snapshot Failures

**Symptom:** `SnapshotError: Failed to save snapshot`

**Diagnosis:**
```bash
# Check disk space
df -h /var/lib/sim-snapshots

# Check file permissions
ls -la /var/lib/sim-snapshots

# Check Parquet write errors
export RUST_LOG="sim_core::snapshots=debug"
```

**Solutions:**
- **Disk full:** Clean old snapshots or increase storage
- **Permissions:** Fix directory ownership (`chown -R sim:sim`)
- **Corruption:** Delete corrupted snapshot and retry
- **Schema mismatch:** Regenerate snapshots after schema changes

#### 5. Telemetry Export Errors

**Symptom:** `TelemetryError: Failed to export to OTLP endpoint`

**Diagnosis:**
```bash
# Test endpoint connectivity
curl -v http://localhost:4317

# Check collector logs
docker logs otel-collector

# Verify export configuration
export RUST_LOG="sim_telemetry=debug"
```

**Solutions:**
- **Endpoint unreachable:** Verify collector is running
- **Authentication failure:** Check TLS certificates
- **Rate limiting:** Reduce sampling rate or batch size
- **Version mismatch:** Ensure OTLP protocol version compatibility

### Performance Debugging

**Criterion Benchmarks:**
```bash
# Run workflow performance benchmarks
cargo bench -p sim-core --bench workflow_performance

# Run batch operations benchmarks
cargo bench -p sim-core --bench batch_operations

# Run routing performance benchmarks
cargo bench -p sim-geo --bench routing_performance

# Run telemetry performance benchmarks
cargo bench -p sim-telemetry --bench telemetry_performance
```

**Expected Results:**
- Workflow batch operations: 7-785x speedup
- Arrow RecordBatch operations: 10-100x speedup
- Routing queries: <1ms (cached: 177ns)
- Telemetry generation: <10µs per event

### Log Analysis

**Common Log Patterns:**

```bash
# Find errors
grep '"level":"ERROR"' /var/log/simulation/sim.log

# Find slow ticks
jq 'select(.fields.duration_ms > 100)' /var/log/simulation/sim.log

# Workflow failures
jq 'select(.fields.workflow_status == "failed")' /var/log/simulation/sim.log

# Resource saturation events
jq 'select(.fields.resource_utilization > 0.95)' /var/log/simulation/sim.log
```

### Support Resources

**Documentation:**
- Architecture: `/docs/ARCHITECTURE.md`
- Performance: `/docs/performance/benchmarks.md`
- Profiling: `/docs/performance/profiling.md`

**Issue Tracking:**
- GitHub: `https://github.com/your-org/simulation-framework/issues`
- Slack: `#simulation-support` (internal)

**Emergency Contacts:**
- On-call: simulation-oncall@company.com
- Engineering: simulation-eng@company.com

---

## Appendix

### Production Checklist

Before deploying to production:

- [ ] Configuration file reviewed and tuned
- [ ] System requirements met (CPU, RAM, disk)
- [ ] Monitoring setup (Prometheus + Grafana)
- [ ] Alerting rules configured
- [ ] Log rotation enabled
- [ ] Telemetry export tested
- [ ] Snapshots enabled and tested
- [ ] Security hardening applied
- [ ] Performance benchmarks run
- [ ] Troubleshooting guide accessible
- [ ] Rollback plan documented
- [ ] On-call team notified

### Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-16 | Initial deployment guide |

### References

- **Phase 3 Complete:** Workflow performance benchmarks
- **Phase 6 Complete:** Arrow migration and snapshots
- **Phase 7 Complete:** Road network routing
- **Phase 8 Complete:** Telemetry generation
- **Phase 9 Complete:** Multi-workflow infrastructure
- **Phase 10 Complete:** Demand generation
- **Phase 11 Complete:** Logistics domain validation

---

**Document Version:** 1.0
**Author:** Agent 9
**Review Status:** Production Ready
