//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_checker::{Type, TypeFlags};

/// ts_api_utils.go IsTypeUndefinedType.
pub fn is_type_undefined_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Undefined)
}

/// ts_api_utils.go IsUndefinedIdentifier.
pub fn is_undefined_identifier(node: Option<P<Node>>) -> bool {
    node.is_some_and(|n| ast::is_identifier(n) && n.text() == "undefined")
}

/// ts_api_utils.go IsNullLiteral.
pub fn is_null_literal(node: Option<P<Node>>) -> bool {
    node.is_some_and(|n| n.kind() == Kind::NullKeyword)
}

/// ts_api_utils.go IsVoidExpression.
pub fn is_void_expression(node: Option<P<Node>>) -> bool {
    node.is_some_and(ast::is_void_expression)
}

/// ts_api_utils.go IsUndefinedLiteral.
pub fn is_undefined_literal(node: Option<P<Node>>) -> bool {
    is_undefined_identifier(node) || is_void_expression(node)
}
