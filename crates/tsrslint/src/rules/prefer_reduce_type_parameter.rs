// Port of internal/rules/prefer_reduce_type_parameter/prefer_reduce_type_parameter.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("prefer-reduce-type-parameter"))
}
