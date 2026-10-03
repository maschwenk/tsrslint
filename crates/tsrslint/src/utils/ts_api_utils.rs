//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go GetConstructSignatures.
pub fn get_construct_signatures(c: &mut Checker, t: P<Type>) -> &'static [P<Signature>] {
    c.get_signatures_of_type(t, SignatureKind::Construct)
}
