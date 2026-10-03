//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_eslint.go GetTypeName.
pub fn get_type_name(c: &mut Checker, t: P<Type>) -> String {
    // It handles `string` and string literal types as string.
    if t.flags().intersects(TypeFlags::StringLike) {
        return "string".to_string();
    }
    // If the type is a type parameter which extends primitive string types, but it was not
    // recognized as a string like, check the constraint type via the AST.
    if is_type_parameter(t) {
        if let Some(symbol) = t.symbol() {
            let decls = symbol.declarations();
            if let Some(&decl) = decls.first() {
                if ast::is_type_parameter_declaration(decl) {
                    if let Some(constraint) = decl.as_type_parameter_declaration().constraint {
                        let ct = c.get_type_from_type_node(constraint);
                        return get_type_name(c, ct);
                    }
                }
            }
        }
    }
    if is_union_type(t)
        && union_type_parts(t)
            .into_iter()
            .all(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    if is_intersection_type(t)
        && intersection_type_parts(t)
            .into_iter()
            .any(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    c.type_to_string_exported(t)
}
