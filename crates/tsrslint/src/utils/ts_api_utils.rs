//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go IsIntrinsicVoidType.
pub fn is_intrinsic_void_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Void)
}

/// ts_api_utils.go IsUndefinedIdentifier.
pub fn is_undefined_identifier(node: P<Node>) -> bool {
    ast::is_identifier(node) && node.as_identifier().text() == "undefined"
}

/// ts_api_utils.go IsVoidExpression.
pub fn is_void_expression(node: P<Node>) -> bool {
    ast::is_void_expression(node)
}

/// ts_api_utils.go IsUndefinedLiteral.
pub fn is_undefined_literal(node: P<Node>) -> bool {
    is_undefined_identifier(node) || is_void_expression(node)
}
