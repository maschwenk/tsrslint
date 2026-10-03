// Port of internal/rules/no_useless_default_assignment/no_useless_default_assignment.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-useless-default-assignment"))
}
