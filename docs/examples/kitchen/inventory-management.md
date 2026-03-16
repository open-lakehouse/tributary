# Inventory Management Example

Auto-replenishment system with supply ordering triggers and stock tracking.

## Overview

Demonstrates:
- InventoryManager for stock tracking
- Automatic reorder triggers
- Supply chain workflow integration
- Low-stock monitoring

## Code Location

`/Users/robert.pack/code/management/crates/sim-kitchen/src/resources/inventory.rs`

## InventoryManager API

```rust
pub struct InventoryManager {
    pub fn new() -> Self;
    pub fn check_quantity(&self, item: &str) -> usize;
    pub fn consume(&mut self, item: &str, quantity: usize) -> Result<()>;
    pub fn restock(&mut self, item: &str, quantity: usize);
    pub fn check_low_stock(&self) -> Vec<String>;
    pub fn set_reorder_point(&mut self, item: &str, threshold: usize);
}
```

## Example Usage

```rust
use sim_kitchen::{InventoryManager, SupplyChainWorkflow};

// Create inventory
let mut inventory = InventoryManager::new();

// Set initial stock
inventory.restock("beef".to_string(), 100);
inventory.restock("buns".to_string(), 200);

// Set reorder points
inventory.set_reorder_point("beef", 20);
inventory.set_reorder_point("buns", 50);

// Consume during order
inventory.consume("beef", 1)?;  // Makes 1 burger
inventory.consume("buns", 2)?;  // 2 buns per burger

// Check for low stock
let low_stock = inventory.check_low_stock();
if !low_stock.is_empty() {
    // Trigger supply chain workflow
    start_supply_chain_workflow(low_stock);
}
```

## Integration with Workflows

```rust
struct InventoryAwareHooks {
    inventory: Arc<Mutex<InventoryManager>>,
}

impl SimulationHooks for InventoryAwareHooks {
    fn on_agents_ticked(&mut self, ctx: &SimulationContext, results: &[AgentTickResult]) -> Result<()> {
        for result in results {
            for demand in &result.demands {
                if demand.demand_type == "food_order" {
                    // Check if we have ingredients
                    let mut inventory = self.inventory.lock().unwrap();

                    match inventory.consume("beef", 1) {
                        Ok(_) => {
                            // Can fulfill order
                        }
                        Err(_) => {
                            // Out of stock - trigger supply order
                            let low_stock = inventory.check_low_stock();
                            self.start_supply_workflow(low_stock, ctx)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
```

## Complete Test

See `/Users/robert.pack/code/management/test_supply_chain.rs` for full example.
