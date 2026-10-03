//! Ports of internal/utils/utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

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

/// utils.go GetHeritageClauses.
pub fn get_heritage_clauses(node: P<Node>) -> Option<P<ast::NodeList>> {
    match node.kind() {
        Kind::ClassDeclaration => node.as_class_declaration().heritage_clauses(),
        Kind::ClassExpression => node.as_class_expression().heritage_clauses(),
        Kind::InterfaceDeclaration => node.as_interface_declaration().heritage_clauses(),
        _ => None,
    }
}
