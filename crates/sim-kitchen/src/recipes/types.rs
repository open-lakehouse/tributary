//! Recipe types for the ghost kitchen simulation

/// Identifies which kitchen station is required for a recipe step.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub enum StationKind {
    Workstation,
    Stove,
    Fryer,
    Oven,
    Grill,
    Prep,
    Unknown(String),
}

impl StationKind {
    /// Parse from template string like "KITCHEN_STATION_WORKSTATION"
    pub fn from_template_str(s: &str) -> Self {
        match s {
            "KITCHEN_STATION_WORKSTATION" => StationKind::Workstation,
            "KITCHEN_STATION_STOVE" => StationKind::Stove,
            "KITCHEN_STATION_FRYER" => StationKind::Fryer,
            "KITCHEN_STATION_OVEN" => StationKind::Oven,
            "KITCHEN_STATION_GRILL" => StationKind::Grill,
            "KITCHEN_STATION_PREP" => StationKind::Prep,
            other => StationKind::Unknown(other.to_string()),
        }
    }
}

/// A single step in a recipe, to be performed at a specific kitchen station.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecipeStep {
    pub step_index: usize,
    pub name: String,
    pub description: String,
    pub station: StationKind,
    pub expected_duration_secs: u64,
}

/// A complete recipe with all steps required to prepare an item.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Recipe {
    pub name: String,
    pub brand_category: String,
    pub steps: Vec<RecipeStep>,
    pub total_duration_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_station_kind_from_workstation() {
        assert_eq!(
            StationKind::from_template_str("KITCHEN_STATION_WORKSTATION"),
            StationKind::Workstation
        );
    }

    #[test]
    fn test_station_kind_from_unknown() {
        assert!(matches!(
            StationKind::from_template_str("UNKNOWN_TYPE"),
            StationKind::Unknown(_)
        ));
    }
}
