// Port of internal/rules/non_nullable_type_assertion_style/non_nullable_type_assertion_style.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("non-nullable-type-assertion-style"))
}
