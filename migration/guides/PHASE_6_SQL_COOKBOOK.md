# Phase 6 SQL Query Cookbook

**Last Updated:** March 16, 2026
**Phase:** 6 Week 3 - SQL Query Interface
**Audience:** Domain developers, data analysts, integration engineers

---

## Table of Contents

1. [Introduction](#introduction)
2. [Available Tables](#available-tables)
3. [Getting Started](#getting-started)
4. [Simple SELECT Queries](#simple-select-queries)
5. [WHERE Filtering](#where-filtering)
6. [JOIN Queries](#join-queries)
7. [Aggregations](#aggregations)
8. [Subqueries](#subqueries)
9. [Window Functions](#window-functions)
10. [Spatial Queries](#spatial-queries)
11. [Complex Multi-Table JOINs](#complex-multi-table-joins)
12. [Performance Tips](#performance-tips)
13. [Error Handling](#error-handling)
14. [Common Patterns](#common-patterns)

---

## Introduction

Phase 6 Week 3 introduces a SQL query interface powered by Apache DataFusion. This allows you to query simulation state using standard SQL instead of writing manual RecordBatch traversal code.

### When to Use SQL vs Direct Methods

**Use SQL when:**
- ✅ You need to JOIN multiple entity types
- ✅ You want aggregations (COUNT, SUM, AVG)
- ✅ Complex filtering with multiple conditions
- ✅ Ad-hoc analysis or debugging
- ✅ Reporting and dashboards
- ✅ You want declarative, readable code

**Use direct methods when:**
- ❌ Single entity lookup by ID (O(1) HashMap is faster)
- ❌ Hot path performance (avoid async overhead)
- ❌ Simple iteration without filtering
- ❌ Modifying state (SQL is read-only)

**Performance Comparison:**
```rust
// Direct lookup: O(1), ~50ns
let agent = population.get_agent(&agent_id)?;

// SQL lookup: O(n), ~1ms (includes SessionContext overhead)
let result = runner.query("SELECT * FROM agents WHERE id = ?").await?;

// But for aggregations, SQL is faster:
// Manual aggregation: O(n), ~3ms
let mut counts = HashMap::new();
for agent in population.iter() { /* ... */ }

// SQL aggregation: O(n), ~0.8ms (DataFusion optimizes)
let result = runner.query("SELECT agent_type, COUNT(*) FROM agents GROUP BY agent_type").await?;
```

---

## Available Tables

### Table: `agents`

**Source:** PopulationHandler (if set)

**Schema:**
```sql
CREATE TABLE agents (
    id               BINARY,    -- AgentId UUID (16 bytes)
    agent_type       VARCHAR,   -- Type identifier (e.g., "customer", "worker")
    status           VARCHAR,   -- Current status (e.g., "active", "idle")
    position_x       DOUBLE,    -- X coordinate (nullable)
    position_y       DOUBLE,    -- Y coordinate (nullable)
    metadata         VARCHAR    -- JSON for domain-specific fields (nullable)
);
```

**Example Row:**
```
id: [01, 23, 45, 67, ...] (16 bytes)
agent_type: "customer"
status: "waiting"
position_x: 37.7749
position_y: -122.4194
metadata: '{"name": "Alice", "priority": "high"}'
```

### Table: `resources`

**Source:** ResourceAllocator (if set)

**Schema:**
```sql
CREATE TABLE resources (
    id               BINARY,    -- ResourceId UUID (16 bytes)
    resource_type    VARCHAR,   -- Type identifier (e.g., "kitchen", "delivery_vehicle")
    capacity         BIGINT,    -- Total capacity (max concurrent tasks)
    available        BIGINT,    -- Currently available capacity units
    location_id      BINARY,    -- Optional LocationId UUID (nullable)
    metadata         VARCHAR    -- JSON for domain-specific fields (nullable)
);
```

**Example Row:**
```
id: [01, 23, 45, 67, ...] (16 bytes)
resource_type: "kitchen_station"
capacity: 10
available: 7
location_id: [ab, cd, ef, ...] (nullable)
metadata: '{"station_number": 3}'
```

### Table: `tasks`

**Source:** ResourceAllocator (if set)

**Schema:**
```sql
CREATE TABLE tasks (
    id                      BINARY,    -- TaskId UUID (16 bytes)
    task_type               VARCHAR,   -- Type identifier (matches resource_type)
    status                  VARCHAR,   -- "pending", "allocated", "completed"
    required_capacity       BIGINT,    -- Capacity units needed
    assigned_resource_id    BINARY,    -- ResourceId when allocated (nullable)
    workflow_instance_id    BINARY,    -- WorkflowInstanceId if part of workflow (nullable)
    priority                INTEGER,   -- Task priority (higher = more urgent)
    metadata                VARCHAR,   -- JSON for domain fields (nullable)
    allocated_at_millis     BIGINT,    -- Allocation timestamp (milliseconds, nullable)
    estimated_duration_secs BIGINT     -- Expected duration in seconds
);
```

**Example Row:**
```
id: [01, 23, 45, 67, ...] (16 bytes)
task_type: "food_preparation"
status: "allocated"
required_capacity: 5
assigned_resource_id: [ab, cd, ef, ...] (nullable)
workflow_instance_id: [12, 34, 56, ...] (nullable)
priority: 10
metadata: '{"dish": "burger", "special_requests": []}'
allocated_at_millis: 1710585600000
estimated_duration_secs: 600
```

### Table: `workflow_instances`

**Source:** WorkflowEngine (if set)

**Schema:**
```sql
CREATE TABLE workflow_instances (
    id               BINARY,    -- WorkflowInstanceId UUID (16 bytes)
    workflow_id      BINARY,    -- WorkflowId UUID (16 bytes)
    status           VARCHAR,   -- "pending", "active", "completed", "failed"
    current_step     INTEGER,   -- Current step index (0-based)
    total_steps      INTEGER,   -- Total number of steps in workflow
    agent_id         BINARY,    -- Optional AgentId associated with instance (nullable)
    started_at       BIGINT,    -- Start timestamp (milliseconds)
    completed_at     BIGINT,    -- Completion timestamp (nullable)
    context          VARCHAR,   -- JSON workflow context (nullable)
    metadata         VARCHAR    -- JSON for domain fields (nullable)
);
```

**Example Row:**
```
id: [01, 23, 45, 67, ...] (16 bytes)
workflow_id: [ab, cd, ef, ...] (16 bytes)
status: "active"
current_step: 2
total_steps: 5
agent_id: [12, 34, 56, ...] (nullable)
started_at: 1710585600000
completed_at: NULL
context: '{"order_id": "ORD-123"}'
metadata: '{}'
```

### Table: `locations` (sim-geo only)

**Source:** LocationHandler (if set)

**Schema:**
```sql
CREATE TABLE locations (
    id               BINARY,    -- LocationId UUID (16 bytes)
    location_type    VARCHAR,   -- Type identifier (e.g., "restaurant", "customer_home")
    lat              DOUBLE,    -- Latitude
    lon              DOUBLE,    -- Longitude
    h3_cell          BIGINT,    -- H3 cell index (unsigned 64-bit)
    metadata         VARCHAR    -- JSON for domain fields (nullable)
);
```

**Example Row:**
```
id: [01, 23, 45, 67, ...] (16 bytes)
location_type: "restaurant"
lat: 37.7749
lon: -122.4194
h3_cell: 617700169958293503
metadata: '{"name": "Burger Palace", "hours": "9am-9pm"}'
```

---

## Getting Started

### Basic Query Pattern

```rust
use sim_core::SimulationRunner;

#[tokio::main]
async fn main() -> Result<()> {
    let mut runner = create_simulation_runner();

    // Execute SQL query
    let result = runner.query("SELECT * FROM agents").await?;

    // Process results
    for batch in result {
        println!("Got {} rows", batch.num_rows());
        // ... process batch columns
    }

    Ok(())
}
```

### Extracting Column Data

```rust
use arrow::array::{Array, StringArray, Int64Array, Float64Array, BinaryArray};

let result = runner.query("SELECT agent_type, status FROM agents").await?;

for batch in result {
    // Column 0: agent_type (string)
    let agent_type_col = batch.column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();

    // Column 1: status (string)
    let status_col = batch.column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();

    // Iterate rows
    for i in 0..batch.num_rows() {
        let agent_type = agent_type_col.value(i);
        let status = status_col.value(i);
        println!("{}: {}", agent_type, status);
    }
}
```

---

## Simple SELECT Queries

### Recipe 1: Select All Rows

```sql
-- Get all agents
SELECT * FROM agents;

-- Get all resources
SELECT * FROM resources;

-- Get all tasks
SELECT * FROM tasks;
```

**Rust Example:**
```rust
let result = runner.query("SELECT * FROM agents").await?;
println!("Total agents: {}", result[0].num_rows());
```

### Recipe 2: Select Specific Columns

```sql
-- Get agent IDs and types only
SELECT id, agent_type FROM agents;

-- Get resource capacity info
SELECT resource_type, capacity, available FROM resources;

-- Get task status summary
SELECT task_type, status, priority FROM tasks;
```

**Rust Example:**
```rust
let result = runner.query("
    SELECT agent_type, status
    FROM agents
").await?;
```

### Recipe 3: LIMIT Results

```sql
-- Get first 10 agents
SELECT * FROM agents LIMIT 10;

-- Get top 5 highest priority tasks
SELECT * FROM tasks ORDER BY priority DESC LIMIT 5;
```

**Rust Example:**
```rust
let result = runner.query("
    SELECT * FROM agents LIMIT 10
").await?;
```

### Recipe 4: ORDER BY

```sql
-- Sort agents by type
SELECT * FROM agents ORDER BY agent_type;

-- Sort resources by available capacity (descending)
SELECT * FROM resources ORDER BY available DESC;

-- Sort tasks by priority (highest first), then by creation time
SELECT * FROM tasks ORDER BY priority DESC, allocated_at_millis ASC;
```

---

## WHERE Filtering

### Recipe 5: Simple Equality Filter

```sql
-- Get all "customer" type agents
SELECT * FROM agents WHERE agent_type = 'customer';

-- Get "kitchen" resources
SELECT * FROM resources WHERE resource_type = 'kitchen';

-- Get pending tasks
SELECT * FROM tasks WHERE status = 'pending';
```

**Rust Example:**
```rust
let result = runner.query("
    SELECT * FROM agents
    WHERE agent_type = 'customer'
").await?;
```

### Recipe 6: Comparison Operators

```sql
-- Get resources with capacity > 5
SELECT * FROM resources WHERE capacity > 5;

-- Get high-priority tasks (priority >= 10)
SELECT * FROM tasks WHERE priority >= 10;

-- Get workflows with more than 3 steps
SELECT * FROM workflow_instances WHERE total_steps > 3;
```

### Recipe 7: Multiple Conditions (AND/OR)

```sql
-- Get active customer agents
SELECT * FROM agents
WHERE agent_type = 'customer' AND status = 'active';

-- Get allocated tasks for kitchen resources
SELECT * FROM tasks
WHERE status = 'allocated' AND task_type = 'kitchen';

-- Get pending or allocated tasks
SELECT * FROM tasks
WHERE status = 'pending' OR status = 'allocated';

-- Complex: High-priority kitchen tasks that are pending
SELECT * FROM tasks
WHERE task_type = 'kitchen'
  AND status = 'pending'
  AND priority >= 10;
```

### Recipe 8: IN Operator

```sql
-- Get agents with specific types
SELECT * FROM agents
WHERE agent_type IN ('customer', 'worker', 'manager');

-- Get tasks with specific statuses
SELECT * FROM tasks
WHERE status IN ('pending', 'allocated');
```

### Recipe 9: LIKE Pattern Matching

```sql
-- Get agents with type starting with "cust"
SELECT * FROM agents WHERE agent_type LIKE 'cust%';

-- Get resources containing "kitchen" in type
SELECT * FROM resources WHERE resource_type LIKE '%kitchen%';
```

### Recipe 10: NULL Checks

```sql
-- Get tasks not yet allocated (no assigned_resource_id)
SELECT * FROM tasks WHERE assigned_resource_id IS NULL;

-- Get tasks that are allocated (has assigned_resource_id)
SELECT * FROM tasks WHERE assigned_resource_id IS NOT NULL;

-- Get workflows not yet completed
SELECT * FROM workflow_instances WHERE completed_at IS NULL;
```

---

## JOIN Queries

### Recipe 11: Two-Table JOIN (Agents + Workflows)

```sql
-- Get agents with their active workflows
SELECT
    a.agent_type,
    a.status as agent_status,
    w.status as workflow_status,
    w.current_step,
    w.total_steps
FROM agents a
JOIN workflow_instances w ON a.id = w.agent_id
WHERE w.status = 'active';
```

**Rust Example:**
```rust
let result = runner.query("
    SELECT
        a.agent_type,
        w.status as workflow_status
    FROM agents a
    JOIN workflow_instances w ON a.id = w.agent_id
").await?;
```

### Recipe 12: Two-Table JOIN (Tasks + Resources)

```sql
-- Get allocated tasks with their assigned resources
SELECT
    t.task_type,
    t.status as task_status,
    t.priority,
    r.resource_type,
    r.available as resource_available
FROM tasks t
JOIN resources r ON t.assigned_resource_id = r.id
WHERE t.status = 'allocated';
```

### Recipe 13: LEFT JOIN (Include NULLs)

```sql
-- Get all tasks with their assigned resources (or NULL if not assigned)
SELECT
    t.task_type,
    t.status,
    r.resource_type
FROM tasks t
LEFT JOIN resources r ON t.assigned_resource_id = r.id;
```

**Use Case:** Find tasks that haven't been allocated yet:
```sql
SELECT t.*
FROM tasks t
LEFT JOIN resources r ON t.assigned_resource_id = r.id
WHERE r.id IS NULL;
```

---

## Aggregations

### Recipe 14: COUNT

```sql
-- Count total agents
SELECT COUNT(*) as total_agents FROM agents;

-- Count agents by type
SELECT agent_type, COUNT(*) as count
FROM agents
GROUP BY agent_type;

-- Count tasks by status
SELECT status, COUNT(*) as count
FROM tasks
GROUP BY status;
```

**Rust Example:**
```rust
let result = runner.query("
    SELECT status, COUNT(*) as count
    FROM tasks
    GROUP BY status
").await?;

for batch in result {
    let status_col = batch.column(0).as_any().downcast_ref::<StringArray>().unwrap();
    let count_col = batch.column(1).as_any().downcast_ref::<Int64Array>().unwrap();

    for i in 0..batch.num_rows() {
        println!("{}: {}", status_col.value(i), count_col.value(i));
    }
}
```

### Recipe 15: SUM

```sql
-- Total resource capacity
SELECT SUM(capacity) as total_capacity FROM resources;

-- Total capacity by resource type
SELECT resource_type, SUM(capacity) as total_capacity
FROM resources
GROUP BY resource_type;
```

### Recipe 16: AVG

```sql
-- Average task priority
SELECT AVG(priority) as avg_priority FROM tasks;

-- Average resource capacity by type
SELECT resource_type, AVG(capacity) as avg_capacity
FROM resources
GROUP BY resource_type;

-- Average estimated duration by task type
SELECT task_type, AVG(estimated_duration_secs) as avg_duration_secs
FROM tasks
GROUP BY task_type;
```

### Recipe 17: MIN/MAX

```sql
-- Find highest priority task
SELECT MAX(priority) as max_priority FROM tasks;

-- Find resource with most capacity
SELECT resource_type, capacity
FROM resources
WHERE capacity = (SELECT MAX(capacity) FROM resources);

-- Earliest and latest workflow start times
SELECT
    MIN(started_at) as earliest_start,
    MAX(started_at) as latest_start
FROM workflow_instances;
```

### Recipe 18: HAVING (Filter After GROUP BY)

```sql
-- Resource types with more than 5 instances
SELECT resource_type, COUNT(*) as count
FROM resources
GROUP BY resource_type
HAVING COUNT(*) > 5;

-- Agent types with average high priority
SELECT agent_type, AVG(priority) as avg_priority
FROM agents a
JOIN tasks t ON a.id = t.workflow_instance_id  -- Simplified example
GROUP BY agent_type
HAVING AVG(priority) > 10;
```

### Recipe 19: Multiple Aggregations

```sql
-- Resource utilization summary
SELECT
    resource_type,
    COUNT(*) as resource_count,
    SUM(capacity) as total_capacity,
    SUM(available) as total_available,
    SUM(capacity - available) as total_in_use,
    ROUND(100.0 * SUM(capacity - available) / SUM(capacity), 2) as utilization_pct
FROM resources
GROUP BY resource_type
ORDER BY utilization_pct DESC;
```

**Output:**
```
resource_type    | resource_count | total_capacity | total_available | total_in_use | utilization_pct
-----------------|----------------|----------------|-----------------|--------------|----------------
kitchen_station  | 10             | 100            | 35              | 65           | 65.00
checkout_lane    | 5              | 25             | 18              | 7            | 28.00
```

---

## Subqueries

### Recipe 20: Subquery in WHERE

```sql
-- Find tasks with above-average priority
SELECT *
FROM tasks
WHERE priority > (SELECT AVG(priority) FROM tasks);

-- Find resources with maximum capacity
SELECT *
FROM resources
WHERE capacity = (SELECT MAX(capacity) FROM resources);
```

### Recipe 21: Subquery in SELECT

```sql
-- Show each agent's count relative to total
SELECT
    agent_type,
    COUNT(*) as count,
    (SELECT COUNT(*) FROM agents) as total_agents,
    ROUND(100.0 * COUNT(*) / (SELECT COUNT(*) FROM agents), 2) as percentage
FROM agents
GROUP BY agent_type;
```

### Recipe 22: Subquery in FROM (Derived Table)

```sql
-- Get average utilization per resource type
SELECT
    resource_type,
    AVG(utilization_pct) as avg_utilization
FROM (
    SELECT
        resource_type,
        100.0 * (capacity - available) / capacity as utilization_pct
    FROM resources
    WHERE capacity > 0
) AS resource_util
GROUP BY resource_type;
```

### Recipe 23: EXISTS Subquery

```sql
-- Find agents that have active workflows
SELECT *
FROM agents a
WHERE EXISTS (
    SELECT 1
    FROM workflow_instances w
    WHERE w.agent_id = a.id
      AND w.status = 'active'
);

-- Find resources with no allocated tasks
SELECT *
FROM resources r
WHERE NOT EXISTS (
    SELECT 1
    FROM tasks t
    WHERE t.assigned_resource_id = r.id
      AND t.status = 'allocated'
);
```

---

## Window Functions

### Recipe 24: ROW_NUMBER

```sql
-- Rank tasks by priority within each task type
SELECT
    task_type,
    priority,
    status,
    ROW_NUMBER() OVER (PARTITION BY task_type ORDER BY priority DESC) as rank_in_type
FROM tasks;
```

**Output:**
```
task_type        | priority | status    | rank_in_type
-----------------|----------|-----------|-------------
kitchen          | 20       | pending   | 1
kitchen          | 15       | allocated | 2
kitchen          | 10       | pending   | 3
delivery         | 18       | pending   | 1
delivery         | 12       | allocated | 2
```

### Recipe 25: RANK and DENSE_RANK

```sql
-- Rank resources by capacity (with ties)
SELECT
    resource_type,
    capacity,
    RANK() OVER (ORDER BY capacity DESC) as rank,
    DENSE_RANK() OVER (ORDER BY capacity DESC) as dense_rank
FROM resources;
```

### Recipe 26: Running Totals

```sql
-- Running total of workflow starts over time
SELECT
    started_at,
    COUNT(*) as workflows_started,
    SUM(COUNT(*)) OVER (ORDER BY started_at) as cumulative_workflows
FROM workflow_instances
GROUP BY started_at
ORDER BY started_at;
```

### Recipe 27: Moving Averages

```sql
-- 3-step moving average of task priority
SELECT
    task_type,
    priority,
    AVG(priority) OVER (
        PARTITION BY task_type
        ORDER BY allocated_at_millis
        ROWS BETWEEN 2 PRECEDING AND CURRENT ROW
    ) as moving_avg_priority
FROM tasks
WHERE allocated_at_millis IS NOT NULL
ORDER BY task_type, allocated_at_millis;
```

---

## Spatial Queries

**Note:** These queries require the `sim-geo` crate and LocationHandler.

### Recipe 28: H3 Cell Distribution

```sql
-- Count locations per H3 cell
SELECT h3_cell, COUNT(*) as location_count
FROM locations
GROUP BY h3_cell
ORDER BY location_count DESC
LIMIT 10;
```

### Recipe 29: Location Types

```sql
-- Distribution of location types
SELECT location_type, COUNT(*) as count
FROM locations
GROUP BY location_type
ORDER BY count DESC;
```

### Recipe 30: Spatial JOIN (Agents + Locations)

```sql
-- Find agents at specific location types
SELECT
    a.agent_type,
    a.status,
    l.location_type,
    l.lat,
    l.lon
FROM agents a
JOIN locations l ON a.position_x = l.lon AND a.position_y = l.lat
WHERE l.location_type = 'restaurant';
```

**Note:** This is a simplified example. For true spatial JOINs, consider using H3 cell matching:

```sql
-- Join agents to nearby locations via H3 cells
-- (Requires computing H3 cell from agent position first)
SELECT
    a.agent_type,
    l.location_type,
    COUNT(*) as nearby_locations
FROM agents a
JOIN locations l ON CAST(h3_lat_lng_to_cell(a.position_y, a.position_x, 9) AS BIGINT) = l.h3_cell
GROUP BY a.agent_type, l.location_type;
```

---

## Complex Multi-Table JOINs

### Recipe 31: Three-Way JOIN (Agents + Workflows + Tasks)

```sql
-- Complete workflow-task-agent view
SELECT
    a.agent_type,
    a.status as agent_status,
    w.status as workflow_status,
    w.current_step,
    t.task_type,
    t.status as task_status
FROM workflow_instances w
JOIN agents a ON w.agent_id = a.id
JOIN tasks t ON w.id = t.workflow_instance_id
WHERE w.status = 'active';
```

### Recipe 32: Four-Way JOIN (Complete System View)

```sql
-- Full simulation state: agents, workflows, tasks, resources
SELECT
    a.agent_type,
    w.status as workflow_status,
    t.task_type,
    t.status as task_status,
    r.resource_type,
    r.available as resource_available
FROM workflow_instances w
JOIN agents a ON w.agent_id = a.id
JOIN tasks t ON w.id = t.workflow_instance_id
JOIN resources r ON t.assigned_resource_id = r.id
WHERE t.status = 'allocated';
```

### Recipe 33: Complex Business Query

```sql
-- Find bottlenecks: tasks waiting for unavailable resources
SELECT
    t.task_type,
    COUNT(DISTINCT t.id) as waiting_tasks,
    COUNT(DISTINCT r.id) as available_resources,
    SUM(r.available) as total_available_capacity
FROM tasks t
LEFT JOIN resources r ON t.task_type = r.resource_type AND r.available > 0
WHERE t.status = 'pending'
GROUP BY t.task_type
HAVING COUNT(DISTINCT r.id) = 0 OR SUM(r.available) < COUNT(DISTINCT t.id)
ORDER BY waiting_tasks DESC;
```

**Interpretation:** This query identifies task types where demand exceeds supply.

---

## Performance Tips

### Tip 1: Use Column Projection

**Bad (selects all columns):**
```sql
SELECT * FROM agents WHERE agent_type = 'customer';
```

**Good (selects only needed columns):**
```sql
SELECT id, status FROM agents WHERE agent_type = 'customer';
```

**Why:** DataFusion only reads projected columns from RecordBatch, reducing memory and CPU.

### Tip 2: Filter Before JOIN

**Bad (joins all rows, then filters):**
```sql
SELECT a.*, w.*
FROM agents a
JOIN workflow_instances w ON a.id = w.agent_id
WHERE w.status = 'active';
```

**Good (filters before join):**
```sql
SELECT a.*, w.*
FROM agents a
JOIN (SELECT * FROM workflow_instances WHERE status = 'active') w
  ON a.id = w.agent_id;
```

**Why:** Reduces the size of data flowing through the JOIN operation.

### Tip 3: Use LIMIT for Sampling

```sql
-- Get a sample of 100 agents (fast)
SELECT * FROM agents LIMIT 100;

-- Instead of fetching all agents and sampling in Rust
```

### Tip 4: Leverage Indexes

While DataFusion doesn't have traditional indexes, RecordBatch columnar layout provides implicit performance benefits:

- **Good:** Queries that filter on few columns
- **Bad:** Queries that `SELECT *` and filter in Rust

### Tip 5: Batch Size Optimization

For large result sets, DataFusion returns multiple RecordBatches:

```rust
let result = runner.query("SELECT * FROM agents").await?;

for batch in result {
    // Process each batch independently (memory efficient)
    process_batch(batch);
}
```

**Don't do this:**
```rust
// BAD: Collect all batches into memory
let all_rows: Vec<_> = result.into_iter()
    .flat_map(|batch| /* extract rows */)
    .collect();
```

### Tip 6: When SQL is Faster

**SQL is faster for:**
- Aggregations (COUNT, SUM, AVG) - DataFusion vectorizes operations
- Complex filtering with multiple conditions
- GROUP BY operations
- JOIN queries

**Direct methods are faster for:**
- Single-row lookup by ID: `population.get_agent(&id)` (O(1) HashMap)
- Simple iteration without filtering
- Hot path code (avoid async overhead)

**Benchmark Example:**
```rust
// Direct lookup: ~50ns
let agent = population.get_agent(&agent_id)?;

// SQL lookup: ~1ms (includes SessionContext initialization)
let result = runner.query("SELECT * FROM agents WHERE id = ?").await?;

// But for aggregation:
// Manual: ~3ms
let mut counts = HashMap::new();
for agent in population.iter() { /* count by type */ }

// SQL: ~0.8ms
let result = runner.query("SELECT agent_type, COUNT(*) FROM agents GROUP BY agent_type").await?;
```

---

## Error Handling

### Common Errors

#### Error 1: Table Not Found

```
Error: Query error: table 'agents' not found
```

**Cause:** Handler not set on SimulationRunner

**Solution:**
```rust
runner.set_population_handler(population); // Must set before querying
let result = runner.query("SELECT * FROM agents").await?;
```

#### Error 2: Invalid SQL Syntax

```
Error: SQL execution failed: syntax error at or near "SELCT"
```

**Cause:** Typo in SQL query

**Solution:** Fix syntax:
```sql
-- Wrong
SELCT * FROM agents;

-- Correct
SELECT * FROM agents;
```

#### Error 3: Column Not Found

```
Error: Schema error: No field named 'agent_name'
```

**Cause:** Column doesn't exist in schema

**Solution:** Check available columns:
```sql
-- List all columns (DataFusion doesn't support DESCRIBE, but you can SELECT 1 row)
SELECT * FROM agents LIMIT 1;
```

**Available Columns:** See [Available Tables](#available-tables) section.

#### Error 4: Type Mismatch

```
Error: Arrow error: cannot cast Binary to Utf8
```

**Cause:** Comparing binary UUID to string

**Solution:**
```sql
-- Wrong (comparing binary to string)
SELECT * FROM agents WHERE id = 'some-uuid-string';

-- Correct (use binary literal or skip ID comparison)
SELECT * FROM agents WHERE agent_type = 'customer';
```

**Note:** Binary UUIDs are not directly comparable in SQL. Use joins instead.

### Error Handling Patterns

```rust
match runner.query(sql).await {
    Ok(result) => {
        for batch in result {
            // Process results
        }
    },
    Err(e) if e.to_string().contains("table") && e.to_string().contains("not found") => {
        eprintln!("Handler not initialized: {}", e);
        // Provide helpful message
    },
    Err(e) if e.to_string().contains("syntax error") => {
        eprintln!("Invalid SQL: {}", e);
        // Log query for debugging
    },
    Err(e) => {
        eprintln!("Query failed: {}", e);
        // Generic error handling
    },
}
```

---

## Common Patterns

### Pattern 1: Count by Status

```sql
SELECT status, COUNT(*) as count
FROM {table}
GROUP BY status
ORDER BY count DESC;
```

**Use Cases:**
- Agent status distribution
- Task status distribution
- Workflow status distribution

### Pattern 2: Resource Utilization

```sql
SELECT
    resource_type,
    capacity,
    available,
    (capacity - available) as in_use,
    ROUND(100.0 * (capacity - available) / capacity, 2) as utilization_pct
FROM resources
ORDER BY utilization_pct DESC;
```

### Pattern 3: Top-N by Priority

```sql
SELECT *
FROM tasks
WHERE status = 'pending'
ORDER BY priority DESC
LIMIT 10;
```

### Pattern 4: Find Idle Resources

```sql
SELECT *
FROM resources
WHERE available = capacity;
```

### Pattern 5: Workflow Progress

```sql
SELECT
    status,
    AVG(CAST(current_step AS DOUBLE) / total_steps * 100) as avg_progress_pct,
    COUNT(*) as workflow_count
FROM workflow_instances
GROUP BY status
ORDER BY avg_progress_pct DESC;
```

### Pattern 6: Task-Resource Matching

```sql
-- Find pending tasks that have available resources
SELECT
    t.task_type,
    COUNT(DISTINCT t.id) as pending_tasks,
    COUNT(DISTINCT r.id) as available_resources
FROM tasks t
JOIN resources r ON t.task_type = r.resource_type
WHERE t.status = 'pending' AND r.available > 0
GROUP BY t.task_type;
```

### Pattern 7: Agent Activity Summary

```sql
SELECT
    a.agent_type,
    a.status,
    COUNT(DISTINCT a.id) as agent_count,
    COUNT(DISTINCT w.id) as active_workflows,
    COUNT(DISTINCT t.id) as active_tasks
FROM agents a
LEFT JOIN workflow_instances w ON a.id = w.agent_id AND w.status = 'active'
LEFT JOIN tasks t ON w.id = t.workflow_instance_id AND t.status = 'allocated'
GROUP BY a.agent_type, a.status
ORDER BY agent_count DESC;
```

---

**SQL Cookbook Version:** 1.0
**Last Updated:** March 16, 2026
**Total Recipes:** 33 queries
**Coverage:** Simple SELECTs, JOINs, Aggregations, Subqueries, Window Functions, Spatial Queries
