// Port of internal/rules/use_unknown_in_catch_callback_variable/use_unknown_in_catch_callback_variable.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("use-unknown-in-catch-callback-variable"))
}
