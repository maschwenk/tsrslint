// Port of internal/rules/prefer_string_starts_ends_with/prefer_string_starts_ends_with.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("prefer-string-starts-ends-with"))
}
