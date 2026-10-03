//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go IsTypeNullType.
pub fn is_type_null_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Null)
}

/// ts_api_utils.go IsTypeUndefinedType.
pub fn is_type_undefined_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Undefined)
}

/// ts_api_utils.go IsTypeVoidType.
pub fn is_type_void_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Void)
}

/// ts_api_utils.go IsNullLiteral.
pub fn is_null_literal(node: P<Node>) -> bool {
    node.kind() == Kind::NullKeyword
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

/// ts_api_utils.go IsNullishLiteral.
pub fn is_nullish_literal(node: P<Node>) -> bool {
    is_null_literal(node) || is_undefined_literal(node)
}

/// ts_api_utils.go IsPropertyOrElementAccess.
pub fn is_property_or_element_access(node: P<Node>) -> bool {
    ast::is_property_access_expression(node) || ast::is_element_access_expression(node)
}

/// ts_api_utils.go IsAccessExpression.
pub fn is_access_expression(node: P<Node>) -> bool {
    ast::is_property_access_expression(node)
        || ast::is_element_access_expression(node)
        || ast::is_call_expression(node)
}

/// ts_api_utils.go IsIntrinsicType.
pub fn is_intrinsic_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Intrinsic)
}

/// ts_api_utils.go IsIntrinsicErrorType.
pub fn is_intrinsic_error_type(t: P<Type>) -> bool {
    is_intrinsic_type(t) && t.as_intrinsic_type().intrinsic_name() == "error"
}
