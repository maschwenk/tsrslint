//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

const ARRAY_PREDICATE_FUNCTIONS: &[&str] = &[
    "every",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
];

/// ts_eslint.go IsArrayMethodCallWithPredicate (node is a CallExpression).
pub fn is_array_method_call_with_predicate(c: &mut Checker, node: P<Node>) -> bool {
    let expression = node.as_call_expression().expression;
    if !ast::is_access_expression(expression) {
        return false;
    }
    let (property_name, ok) = c.get_accessed_property_name(expression);
    if !ok || !ARRAY_PREDICATE_FUNCTIONS.contains(&property_name.as_str()) {
        return false;
    }
    let t = get_constrained_type_at_location(c, expression.expression().unwrap());
    type_recurser(t, &mut |t| c.is_array_or_tuple_type(t))
}
