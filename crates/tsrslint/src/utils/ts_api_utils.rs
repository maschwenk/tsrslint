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
