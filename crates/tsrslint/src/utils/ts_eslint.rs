//! Ports of internal/utils/ts_eslint.go helpers, added as rules need them.

#[allow(unused_imports)]
use super::*;
use rustc_hash::{FxHashMap, FxHashSet};

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

/// ts_eslint.go IsUnsafeAssignment: Some((receiver, sender)) if assigning t to receiver_t is unsafe
/// (any leaks into a non-any receiver, possibly through generic type arguments).
pub fn is_unsafe_assignment(
    t: P<Type>,
    receiver_t: P<Type>,
    c: &mut Checker,
    sender_node: Option<P<Node>>,
) -> Option<(P<Type>, P<Type>)> {
    is_unsafe_assignment_worker(t, receiver_t, c, sender_node, &mut FxHashMap::default())
}

#[allow(clippy::mutable_key_type)]
fn is_unsafe_assignment_worker(
    t: P<Type>,
    receiver: P<Type>,
    c: &mut Checker,
    sender_node: Option<P<Node>>,
    visited: &mut FxHashMap<P<Type>, FxHashSet<P<Type>>>,
) -> Option<(P<Type>, P<Type>)> {
    if is_type_any_type(t) {
        // Allow assignment of any ==> unknown.
        if is_type_unknown_type(receiver) {
            return None;
        }
        if !is_type_any_type(receiver) {
            return Some((receiver, t));
        }
    }

    if let Some(seen) = visited.get_mut(&t) {
        if seen.contains(&receiver) {
            return None;
        }
        seen.insert(receiver);
    } else {
        let mut s = FxHashSet::default();
        s.insert(receiver);
        visited.insert(t, s);
    }

    if tsrs_checker::is_non_deferred_type_reference(t)
        && tsrs_checker::is_non_deferred_type_reference(receiver)
    {
        // TODO - figure out how to handle cases like this, where the types are assignable, but not
        // the same type (see ts_eslint.go).
        if t.target() != receiver.target() {
            // if the type references are different, assume safe, as we won't know how to compare the
            // two types; the generic positions might not be equivalent for both types
            return None;
        }

        if let Some(sender) = sender_node {
            if ast::is_new_expression(sender) {
                let callee = sender.expression().unwrap();
                if ast::is_identifier(callee)
                    && callee.text() == "Map"
                    && sender.arguments().is_empty()
                    && sender.type_argument_list().is_none()
                {
                    // special case to handle new Map(): Map's default empty constructor is typed to
                    // return Map<any, any> (typescript-eslint#2109)
                    return None;
                }
            }
        }

        let type_arguments = c.get_type_arguments(t);
        let receiver_type_arguments = c.get_type_arguments(receiver);
        for (i, &arg) in type_arguments.iter().enumerate() {
            let receiver_arg = receiver_type_arguments[i];
            if is_unsafe_assignment_worker(arg, receiver_arg, c, sender_node, visited).is_some() {
                return Some((receiver, t));
            }
        }
        return None;
    }

    None
}
