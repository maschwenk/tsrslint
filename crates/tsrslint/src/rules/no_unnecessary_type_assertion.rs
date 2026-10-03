// Port of internal/rules/no_unnecessary_type_assertion/no_unnecessary_type_assertion.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-unnecessary-type-assertion"))
}
