// Port of internal/rules/no_unnecessary_type_conversion/no_unnecessary_type_conversion.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-unnecessary-type-conversion"))
}
