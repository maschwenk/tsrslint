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

/// ts_api_utils.go GetConstructSignatures.
pub fn get_construct_signatures(c: &mut Checker, t: P<Type>) -> &'static [P<Signature>] {
    c.get_signatures_of_type(t, SignatureKind::Construct)
}

/// ts_api_utils.go GetWellKnownSymbolPropertyOfType.
pub fn get_well_known_symbol_property_of_type(
    t: P<Type>,
    name: &str,
    c: &mut Checker,
) -> Option<P<Symbol>> {
    let property_name = c.get_property_name_for_known_symbol_name(name);
    c.get_property_of_type(t, &property_name)
}

/// ts_api_utils.go IsIntrinsicVoidType.
pub fn is_intrinsic_void_type(t: P<Type>) -> bool {
    is_type_flag_set(t, TypeFlags::Void)
}

/// ts_api_utils.go CollectAllCallSignatures (ex. getCallSignaturesOfType).
pub fn collect_all_call_signatures(c: &mut Checker, t: P<Type>) -> Vec<P<Signature>> {
    if is_union_type(t) {
        let mut signatures = Vec::new();
        for &subtype in t.types() {
            signatures.extend_from_slice(get_call_signatures(c, subtype));
        }
        return signatures;
    }
    if is_intersection_type(t) {
        let mut signatures: Option<&'static [P<Signature>]> = None;
        for &subtype in t.types() {
            let sig = get_call_signatures(c, subtype);
            if !sig.is_empty() {
                if signatures.is_some() {
                    return Vec::new();
                }
                signatures = Some(sig);
            }
        }
        return signatures.map(|s| s.to_vec()).unwrap_or_default();
    }
    c.get_signatures_of_type(t, SignatureKind::Call).to_vec()
}
