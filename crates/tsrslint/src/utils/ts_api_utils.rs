//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

pub fn is_intrinsic_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Intrinsic)
}

pub fn is_intrinsic_error_type(t: P<Type>) -> bool {
    is_intrinsic_type(t) && t.as_intrinsic_type().intrinsic_name() == "error"
}
