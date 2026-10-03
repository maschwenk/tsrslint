//! Ports of internal/utils/ts_api_utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_api_utils.go IsObjectType.
pub fn is_object_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Object)
}
