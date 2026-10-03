//! Ports of internal/utils/utils.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

use tsrs_ast::CommentRange;
use tsrs_core::TextRange;

/// utils.go GetCommentsInRange: trailing then leading comment ranges starting at in_range.pos, stopping at
/// in_range.end.
pub fn get_comments_in_range(source_file: P<SourceFile>, in_range: TextRange) -> Vec<CommentRange> {
    let text = source_file.text();
    let mut out = Vec::new();
    for comment_range in tsrs_scanner::get_trailing_comment_ranges(text, in_range.pos()) {
        if comment_range.pos() >= in_range.end() {
            break;
        }
        out.push(comment_range);
    }
    for comment_range in tsrs_scanner::get_leading_comment_ranges(text, in_range.pos()) {
        if comment_range.pos() >= in_range.end() {
            break;
        }
        out.push(comment_range);
    }
    out
}

/// utils.go HasCommentsInRange.
pub fn has_comments_in_range(source_file: P<SourceFile>, in_range: TextRange) -> bool {
    !get_comments_in_range(source_file, in_range).is_empty()
}

/// utils.go TypeRecurser: predicate returns "should stop".
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
