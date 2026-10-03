//! Ports of internal/utils/utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// utils.go GetNumberIndexType.
pub fn get_number_index_type(c: &mut Checker, t: P<Type>) -> Option<P<Type>> {
    let number_type = c.number_type;
    c.get_index_type_of_type(t, number_type)
}
