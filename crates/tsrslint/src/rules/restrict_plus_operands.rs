// Port of internal/rules/restrict_plus_operands/restrict_plus_operands.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("restrict-plus-operands"))
}
