//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_eslint.go IsParenlessArrowFunction.
pub fn is_parenless_arrow_function(node: P<Node>) -> bool {
    if !ast::is_arrow_function(node) {
        return false;
    }
    let n = node.as_arrow_function();
    node.parameter_list().unwrap().end() == n.equals_greater_than_token.unwrap().pos()
}

/// ts_eslint.go MemberNameType.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemberNameType {
    Private,
    Quoted,
    Normal,
    Expression,
}

/// ts_eslint.go GetNameFromMember: a string name representation of a member name, with handling for
/// computed property names.
pub fn get_name_from_member(
    source_file: P<SourceFile>,
    member: P<Node>,
) -> (String, MemberNameType) {
    match member.kind() {
        Kind::Identifier => {
            return (
                member.as_identifier().text().to_string(),
                MemberNameType::Normal,
            )
        }
        Kind::PrivateIdentifier => {
            return (
                member.as_private_identifier().text().to_string(),
                MemberNameType::Private,
            )
        }
        Kind::ComputedPropertyName => {
            let expr = member.as_computed_property_name().expression;
            if ast::is_literal_expression(expr) {
                let text = expr.text();
                if !tsrs_scanner::is_valid_identifier(text) {
                    return (format!("\"{text}\""), MemberNameType::Quoted);
                }
                return (text.to_string(), MemberNameType::Normal);
            }
        }
        _ => {}
    }
    let (pos, end) = trim_node_text_range(source_file, member);
    (
        source_file.text()[pos as usize..end as usize].to_string(),
        MemberNameType::Expression,
    )
}
