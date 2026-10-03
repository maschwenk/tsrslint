// Port of internal/rules/no_unnecessary_template_expression/no_unnecessary_template_expression.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("no-unnecessary-template-expression"))
}
