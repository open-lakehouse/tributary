use chrono::{DateTime, Utc};
use geo::Point;
use serde::{Deserialize, Serialize};

use crate::idents::{BrandId, KitchenId, MenuItemId, OrderId, OrderLineId, PersonId, SiteId};
use crate::state::{OrderLineStatus, OrderStatus, PersonStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: DateTime<Utc>,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCreatedPayload {
    pub site_id: SiteId,
    pub person_id: PersonId,
    pub items: Vec<(BrandId, MenuItemId)>,
    pub destination: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonUpdatedPayload {
    pub person_id: PersonId,
    pub status: PersonStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderUpdatedPayload {
    pub order_id: OrderId,
    pub status: OrderStatus,
    pub actor_id: Option<PersonId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderLineUpdatedPayload {
    pub order_line_id: OrderLineId,
    pub status: OrderLineStatus,
    pub kitchen_id: Option<KitchenId>,
    pub actor_id: Option<PersonId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventPayload {
    PersonUpdated(PersonUpdatedPayload),
    OrderUpdated(OrderUpdatedPayload),
    OrderLineUpdated(OrderLineUpdatedPayload),
    OrderCreated(OrderCreatedPayload),
}

impl EventPayload {
    pub fn person_updated(person_id: PersonId, status: PersonStatus) -> Self {
        Self::PersonUpdated(PersonUpdatedPayload { person_id, status })
    }

    pub fn order_updated(
        order_id: OrderId,
        status: OrderStatus,
        actor_id: Option<PersonId>,
    ) -> Self {
        Self::OrderUpdated(OrderUpdatedPayload {
            order_id,
            status,
            actor_id,
        })
    }

    pub fn order_line_updated(
        order_line_id: OrderLineId,
        status: OrderLineStatus,
        kitchen_id: Option<KitchenId>,
        actor_id: Option<PersonId>,
    ) -> Self {
        Self::OrderLineUpdated(OrderLineUpdatedPayload {
            order_line_id,
            status,
            kitchen_id,
            actor_id,
        })
    }

    pub fn order_failed(order_id: OrderId, actor_id: Option<PersonId>) -> Self {
        Self::OrderUpdated(OrderUpdatedPayload {
            order_id,
            status: OrderStatus::Failed,
            actor_id,
        })
    }
}

#[derive(Debug)]
pub struct EventStats {
    pub num_orders_created: u32,
    pub num_orders_updated: u32,
    pub num_order_lines_updated: u32,
    pub num_people_updated: u32,
}

impl Default for EventStats {
    fn default() -> Self {
        Self::new()
    }
}

impl EventStats {
    pub fn new() -> Self {
        Self {
            num_orders_created: 0,
            num_orders_updated: 0,
            num_order_lines_updated: 0,
            num_people_updated: 0,
        }
    }

    pub fn add(&mut self, other: &EventStats) {
        self.num_orders_created += other.num_orders_created;
        self.num_orders_updated += other.num_orders_updated;
        self.num_order_lines_updated += other.num_order_lines_updated;
        self.num_people_updated += other.num_people_updated;
    }

    pub fn handle_event(&mut self, event: &EventPayload) {
        match event {
            EventPayload::OrderCreated(_) => self.num_orders_created += 1,
            EventPayload::OrderUpdated(_) => self.num_orders_updated += 1,
            EventPayload::OrderLineUpdated(_) => self.num_order_lines_updated += 1,
            EventPayload::PersonUpdated(_) => self.num_people_updated += 1,
        }
    }
}
