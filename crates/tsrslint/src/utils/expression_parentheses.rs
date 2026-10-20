//! Port of internal/utils/expression_parentheses.go.

use super::*;

/// expression_parentheses.go IsStartOfExpressionStatementNeedingParentheses. Matching start positions
/// limits traversal to the beginning of a statement and stops at existing parentheses.
pub fn is_start_of_expression_statement_needing_parentheses(
    source_file: P<SourceFile>,
    node: P<Node>,
    first_token: Kind,
) -> bool {
    if !matches!(
        first_token,
        Kind::OpenBraceToken | Kind::ClassKeyword | Kind::FunctionKeyword
    ) {
        return false;
    }
    let start = trim_node_text_range(source_file, node).0;
    let mut ancestor = node.parent();
    while let Some(a) = ancestor {
        if trim_node_text_range(source_file, a).0 != start {
            break;
        }
        if ast::is_expression_statement(a) {
            return true;
        }
        ancestor = a.parent();
    }
    false
}

/// expression_parentheses.go IsStartOfArrowFunctionBodyNeedingParentheses.
pub fn is_start_of_arrow_function_body_needing_parentheses(
    source_file: P<SourceFile>,
    node: P<Node>,
    first_token: Kind,
) -> bool {
    if first_token != Kind::OpenBraceToken {
        return false;
    }
    let mut current = node;
    while let Some(parent) = current.parent() {
        if ast::is_parenthesized_expression(parent) {
            return false;
        }
        if ast::is_arrow_function(parent) && parent.body() == Some(current) {
            return true;
        }
        if trim_node_text_range(source_file, parent).0
            != trim_node_text_range(source_file, current).0
        {
            return false;
        }
        current = parent;
    }
    false
}
