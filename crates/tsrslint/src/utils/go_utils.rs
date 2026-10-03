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

/// utils.go GetNumberIndexType.
pub fn get_number_index_type(c: &mut Checker, t: P<Type>) -> Option<P<Type>> {
    let number_type = c.number_type;
    c.get_index_type_of_type(t, number_type)
}

/// utils.go GetCommentsInRange: trailing then leading comment ranges starting at `pos` that begin
/// before `end`.
pub fn get_comments_in_range(file: P<SourceFile>, pos: i32, end: i32) -> Vec<ast::CommentRange> {
    let mut out = Vec::new();
    for c in tsrs_scanner::get_trailing_comment_ranges(file.text(), pos) {
        if c.pos() >= end {
            break;
        }
        out.push(c);
    }
    for c in tsrs_scanner::get_leading_comment_ranges(file.text(), pos) {
        if c.pos() >= end {
            break;
        }
        out.push(c);
    }
    out
}

/// utils.go HasCommentsInRange.
pub fn has_comments_in_range(file: P<SourceFile>, pos: i32, end: i32) -> bool {
    !get_comments_in_range(file, pos, end).is_empty()
}

/// utils.go IsStrWhiteSpace (jsnum's StrWhiteSpaceChar: LineTerminator, WhiteSpace, Unicode Zs).
pub fn is_str_white_space(r: char) -> bool {
    matches!(
        r,
        '\n' | '\r'
            | '\u{2028}'
            | '\u{2029}'
            | '\t'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{FEFF}'
            // Unicode category Zs
            | ' '
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}
