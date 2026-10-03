//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go IsObjectType.
pub fn is_object_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Object)
}

/// ts_api_utils.go IsIntrinsicType.
pub fn is_intrinsic_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Intrinsic)
}

/// ts_api_utils.go IsIntrinsicErrorType.
pub fn is_intrinsic_error_type(t: P<Type>) -> bool {
    is_intrinsic_type(t)
        && t.data_tag() == tsrs_checker::TypeDataTag::Intrinsic
        && t.as_intrinsic_type().intrinsic_name() == "error"
}
