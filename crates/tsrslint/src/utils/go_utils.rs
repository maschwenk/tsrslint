//! Ports of internal/utils/utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_checker::{Type, TypeFlags};

/// utils.go TypeRecurser: the predicate returns true to stop.
pub fn type_recurser(t: P<Type>, predicate: &mut dyn FnMut(P<Type>) -> bool) -> bool {
    if is_type_flag_set(t, TypeFlags::UnionOrIntersection) {
        for &subtype in t.types() {
            if type_recurser(subtype, predicate) {
                return true;
            }
        }
        false
    } else {
        predicate(t)
    }
}
