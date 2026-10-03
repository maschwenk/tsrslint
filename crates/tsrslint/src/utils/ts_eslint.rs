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

/// ts_eslint.go GetTypeName.
pub fn get_type_name(c: &mut Checker, t: P<Type>) -> String {
    // It handles `string` and string literal types as string.
    if t.flags().intersects(TypeFlags::StringLike) {
        return "string".to_string();
    }
    // If the type is a type parameter which extends primitive string types,
    // but it was not recognized as a string like. So check the constraint
    // type of the type parameter.
    if is_type_parameter(t) {
        if let Some(symbol) = t.symbol() {
            if let Some(&decl) = symbol.declarations().first() {
                if ast::is_type_parameter_declaration(decl) {
                    if let Some(constraint) = decl.as_type_parameter_declaration().constraint {
                        let ct = c.get_type_from_type_node_exported(constraint);
                        return get_type_name(c, ct);
                    }
                }
            }
        }
    }
    // If the type is a union and all types in the union are string like,
    // return `string`.
    if is_union_type(t)
        && union_type_parts(t)
            .into_iter()
            .all(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    // If the type is an intersection and a type in the intersection is string
    // like, return `string`.
    if is_intersection_type(t)
        && intersection_type_parts(t)
            .into_iter()
            .any(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    c.type_to_string_exported(t)
}
