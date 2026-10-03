//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_checker::{Checker, Type, TypeFlags};

/// ts_eslint.go GetForStatementHeadLoc.
pub fn get_for_statement_head_loc(source_file: P<SourceFile>, node: P<Node>) -> (i32, i32) {
    let statement = node.statement();
    let (pos, _) = trim_node_text_range(source_file, node);
    (pos, statement.pos())
}

/// ts_eslint.go GetTypeName.
pub fn get_type_name(c: &mut Checker, t: P<Type>) -> String {
    // It handles `string` and string literal types as string.
    if t.flags().intersects(TypeFlags::StringLike) {
        return "string".to_string();
    }
    // If the type is a type parameter which extends primitive string types,
    // but it was not recognized as a string like. So check the constraint
    // type of the type parameter.
    if is_type_parameter(t) {
        if let Some(symbol) = t.symbol() {
            let decls = symbol.declarations();
            if !decls.is_empty() && ast::is_type_parameter_declaration(decls[0]) {
                if let Some(constraint) = decls[0].as_type_parameter_declaration().constraint() {
                    let ct = c.get_type_from_type_node(constraint);
                    return get_type_name(c, ct);
                }
            }
        }
    }
    // If the type is a union and all types in the union are string like,
    // return `string`.
    if is_union_type(t)
        && union_type_parts(t)
            .into_iter()
            .all(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    // If the type is an intersection and a type in the intersection is string
    // like, return `string`.
    if is_intersection_type(t)
        && intersection_type_parts(t)
            .into_iter()
            .any(|t| get_type_name(c, t) == "string")
    {
        return "string".to_string();
    }
    c.type_to_string_exported(t)
}
