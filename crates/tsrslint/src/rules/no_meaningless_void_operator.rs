// Port of internal/rules/no_meaningless_void_operator/no_meaningless_void_operator.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-meaningless-void-operator"))
}
