//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_eslint.go GetFunctionHeadLoc.
pub fn get_function_head_loc(source_file: P<SourceFile>, node: P<Node>) -> (i32, i32) {
    if ast::is_arrow_function(node) {
        if let Some(arrow) = node.as_arrow_function().equals_greater_than_token {
            return (arrow.pos(), arrow.end());
        }
    }
    let Some(params) = node.parameter_list() else {
        return trim_node_text_range(source_file, node);
    };
    // The parameter list's range starts right after `(`, so `pos() - 1` is the `(`.
    let end = params.pos() - 1;
    // Skip leading decorators; keep modifiers such as `export`, `public`, `async`.
    let mut start = tsrs_scanner::get_token_pos_of_node(node, source_file, false);
    for &m in node.modifier_nodes() {
        if m.kind() != Kind::Decorator {
            start = tsrs_scanner::get_token_pos_of_node(m, source_file, false);
            break;
        }
    }
    if end < start {
        return trim_node_text_range(source_file, node);
    }
    (start, end)
}

const ARRAY_PREDICATE_FUNCTIONS: &[&str] = &[
    "every",
    "filter",
    "find",
    "findIndex",
    "findLast",
    "findLastIndex",
    "some",
];

/// ts_eslint.go IsArrayMethodCallWithPredicate (`node` is the CallExpression).
pub fn is_array_method_call_with_predicate(c: &mut Checker, node: P<Node>) -> bool {
    let callee = node.expression().unwrap();
    if !ast::is_access_expression(callee) {
        return false;
    }
    let (property_name, ok) = c.get_accessed_property_name(callee);
    if !ok || !ARRAY_PREDICATE_FUNCTIONS.contains(&property_name.as_str()) {
        return false;
    }
    let t = get_constrained_type_at_location(c, callee.expression().unwrap());
    type_recurser(t, &mut |t| c.is_array_or_tuple_type(t))
}

/// ts_eslint.go IsRestParameterDeclaration.
pub fn is_rest_parameter_declaration(decl: P<Node>) -> bool {
    ast::is_parameter_declaration(decl)
        && decl
            .as_parameter_declaration()
            .dot_dot_dot_token()
            .is_some()
}

/// ts_eslint.go GetForStatementHeadLoc.
pub fn get_for_statement_head_loc(source_file: P<SourceFile>, node: P<Node>) -> (i32, i32) {
    let statement = if ast::is_for_statement(node) {
        node.as_for_statement().iteration_statement_base.statement
    } else {
        node.as_for_in_or_of_statement().statement
    };
    let (pos, _) = trim_node_text_range(source_file, node);
    (pos, statement.pos())
}

/// ts_eslint.go GetDeclaration.
pub fn get_declaration(c: &mut Checker, node: P<Node>) -> Option<P<Node>> {
    let symbol = c.get_symbol_at_location_exported(node)?;
    symbol.declarations().first().copied()
}
