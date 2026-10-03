// Port of internal/rules/related_getter_setter_pairs/related_getter_setter_pairs.go. NOT YET PORTED: create() reports that, and the rule tests skip it.

use crate::rule::Rule;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Err(super::not_ported("related-getter-setter-pairs"))
}
