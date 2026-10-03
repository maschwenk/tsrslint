//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go IsNullableType.
pub fn is_nullable_type(c: &mut Checker, t: P<Type>) -> bool {
    c.is_nullable_type(t)
}

/// ts_api_utils.go GetConstructSignatures.
pub fn get_construct_signatures(c: &mut Checker, t: P<Type>) -> &'static [P<Signature>] {
    c.get_signatures_of_type(t, SignatureKind::Construct)
}

/// ts_api_utils.go IsNullLiteral.
pub fn is_null_literal(node: Option<P<Node>>) -> bool {
    node.is_some_and(|n| n.kind() == Kind::NullKeyword)
}

/// ts_api_utils.go IsUndefinedIdentifier.
pub fn is_undefined_identifier(node: Option<P<Node>>) -> bool {
    node.is_some_and(|n| ast::is_identifier(n) && n.text() == "undefined")
}

/// ts_api_utils.go IsNullLiteralOrUndefinedIdentifier.
pub fn is_null_literal_or_undefined_identifier(node: Option<P<Node>>) -> bool {
    is_null_literal(node) || is_undefined_identifier(node)
}
