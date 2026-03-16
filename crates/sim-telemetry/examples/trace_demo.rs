use sim_telemetry::core::{TelemetryGenerator, TelemetryRecord};
use sim_telemetry::generators::EventToTraceGenerator;
use serde_json::json;

fn main() {
    let mut generator = EventToTraceGenerator::new();
    
    println!("=== Distributed Tracing Demo ===\n");
    
    // Scenario 1: Agent lifecycle
    println!("Scenario 1: Agent Lifecycle (Single Trace Tree)");
    println!("------------------------------------------------");
    
    let agent_events = vec![
        json!({
            "id": "evt-001",
            "type": "com.simulation.agent_created",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:00Z",
            "data": {"agent_id": "agent-001", "type": "worker"}
        }),
        json!({
            "id": "evt-002",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:01Z",
            "data": {"agent_id": "agent-001", "tick": 1, "action": "move"}
        }),
        json!({
            "id": "evt-003",
            "type": "com.simulation.agent_ticked",
            "source": "urn:sim:agent:agent-001",
            "time": "2026-03-16T10:00:02Z",
            "data": {"agent_id": "agent-001", "tick": 2, "action": "harvest"}
        }),
    ];
    
    let mut agent_spans = Vec::new();
    for event in &agent_events {
        let records = generator.generate(event).unwrap();
        agent_spans.push(records[0].clone());
    }
    
    // Print agent trace tree
    print_trace_tree(&agent_spans, "Agent-001 Lifecycle");
    
    // Scenario 2: Multiple independent agents
    println!("\nScenario 2: Multiple Independent Agents");
    println!("----------------------------------------");
    
    let event_a1 = json!({
        "id": "evt-101",
        "type": "com.simulation.agent_created",
        "source": "urn:sim:agent:agent-A",
        "time": "2026-03-16T10:00:00Z",
        "data": {"agent_id": "agent-A"}
    });
    
    let event_b1 = json!({
        "id": "evt-201",
        "type": "com.simulation.agent_created",
        "source": "urn:sim:agent:agent-B",
        "time": "2026-03-16T10:00:00Z",
        "data": {"agent_id": "agent-B"}
    });
    
    let records_a = generator.generate(&event_a1).unwrap();
    let records_b = generator.generate(&event_b1).unwrap();
    
    let trace_a = extract_trace_id(&records_a[0]);
    let trace_b = extract_trace_id(&records_b[0]);
    
    println!("Agent A Trace ID: {}", &trace_a[..8]);
    println!("Agent B Trace ID: {}", &trace_b[..8]);
    println!("Independent traces: {}", trace_a != trace_b);
    
    // Scenario 3: Workflow with steps
    println!("\nScenario 3: Workflow Execution Chain");
    println!("-------------------------------------");
    
    let workflow_events = vec![
        json!({
            "id": "evt-301",
            "type": "com.simulation.workflow_started",
            "source": "urn:sim:workflow:harvest-wf",
            "time": "2026-03-16T10:00:00Z",
            "data": {"workflow_id": "harvest-wf", "initiator": "agent-001"}
        }),
        json!({
            "id": "evt-302",
            "type": "com.simulation.workflow_step_executed",
            "source": "urn:sim:workflow:harvest-wf",
            "time": "2026-03-16T10:00:01Z",
            "data": {"workflow_id": "harvest-wf", "step": "locate_resource"}
        }),
        json!({
            "id": "evt-303",
            "type": "com.simulation.workflow_step_executed",
            "source": "urn:sim:workflow:harvest-wf",
            "time": "2026-03-16T10:00:02Z",
            "data": {"workflow_id": "harvest-wf", "step": "allocate_capacity"}
        }),
        json!({
            "id": "evt-304",
            "type": "com.simulation.workflow_completed",
            "source": "urn:sim:workflow:harvest-wf",
            "time": "2026-03-16T10:00:03Z",
            "data": {"workflow_id": "harvest-wf", "status": "success"}
        }),
    ];
    
    let mut workflow_spans = Vec::new();
    for event in &workflow_events {
        let records = generator.generate(event).unwrap();
        workflow_spans.push(records[0].clone());
    }
    
    print_trace_tree(&workflow_spans, "Harvest Workflow");
    
    println!("\n=== Correlation Summary ===");
    println!("- Events from same source share trace_id");
    println!("- Sequential events form parent-child chains");
    println!("- Different sources get independent traces");
    println!("- Workflow steps automatically linked");
}

fn extract_trace_id(record: &TelemetryRecord) -> String {
    match record {
        TelemetryRecord::Trace { trace_id, .. } => trace_id.clone(),
        _ => panic!("Expected Trace record"),
    }
}

fn print_trace_tree(spans: &[TelemetryRecord], title: &str) {
    println!("\n{}", title);
    
    // Extract span info
    let mut span_info = Vec::new();
    for span in spans {
        match span {
            TelemetryRecord::Trace {
                trace_id,
                span_id,
                parent_span_id,
                name,
                ..
            } => {
                span_info.push((
                    trace_id.clone(),
                    span_id.clone(),
                    parent_span_id.clone(),
                    name.clone(),
                ));
            }
            _ => panic!("Expected Trace record"),
        }
    }
    
    if span_info.is_empty() {
        return;
    }
    
    let trace_id = &span_info[0].0;
    println!("Trace: {}", &trace_id[..16]);
    println!();
    
    for (i, (_, span_id, parent_id, name)) in span_info.iter().enumerate() {
        let indent = "  ".repeat(i);
        let short_span = &span_id[..8];
        
        if let Some(parent) = parent_id {
            let short_parent = &parent[..8];
            println!("{}├─ {} [{}] (parent: {})", indent, name, short_span, short_parent);
        } else {
            println!("{}└─ {} [{}] (root)", indent, name, short_span);
        }
    }
}
