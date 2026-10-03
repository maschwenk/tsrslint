//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;

/// ts_eslint.go GetDeclaration: the first declaration of the symbol at `node`.
pub fn get_declaration(c: &mut Checker, node: P<Node>) -> Option<P<Node>> {
    let symbol = c.get_symbol_at_location_exported(node)?;
    symbol.declarations().first().copied()
}

/// ts_eslint.go GetContextualType.
pub fn get_contextual_type(c: &mut Checker, node: P<Node>) -> Option<P<Type>> {
    use tsrs_checker::ContextFlags;
    let parent = node.parent()?;
    if ast::is_call_expression(parent) || ast::is_new_expression(parent) {
        if Some(node) == parent.expression() {
            // is the callee, so has no contextual type
            return None;
        }
    } else if ast::is_variable_declaration(parent)
        || ast::is_property_declaration(parent)
        || ast::is_parameter_declaration(parent)
    {
        return parent.type_node().map(|t| c.get_type_from_type_node(t));
    } else if parent.kind() == Kind::JsxExpression {
        return c.get_contextual_type(parent, ContextFlags::None);
    } else if ast::is_identifier(node)
        && (ast::is_property_assignment(parent) || ast::is_shorthand_property_assignment(parent))
    {
        return c.get_contextual_type(node, ContextFlags::None);
    } else if ast::is_binary_expression(parent)
        && parent.as_binary_expression().operator_token.kind() == Kind::EqualsToken
        && parent.as_binary_expression().right.get() == node
    {
        // is RHS of assignment
        return Some(c.get_type_at_location(parent.as_binary_expression().left));
    } else if parent.kind() != Kind::JsxExpression && !ast::is_template_span(parent) {
        // parent is not something we know we can get the contextual type of
        return None;
    }
    c.get_contextual_type(node, ContextFlags::None)
}
