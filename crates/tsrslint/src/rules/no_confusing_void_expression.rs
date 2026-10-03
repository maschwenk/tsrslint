// Port of internal/rules/no_confusing_void_expression/no_confusing_void_expression.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-confusing-void-expression"))
}
