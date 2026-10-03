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

/// ts_eslint.go IsTypeAnyArrayType: true if the type is `any[]`.
pub fn is_type_any_array_type(t: P<Type>, c: &mut Checker) -> bool {
    c.is_array_type(t) && is_type_any_type(c.get_type_arguments(t)[0])
}

/// ts_eslint.go IsTypeUnknownArrayType: true if the type is `unknown[]`.
pub fn is_type_unknown_array_type(t: P<Type>, c: &mut Checker) -> bool {
    c.is_array_type(t) && is_type_unknown_type(c.get_type_arguments(t)[0])
}

/// ts_eslint.go IsUnsafeAssignment: Some((receiver, sender)) if an any is assigned to a non-any type,
/// including in generic positions.
pub fn is_unsafe_assignment(
    t: P<Type>,
    receiver_t: P<Type>,
    c: &mut Checker,
    sender_node: Option<P<Node>>,
) -> Option<(P<Type>, P<Type>)> {
    let mut visited: rustc_hash::FxHashMap<P<Type>, rustc_hash::FxHashSet<P<Type>>> =
        Default::default();
    is_unsafe_assignment_worker(t, receiver_t, c, sender_node, &mut visited)
}

#[allow(clippy::mutable_key_type)]
fn is_unsafe_assignment_worker(
    t: P<Type>,
    receiver: P<Type>,
    c: &mut Checker,
    sender_node: Option<P<Node>>,
    visited: &mut rustc_hash::FxHashMap<P<Type>, rustc_hash::FxHashSet<P<Type>>>,
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

    if let Some(type_already_visited) = visited.get_mut(&t) {
        if type_already_visited.contains(&receiver) {
            return None;
        }
        type_already_visited.insert(receiver);
    } else {
        let mut set = rustc_hash::FxHashSet::default();
        set.insert(receiver);
        visited.insert(t, set);
    }

    if tsrs_checker::is_non_deferred_type_reference(t)
        && tsrs_checker::is_non_deferred_type_reference(receiver)
    {
        if t.target() != receiver.target() {
            // if the type references are different, assume safe, as we won't know how to compare
            // the two types; the generic positions might not be equivalent for both types
            return None;
        }

        if let Some(sender_node) = sender_node {
            if ast::is_new_expression(sender_node)
                && sender_node
                    .expression()
                    .is_some_and(|e| ast::is_identifier(e) && e.text() == "Map")
                && sender_node.arguments().is_empty()
                && sender_node.type_argument_list().is_none()
            {
                // special case to handle `new Map()`: Map's default empty constructor is typed
                // to return `Map<any, any>`
                return None;
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

/// ts_eslint.go GetContextualType: the type of the target the node is going into.
pub fn get_contextual_type(c: &mut Checker, node: P<Node>) -> Option<P<Type>> {
    let parent = node.parent()?;

    if ast::is_call_expression(parent) || ast::is_new_expression(parent) {
        if parent.expression() == Some(node) {
            // is the callee, so has no contextual type
            return None;
        }
    } else if ast::is_variable_declaration(parent)
        || ast::is_property_declaration(parent)
        || ast::is_parameter_declaration(parent)
    {
        return parent.type_node().map(|t| c.get_type_from_type_node(t));
    } else if parent.kind() == Kind::JsxExpression {
        return c.get_contextual_type(parent, tsrs_checker::ContextFlags::None);
    } else if ast::is_identifier(node)
        && (ast::is_property_assignment(parent) || ast::is_shorthand_property_assignment(parent))
    {
        return c.get_contextual_type(node, tsrs_checker::ContextFlags::None);
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
    // TODO - support return statement checking

    c.get_contextual_type(node, tsrs_checker::ContextFlags::None)
}

/// ts_eslint.go GetThisExpression.
pub fn get_this_expression(mut node: P<Node>) -> Option<P<Node>> {
    loop {
        node = ast::skip_parentheses(node);
        if ast::is_call_expression(node) {
            node = node.expression().unwrap();
        } else if node.kind() == Kind::ThisKeyword {
            return Some(node);
        } else if ast::is_access_expression(node) {
            node = node.expression().unwrap();
        } else {
            break;
        }
    }
    None
}

/// ts_eslint.go IsRestParameterDeclaration.
pub fn is_rest_parameter_declaration(decl: P<Node>) -> bool {
    ast::is_parameter_declaration(decl)
        && decl
            .as_parameter_declaration()
            .dot_dot_dot_token()
            .is_some()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DiscriminatedAnyType {
    Any,
    PromiseAny,
    AnyArray,
    Safe,
}

/// ts_eslint.go DiscriminateAnyType: Any if the type is `any`, AnyArray if it is `any[]` or
/// `readonly any[]`, PromiseAny if it is `Promise<any>`, otherwise Safe.
pub fn discriminate_any_type(
    t: P<Type>,
    c: &mut Checker,
    program: &Program,
    node: P<Node>,
) -> DiscriminatedAnyType {
    let mut visited = rustc_hash::FxHashSet::default();
    discriminate_any_type_worker(t, c, program, node, &mut visited)
}

#[allow(clippy::mutable_key_type)]
fn discriminate_any_type_worker(
    t: P<Type>,
    c: &mut Checker,
    program: &Program,
    node: P<Node>,
    // TODO(port): do we really need visited here?
    visited: &mut rustc_hash::FxHashSet<P<Type>>,
) -> DiscriminatedAnyType {
    if visited.contains(&t) {
        return DiscriminatedAnyType::Safe;
    }
    visited.insert(t);
    if is_type_any_type(t) {
        return DiscriminatedAnyType::Any;
    }
    if is_type_any_array_type(t, c) {
        return DiscriminatedAnyType::AnyArray;
    }
    let found_promise_any = type_recurser(t, &mut |t| {
        if !is_thenable_type(c, node, Some(t)) {
            return false;
        }
        let Some(awaited_type) = c.get_awaited_type(t) else {
            return false;
        };
        discriminate_any_type_worker(awaited_type, c, program, node, visited)
            == DiscriminatedAnyType::Any
    });
    if found_promise_any {
        return DiscriminatedAnyType::PromiseAny;
    }
    DiscriminatedAnyType::Safe
}

/// ts_eslint.go GetParentFunctionNode.
pub fn get_parent_function_node(node: P<Node>) -> Option<P<Node>> {
    let mut current = node.parent();
    while let Some(c) = current {
        if ast::is_function_like_declaration(c) {
            return Some(c);
        }
        current = c.parent();
    }
    None
}

/// ts_eslint.go getBaseEnumType: if passed an enum member, returns the type of the parent;
/// otherwise returns itself.
fn get_base_enum_type(c: &mut Checker, t: P<Type>) -> P<Type> {
    let symbol = t.symbol();
    if !is_symbol_flag_set(symbol, SymbolFlags::EnumMember) {
        return t;
    }
    let parent = symbol
        .unwrap()
        .value_declaration()
        .unwrap()
        .parent()
        .unwrap();
    c.get_type_at_location(parent)
}

/// ts_eslint.go GetEnumLiterals: only the enum literals of a type.
pub fn get_enum_literals(t: P<Type>) -> Vec<P<Type>> {
    union_type_parts(t)
        .into_iter()
        .filter(|&sub_type| is_type_flag_set(sub_type, TypeFlags::EnumLiteral))
        .collect()
}

/// ts_eslint.go GetEnumTypes: the enum types of a type (0 or more).
pub fn get_enum_types(c: &mut Checker, t: P<Type>) -> Vec<P<Type>> {
    get_enum_literals(t)
        .into_iter()
        .map(|t| get_base_enum_type(c, t))
        .collect()
}
