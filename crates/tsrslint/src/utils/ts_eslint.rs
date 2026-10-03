//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_eslint.go IsParenlessArrowFunction.
pub fn is_parenless_arrow_function(node: P<Node>) -> bool {
    if !ast::is_arrow_function(node) {
        return false;
    }
    let n = node.as_arrow_function();
    node.parameter_list().unwrap().end() == n.equals_greater_than_token.unwrap().pos()
}
