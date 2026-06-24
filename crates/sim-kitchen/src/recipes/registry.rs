//! Recipe registry for looking up recipes by item name

use super::types::{Recipe, RecipeStep, StationKind};
use std::collections::HashMap;

/// Holds all known recipes, keyed by item name.
pub struct RecipeRegistry {
    recipes: HashMap<String, Recipe>, // item_name -> Recipe
}

impl Default for RecipeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RecipeRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            recipes: HashMap::new(),
        }
    }

    /// Load default recipes from bundled universe templates.
    /// Parses fast_food.json, asian.json, mexican.json etc.
    pub fn load_defaults() -> Self {
        let mut registry = Self::new();

        let templates = [
            (
                "fast_food",
                include_str!("../../../universe/templates/base/brands/fast_food.json"),
            ),
            (
                "asian",
                include_str!("../../../universe/templates/base/brands/asian.json"),
            ),
            (
                "mexican",
                include_str!("../../../universe/templates/base/brands/mexican.json"),
            ),
        ];

        for (category, json_str) in &templates {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
                registry.parse_brand_template(&value, category);
            }
        }

        registry
    }

    fn parse_brand_template(&mut self, value: &serde_json::Value, category: &str) {
        let items = match value.get("items").and_then(|v| v.as_array()) {
            Some(arr) => arr,
            None => return,
        };

        for item in items {
            let name = match item.get("name").and_then(|v| v.as_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            let instructions = match item.get("instructions").and_then(|v| v.as_array()) {
                Some(arr) => arr,
                None => continue,
            };

            let mut steps = Vec::new();
            let mut total_duration: u64 = 0;

            for (i, instr) in instructions.iter().enumerate() {
                let step_name = instr
                    .get("step")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                let description = instr
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let station_str = instr
                    .get("required_station")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let station = StationKind::from_template_str(station_str);

                let duration = instr
                    .get("expected_duration")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);

                total_duration += duration;

                steps.push(RecipeStep {
                    step_index: i,
                    name: step_name,
                    description,
                    station,
                    expected_duration_secs: duration,
                });
            }

            let recipe = Recipe {
                name: name.clone(),
                brand_category: category.to_string(),
                steps,
                total_duration_secs: total_duration,
            };

            self.recipes.insert(name, recipe);
        }
    }

    /// Get a recipe by item name.
    pub fn get(&self, item_name: &str) -> Option<&Recipe> {
        self.recipes.get(item_name)
    }

    /// Get all recipes.
    pub fn all_recipes(&self) -> &HashMap<String, Recipe> {
        &self.recipes
    }

    /// Get recipe count.
    pub fn len(&self) -> usize {
        self.recipes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recipes.is_empty()
    }

    /// Get a random recipe (for orders without specific items).
    pub fn random_recipe(&self, rng: &mut impl rand::Rng) -> Option<&Recipe> {
        if self.recipes.is_empty() {
            return None;
        }
        let keys: Vec<&String> = self.recipes.keys().collect();
        let idx = rng.random_range(0..keys.len());
        self.recipes.get(keys[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_new_is_empty() {
        let registry = RecipeRegistry::new();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_load_defaults_is_not_empty() {
        let registry = RecipeRegistry::load_defaults();
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_get_returns_none_for_unknown() {
        let registry = RecipeRegistry::new();
        assert!(registry.get("nonexistent_item").is_none());
    }
}
