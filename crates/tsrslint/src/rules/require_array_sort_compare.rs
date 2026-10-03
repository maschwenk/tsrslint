// Port of internal/rules/require_array_sort_compare/require_array_sort_compare.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("require-array-sort-compare"))
}
