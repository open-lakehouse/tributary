//! Package agents

use geo::Point;
use serde_json::{Value, json};
use sim_core::{
    Agent, AgentId, AgentStatus, Result, SimulationContext, population::AgentTickResult,
};

/// Delivery priority; higher priorities are allocated first at every step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePriority {
    Standard,
    Express,
    Overnight,
}

impl PackagePriority {
    /// Task priority used by the allocator (higher = sooner)
    pub fn rank(self) -> i32 {
        match self {
            Self::Standard => 0,
            Self::Express => 1,
            Self::Overnight => 2,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Express => "express",
            Self::Overnight => "overnight",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "standard" => Self::Standard,
            "express" => Self::Express,
            "overnight" => Self::Overnight,
            _ => return None,
        })
    }
}

/// Where a package is in its shipment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageStage {
    /// Created, not yet received at the warehouse
    Created,
    /// Received at the warehouse
    Received,
    /// Sorted for delivery
    Sorted,
    /// Loaded and on its way
    InTransit,
    /// Delivered
    Delivered,
}

impl AgentStatus for PackageStage {
    fn as_str(&self) -> &str {
        match self {
            Self::Created => "created",
            Self::Received => "received",
            Self::Sorted => "sorted",
            Self::InTransit => "in_transit",
            Self::Delivered => "delivered",
        }
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Delivered)
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Delivered)
    }
}

impl PackageStage {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "created" => Self::Created,
            "received" => Self::Received,
            "sorted" => Self::Sorted,
            "in_transit" => Self::InTransit,
            "delivered" => Self::Delivered,
            _ => return None,
        })
    }
}

/// A package being shipped from a warehouse to a delivery address
///
/// Packages are passive: their stage is advanced by
/// [`LogisticsSimulation`](crate::LogisticsSimulation) as the package's
/// shipment workflow progresses. Its position is the warehouse until it is
/// delivered, then the destination.
#[derive(Debug, Clone)]
pub struct Package {
    pub id: AgentId,
    pub tracking_number: String,
    pub warehouse: String,
    pub origin: Point<f64>,
    pub destination: Point<f64>,
    pub weight_kg: f64,
    pub priority: PackagePriority,
    pub stage: PackageStage,
}

impl Package {
    /// A new package at `origin`, bound for `destination`
    pub fn new(
        tracking_number: impl Into<String>,
        warehouse: impl Into<String>,
        origin: Point<f64>,
        destination: Point<f64>,
        weight_kg: f64,
        priority: PackagePriority,
    ) -> Self {
        Self {
            id: AgentId::new(),
            tracking_number: tracking_number.into(),
            warehouse: warehouse.into(),
            origin,
            destination,
            weight_kg,
            priority,
            stage: PackageStage::Created,
        }
    }
}

impl Agent for Package {
    fn id(&self) -> AgentId {
        self.id
    }

    fn agent_type(&self) -> &str {
        "package"
    }

    fn status(&self) -> &dyn AgentStatus {
        &self.stage
    }

    fn position(&self) -> Option<Point<f64>> {
        Some(if self.stage == PackageStage::Delivered {
            self.destination
        } else {
            self.origin
        })
    }

    fn tick(&mut self, _ctx: &SimulationContext) -> Result<AgentTickResult> {
        Ok(AgentTickResult::default())
    }

    fn state_data(&self) -> Value {
        json!({
            "tracking_number": self.tracking_number,
            "warehouse": self.warehouse,
            "origin": [self.origin.x(), self.origin.y()],
            "destination": [self.destination.x(), self.destination.y()],
            "weight_kg": self.weight_kg,
            "priority": self.priority.as_str(),
        })
    }

    fn from_arrow_row(row: &sim_core::population::ArrowAgentRow) -> Result<Self> {
        let bad = |what: &str| sim_core::Error::Agent(format!("package row: bad {what}"));
        let id = AgentId::from_bytes(&row.id).map_err(|_| bad("id"))?;
        let meta: Value = row
            .metadata
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?
            .ok_or_else(|| bad("metadata"))?;
        let text = |key: &str| {
            meta.get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| bad(key))
        };
        let point = |key: &str| {
            let xy = meta
                .get(key)
                .and_then(Value::as_array)
                .ok_or_else(|| bad(key))?;
            match (
                xy.first().and_then(Value::as_f64),
                xy.get(1).and_then(Value::as_f64),
            ) {
                (Some(x), Some(y)) => Ok(Point::new(x, y)),
                _ => Err(bad(key)),
            }
        };
        Ok(Self {
            id,
            tracking_number: text("tracking_number")?,
            warehouse: text("warehouse")?,
            origin: point("origin")?,
            destination: point("destination")?,
            weight_kg: meta
                .get("weight_kg")
                .and_then(Value::as_f64)
                .ok_or_else(|| bad("weight_kg"))?,
            priority: PackagePriority::parse(&text("priority")?).ok_or_else(|| bad("priority"))?,
            stage: PackageStage::parse(&row.status).ok_or_else(|| bad("status"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrow_round_trip_keeps_every_field() {
        let mut pkg = Package::new(
            "PKG-000001",
            "Warehouse North",
            Point::new(-122.4, 37.8),
            Point::new(-122.3, 37.75),
            2.5,
            PackagePriority::Express,
        );
        pkg.stage = PackageStage::InTransit;
        let back = Package::from_arrow_row(&pkg.to_arrow_row()).unwrap();
        assert_eq!(back.id, pkg.id);
        assert_eq!(back.tracking_number, pkg.tracking_number);
        assert_eq!(back.warehouse, pkg.warehouse);
        assert_eq!(back.destination, pkg.destination);
        assert_eq!(back.weight_kg, 2.5);
        assert_eq!(back.priority, PackagePriority::Express);
        assert_eq!(back.stage, PackageStage::InTransit);
    }

    #[test]
    fn position_moves_to_destination_on_delivery() {
        let mut pkg = Package::new(
            "PKG-1",
            "W",
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            1.0,
            PackagePriority::Standard,
        );
        assert_eq!(pkg.position(), Some(Point::new(0.0, 0.0)));
        pkg.stage = PackageStage::Delivered;
        assert_eq!(pkg.position(), Some(Point::new(1.0, 1.0)));
    }
}
