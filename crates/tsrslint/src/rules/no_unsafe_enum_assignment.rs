// Port of internal/rules/no_unsafe_enum_assignment/{no_unsafe_enum_assignment,type_comparison,function_signature}.go.

use rustc_hash::{FxHashMap, FxHashSet};
use tsrs_ast::{self as ast, Kind, Node};
use tsrs_checker::{
    CheckMode, Checker, ContextFlags, ElementFlags, LiteralValue, ObjectFlags, Type, TypeFlags,
};
use tsrs_core::P;

use crate::rule::{Ctx, Listener, Rule, RuleMessage, RuleVisitor};
use crate::utils;

#[derive(Clone, Copy)]
enum Message {
    Access,
    Argument,
    Assertion,
    Assignment,
    Mutation,
    Return,
}

impl Message {
    fn build(self, names: &str) -> RuleMessage {
        let (id, description) = match self {
            Message::Access => (
                "unsafeEnumAccess",
                format!("The computed key used here does not have a shared enum type with the expected enum {names}."),
            ),
            Message::Argument => (
                "unsafeEnumArgument",
                format!("The argument passed here does not have a shared enum type with the expected enum {names}."),
            ),
            Message::Assertion => (
                "unsafeEnumAssertion",
                format!("The value asserted here does not have a shared enum type with the expected enum {names}."),
            ),
            Message::Assignment => (
                "unsafeEnumAssignment",
                format!("The value assigned here does not have a shared enum type with the expected enum {names}."),
            ),
            Message::Mutation => (
                "unsafeEnumMutation",
                format!("This mutation can produce a value outside of the expected enum {names}."),
            ),
            Message::Return => (
                "unsafeEnumReturn",
                format!("The value returned here does not have a shared enum type with the expected enum {names}."),
            ),
        };
        RuleMessage::new(id, description)
    }
}

pub struct NoUnsafeEnumAssignment;

pub fn create(_options: Option<&serde_json::Value>) -> Result<Box<dyn Rule>, String> {
    Ok(Box::new(NoUnsafeEnumAssignment))
}

const LISTENERS: &[Listener] = &[
    Listener::Enter(Kind::VariableDeclaration),
    Listener::Enter(Kind::Parameter),
    Listener::Enter(Kind::BindingElement),
    Listener::Enter(Kind::ShorthandPropertyAssignment),
    Listener::Enter(Kind::PropertyDeclaration),
    Listener::Enter(Kind::BinaryExpression),
    Listener::Enter(Kind::PrefixUnaryExpression),
    Listener::Enter(Kind::PostfixUnaryExpression),
    Listener::Enter(Kind::AsExpression),
    Listener::Enter(Kind::TypeAssertionExpression),
    Listener::Enter(Kind::SatisfiesExpression),
    Listener::Enter(Kind::ReturnStatement),
    Listener::Enter(Kind::ArrowFunction),
    Listener::Enter(Kind::CallExpression),
    Listener::Enter(Kind::NewExpression),
    Listener::Enter(Kind::TaggedTemplateExpression),
    Listener::Enter(Kind::JsxAttribute),
    Listener::NotAllowPattern(Kind::ArrayLiteralExpression),
    Listener::NotAllowPattern(Kind::ObjectLiteralExpression),
    Listener::Enter(Kind::ElementAccessExpression),
];

impl Rule for NoUnsafeEnumAssignment {
    fn name(&self) -> &'static str {
        "no-unsafe-enum-assignment"
    }
    fn create_visitor(&'static self, _ctx: &mut Ctx) -> Box<dyn RuleVisitor> {
        Box::new(Visitor::default())
    }
}

// ---- type_comparison.go ----

const MAXIMUM_TYPE_COMPARISON_DEPTH: i32 = 5;

/// (sender, receiver, within generic members)
type ComparisonVisit = (P<Type>, P<Type>, bool);

/// Go enumTypeChecker: caches that live for one file.
#[derive(Default)]
struct EnumTypeChecker {
    generic_types: FxHashMap<P<Type>, bool>,
    mismatches: FxHashMap<(P<Type>, P<Type>), bool>,
}

fn constraint(c: &mut Checker, t: P<Type>) -> P<Type> {
    c.get_base_constraint_of_type(t).unwrap_or(t)
}

fn type_arguments(c: &mut Checker, t: P<Type>) -> &'static [P<Type>] {
    if utils::is_type_flag_set(t, TypeFlags::Object)
        && t.object_flags().intersects(ObjectFlags::Reference)
    {
        return c.get_type_arguments(t);
    }
    &[]
}

impl EnumTypeChecker {
    fn is_generic(&mut self, c: &mut Checker, t: P<Type>) -> bool {
        if let Some(&generic) = self.generic_types.get(&t) {
            return generic;
        }
        self.generic_types.insert(t, false);
        let mut generic = utils::is_type_flag_set(t, TypeFlags::Instantiable);
        if !generic && utils::is_type_flag_set(t, TypeFlags::UnionOrIntersection) {
            generic = t.types().iter().any(|&part| self.is_generic(c, part));
        }
        if !generic {
            if let Some(alias) = t.alias() {
                generic = alias
                    .type_arguments()
                    .iter()
                    .any(|&argument| self.is_generic(c, argument));
            }
        }
        if !generic {
            generic = type_arguments(c, t)
                .iter()
                .any(|&argument| self.is_generic(c, argument));
        }
        self.generic_types.insert(t, generic);
        generic
    }

    fn mismatch(&mut self, c: &mut Checker, sender: P<Type>, receiver: P<Type>) -> bool {
        if let Some(&mismatch) = self.mismatches.get(&(sender, receiver)) {
            return mismatch;
        }
        let mismatch = self.deep_mismatch(c, sender, receiver, &mut FxHashMap::default(), 0, false);
        self.mismatches.insert((sender, receiver), mismatch);
        mismatch
    }

    fn deep_mismatch(
        &mut self,
        c: &mut Checker,
        sender: P<Type>,
        receiver: P<Type>,
        visited: &mut FxHashMap<ComparisonVisit, i32>,
        depth: i32,
        within_generic_members: bool,
    ) -> bool {
        if depth > MAXIMUM_TYPE_COMPARISON_DEPTH {
            return false;
        }
        let sender = constraint(c, sender);
        let receiver = constraint(c, receiver);
        if sender == receiver {
            return false;
        }
        let visit = (sender, receiver, within_generic_members);
        if visited
            .get(&visit)
            .is_some_and(|&previous_depth| previous_depth <= depth)
        {
            return false;
        }
        visited.insert(visit, depth);
        if direct_mismatch(c, sender, receiver) {
            return true;
        }
        let sender_args = type_arguments(c, sender);
        let receiver_args = type_arguments(c, receiver);
        if sender_args.len() == receiver_args.len() {
            for (&argument, &receiver_argument) in sender_args.iter().zip(receiver_args) {
                if self.deep_mismatch(
                    c,
                    argument,
                    receiver_argument,
                    visited,
                    depth + 1,
                    within_generic_members,
                ) {
                    return true;
                }
            }
        }
        let generic = self.is_generic(c, sender) || self.is_generic(c, receiver);
        if generic && within_generic_members {
            return false;
        }
        let sender_element = utils::get_number_index_type(c, sender);
        let receiver_element = utils::get_number_index_type(c, receiver);
        if let (Some(sender_element), Some(receiver_element)) = (sender_element, receiver_element) {
            if self.deep_mismatch(
                c,
                sender_element,
                receiver_element,
                visited,
                depth + 1,
                generic || within_generic_members,
            ) {
                return true;
            }
        }
        for &receiver_property in c.get_properties_of_type(receiver) {
            let Some(sender_property) = c.get_property_of_type(sender, receiver_property.name())
            else {
                continue;
            };
            let sender_property_type = c.get_type_of_symbol(sender_property);
            let receiver_property_type = c.get_type_of_symbol(receiver_property);
            if self.deep_mismatch(
                c,
                sender_property_type,
                receiver_property_type,
                visited,
                depth + 1,
                generic || within_generic_members,
            ) {
                return true;
            }
        }
        false
    }
}

fn primitive_like(t: P<Type>, flags: TypeFlags) -> bool {
    utils::union_type_parts(t).into_iter().all(|part| {
        utils::intersection_type_parts(part)
            .into_iter()
            .any(|part| utils::is_type_flag_set(part, flags))
    })
}

fn direct_mismatch(c: &mut Checker, sender: P<Type>, receiver: P<Type>) -> bool {
    let receiver_enums = utils::get_enum_types(c, receiver);
    let mut non_enums = Vec::new();
    let mut number_enum = false;
    let mut string_enum = false;
    for part in utils::union_type_parts(receiver) {
        if utils::is_type_flag_set(part, TypeFlags::EnumLike) {
            if utils::is_type_flag_set(part, TypeFlags::NumberLiteral) {
                number_enum = true;
            } else {
                string_enum = true;
            }
        }
        if utils::get_enum_types(c, part).is_empty() {
            non_enums.push(part);
        }
    }
    for part in utils::union_type_parts(sender) {
        if !(number_enum && primitive_like(part, TypeFlags::NumberLike)
            || string_enum && primitive_like(part, TypeFlags::StringLike))
        {
            continue;
        }
        let shared = utils::get_enum_types(c, part)
            .iter()
            .any(|sender_enum| receiver_enums.contains(sender_enum));
        if !shared
            && !non_enums
                .iter()
                .any(|&receiver| c.is_type_assignable_to(part, receiver))
        {
            return true;
        }
    }
    false
}

/// The enum names in `types`, quoted, sorted and joined by ", ".
fn describe(c: &mut Checker, types: &[P<Type>]) -> String {
    fn visit(
        c: &mut Checker,
        t: P<Type>,
        depth: i32,
        names: &mut FxHashSet<String>,
        visited: &mut FxHashMap<P<Type>, i32>,
    ) {
        if depth > MAXIMUM_TYPE_COMPARISON_DEPTH {
            return;
        }
        let t = constraint(c, t);
        if visited
            .get(&t)
            .is_some_and(|&previous_depth| previous_depth <= depth)
        {
            return;
        }
        visited.insert(t, depth);
        for enum_type in utils::get_enum_types(c, t) {
            names.insert(utils::type_to_string(c, enum_type));
        }
        for &argument in type_arguments(c, t) {
            visit(c, argument, depth + 1, names, visited);
        }
        if let Some(element) = utils::get_number_index_type(c, t) {
            visit(c, element, depth + 1, names, visited);
        }
        for &property in c.get_properties_of_type(t) {
            let property_type = c.get_type_of_symbol(property);
            visit(c, property_type, depth + 1, names, visited);
        }
    }
    let mut names = FxHashSet::default();
    let mut visited = FxHashMap::default();
    for &t in types {
        visit(c, t, 0, &mut names, &mut visited);
    }
    let mut result: Vec<String> = names.into_iter().map(|name| format!("'{name}'")).collect();
    result.sort();
    result.join(", ")
}

fn mapped_key_constraint_types(c: &mut Checker, declaration: P<Node>) -> Vec<P<Type>> {
    if !matches!(
        declaration.kind(),
        Kind::GetAccessor
            | Kind::Parameter
            | Kind::PropertyDeclaration
            | Kind::PropertySignature
            | Kind::VariableDeclaration
    ) {
        return Vec::new();
    }
    let Some(type_node) = declaration.type_node() else {
        return Vec::new();
    };
    if ast::is_type_reference_node(type_node) {
        let t = c.get_type_from_type_node(type_node);
        if utils::is_object_type(t) && t.object_flags().intersects(ObjectFlags::Mapped) {
            if let Some(symbol) = t.symbol() {
                for &declaration in symbol.declarations() {
                    if ast::is_mapped_type_node(declaration)
                        && declaration.as_mapped_type_node().name_type.is_some()
                    {
                        return Vec::new();
                    }
                }
            }
            // Resolves the mapped type's members, which sets its constraint type.
            c.get_properties_of_type(t);
            if let Some(constraint) = t.as_mapped_type().constraint_type.get() {
                let constrained = c
                    .get_base_constraint_of_type(constraint)
                    .unwrap_or(constraint);
                if !utils::get_enum_types(c, constrained).is_empty() {
                    return vec![constraint];
                }
            }
        }
    }
    if ast::is_mapped_type_node(type_node) {
        let type_parameter = type_node.as_mapped_type_node().type_parameter;
        if let Some(constraint) = type_parameter.as_type_parameter_declaration().constraint {
            return vec![c.get_type_from_type_node(constraint)];
        }
    }
    let mut result = Vec::new();
    if ast::is_type_literal_node(type_node) {
        for &member in type_node.as_type_literal_node().members.nodes() {
            if let Some(name) = member
                .name()
                .filter(|&name| ast::is_computed_property_name(name))
            {
                result.push(c.get_type_at_location(name.expression().unwrap()));
            }
        }
    }
    result
}

// ---- function_signature.go ----

struct FunctionSignature {
    parameters: Vec<P<Type>>,
    rest: Vec<P<Type>>,
    rest_index: usize,
    index: usize,
    consumed: bool,
}

impl FunctionSignature {
    fn new(c: &mut Checker, node: P<Node>) -> FunctionSignature {
        let signature = c.get_resolved_signature(node, None, CheckMode::Normal);
        let mut result = FunctionSignature {
            parameters: Vec::new(),
            rest: Vec::new(),
            rest_index: 0,
            index: 0,
            consumed: false,
        };
        for (i, &parameter) in signature.parameters().iter().enumerate() {
            let t = c
                .get_type_of_symbol_at_location(parameter, Some(node))
                .unwrap();
            let constrained = constraint(c, t);
            if parameter
                .declarations()
                .first()
                .is_some_and(|&declaration| utils::is_rest_parameter_declaration(declaration))
            {
                result.rest_index = i;
                result.rest = if constrained.is_tuple_type() {
                    type_arguments(c, constrained).to_vec()
                } else if let Some(element) = utils::get_number_index_type(c, constrained) {
                    vec![element]
                } else {
                    vec![constrained]
                };
                break;
            }
            result.parameters.push(t);
        }
        result
    }

    fn next(&mut self) -> Option<P<Type>> {
        let index = self.index;
        self.index += 1;
        if index < self.parameters.len() && !self.consumed {
            return Some(self.parameters[index]);
        }
        if self.rest.is_empty() {
            return None;
        }
        // Without `consumed`, index >= parameters.len() == rest_index here.
        if self.consumed || index - self.rest_index >= self.rest.len() {
            return self.rest.last().copied();
        }
        Some(self.rest[index - self.rest_index])
    }
}

// ---- no_unsafe_enum_assignment.go ----

fn literal_member_name(t: P<Type>) -> Option<String> {
    match t.as_literal_type().value()? {
        LiteralValue::String(value) => Some(value.to_string()),
        LiteralValue::Number(value) => Some(value.to_string()),
        LiteralValue::Boolean(_) | LiteralValue::BigInt(_) => None,
    }
}

/// `(a & b)`, `(a | b)`, `(a ^ b)` and `~a` whose operands are all safe for `receiver`.
fn safe_bitwise(c: &mut Checker, node: P<Node>, receiver: P<Type>) -> bool {
    let node = ast::skip_parentheses(node);
    let safe_operand = |c: &mut Checker, operand: P<Node>| {
        safe_bitwise(c, operand, receiver) || {
            let operand_type = c.get_type_at_location(operand);
            !direct_mismatch(c, operand_type, receiver)
        }
    };
    if ast::is_binary_expression(node) {
        let expr = node.as_binary_expression();
        if matches!(
            expr.operator_token.kind(),
            Kind::AmpersandToken | Kind::CaretToken | Kind::BarToken
        ) {
            return safe_operand(c, expr.left) && safe_operand(c, expr.right.get());
        }
    }
    if ast::is_prefix_unary_expression(node) {
        let expr = node.as_prefix_unary_expression();
        return expr.operator == Kind::TildeToken && safe_operand(c, expr.operand);
    }
    false
}

#[derive(Default)]
struct Visitor {
    types: EnumTypeChecker,
    /// Nodes that a reported assignment already covers; the literal and `satisfies` listeners skip them.
    checked: FxHashSet<P<Node>>,
}

impl Visitor {
    fn report(&mut self, ctx: &mut Ctx, node: P<Node>, message: Message, types: &[P<Type>]) {
        let names = describe(ctx.checker, types);
        let (mut pos, end) = ctx.trim(node);
        if ast::is_parameter_declaration(node) {
            pos = ctx.trim(node.name().unwrap()).0;
        }
        ctx.report_range(pos, end, message.build(&names));
    }

    fn mark_checked(&mut self, node: P<Node>) {
        let node = ast::skip_parentheses(node);
        self.checked.insert(node);
        match node.kind() {
            Kind::ArrayLiteralExpression => {
                for &element in node.as_array_literal_expression().elements.nodes() {
                    self.mark_checked(element);
                }
            }
            Kind::ObjectLiteralExpression => {
                for &property in node.as_object_literal_expression().properties.nodes() {
                    self.mark_checked(property);
                }
            }
            Kind::ConditionalExpression => {
                let expr = node.as_conditional_expression();
                self.mark_checked(expr.when_true);
                self.mark_checked(expr.when_false);
            }
            Kind::BinaryExpression => {
                let expr = node.as_binary_expression();
                if matches!(
                    expr.operator_token.kind(),
                    Kind::AmpersandAmpersandToken | Kind::BarBarToken | Kind::QuestionQuestionToken
                ) {
                    self.mark_checked(expr.left);
                    self.mark_checked(expr.right.get());
                }
            }
            Kind::PropertyAssignment => self.mark_checked(node.initializer().unwrap()),
            Kind::ShorthandPropertyAssignment => self.mark_checked(node.name().unwrap()),
            Kind::SpreadElement
            | Kind::SpreadAssignment
            | Kind::AsExpression
            | Kind::TypeAssertionExpression
            | Kind::NonNullExpression
            | Kind::SatisfiesExpression => self.mark_checked(node.expression().unwrap()),
            _ => {}
        }
    }

    fn unsafe_assignment(
        &mut self,
        c: &mut Checker,
        sender_node: P<Node>,
        receiver: P<Type>,
        sender: Option<P<Type>>,
    ) -> bool {
        let sender = match sender {
            Some(sender) => sender,
            None => c.get_type_at_location(sender_node),
        };
        self.types.mismatch(c, sender, receiver) && !safe_bitwise(c, sender_node, receiver)
    }

    fn check_assignment(
        &mut self,
        ctx: &mut Ctx,
        receiver: P<Type>,
        sender: P<Node>,
        reporting: P<Node>,
        message: Message,
        sender_type: Option<P<Type>>,
    ) {
        let sender = ast::skip_parentheses(sender);
        if ast::is_array_literal_expression(sender)
            || ast::is_object_literal_expression(sender)
            || !self.unsafe_assignment(ctx.checker, sender, receiver, sender_type)
        {
            return;
        }
        self.report(ctx, reporting, message, &[receiver]);
        self.mark_checked(sender);
    }

    fn check_arguments(&mut self, ctx: &mut Ctx, node: P<Node>, args: &[P<Node>], tagged: bool) {
        let mut signature = FunctionSignature::new(ctx.checker, node);
        if tagged {
            signature.next();
        }
        for &argument in args {
            if ast::is_spread_element(argument) {
                let spread_type = ctx
                    .checker
                    .get_type_at_location(argument.expression().unwrap());
                if spread_type.is_tuple_type() {
                    let mut mismatches = Vec::new();
                    for &element in type_arguments(ctx.checker, spread_type) {
                        if let Some(parameter) = signature.next() {
                            if self.types.mismatch(ctx.checker, element, parameter) {
                                mismatches.push(parameter);
                            }
                        }
                    }
                    if !mismatches.is_empty() {
                        self.report(ctx, argument, Message::Argument, &mismatches);
                    }
                    if spread_type
                        .target_tuple_type()
                        .combined_flags
                        .get()
                        .intersects(ElementFlags::Variable)
                    {
                        signature.consumed = true;
                    }
                    continue;
                }
            }
            if let Some(parameter) = signature.next() {
                self.check_assignment(ctx, parameter, argument, argument, Message::Argument, None);
            }
        }
    }

    fn check_return(&mut self, ctx: &mut Ctx, sender: P<Node>, reporting: P<Node>) {
        let Some(function) = utils::get_parent_function_node(sender) else {
            return;
        };
        let signature = ctx
            .checker
            .get_signature_from_declaration_exported(function);
        let mut receiver = Some(ctx.checker.get_return_type_of_signature(signature));
        let mut sender_type = Some(ctx.checker.get_type_at_location(sender));
        if ast::is_async_function(function) {
            receiver = receiver.and_then(|t| ctx.checker.get_awaited_type(t));
            sender_type = sender_type.and_then(|t| ctx.checker.get_awaited_type(t));
        }
        if let (Some(receiver), Some(sender_type)) = (receiver, sender_type) {
            self.check_assignment(
                ctx,
                receiver,
                sender,
                reporting,
                Message::Return,
                Some(sender_type),
            );
        }
    }

    fn check_mutation(&mut self, ctx: &mut Ctx, target: P<Node>, reporting: P<Node>) {
        let t = ctx.checker.get_type_at_location(target);
        let constrained = constraint(ctx.checker, t);
        if !utils::get_enum_types(ctx.checker, constrained).is_empty() {
            self.report(ctx, reporting, Message::Mutation, &[t]);
        }
    }

    fn check_initializer(&mut self, ctx: &mut Ctx, node: P<Node>) {
        if let Some(init) = node.initializer() {
            let receiver = ctx.checker.get_type_at_location(node.name().unwrap());
            self.check_assignment(ctx, receiver, init, node, Message::Assignment, None);
        }
    }

    fn check_assertion(&mut self, ctx: &mut Ctx, node: P<Node>) {
        let receiver = ctx.checker.get_type_at_location(node.type_node().unwrap());
        self.check_assignment(
            ctx,
            receiver,
            node.expression().unwrap(),
            node,
            Message::Assertion,
            None,
        );
    }

    fn property_declaration(&mut self, ctx: &mut Ctx, node: P<Node>) {
        let Some(init) = node.initializer() else {
            return;
        };
        let mut heritage_types = Vec::new();
        let name = node.name().unwrap();
        let member_name = if ast::is_computed_property_name(name) {
            let t = ctx.checker.get_type_at_location(name.expression().unwrap());
            if utils::is_type_flag_set(t, TypeFlags::StringLiteral | TypeFlags::NumberLiteral) {
                literal_member_name(t)
            } else {
                None
            }
        } else if ast::is_identifier(name)
            || ast::is_string_literal(name)
            || ast::is_numeric_literal(name)
        {
            Some(name.text().to_string())
        } else {
            None
        };
        if let Some(member_name) = member_name {
            if let Some(clauses) = utils::get_heritage_clauses(node.parent().unwrap()) {
                for &clause in clauses.nodes() {
                    let clause = clause.as_heritage_clause();
                    if ast::is_static(node) && clause.token != Kind::ExtendsKeyword {
                        continue;
                    }
                    for &heritage in clause.types.get().nodes() {
                        let heritage_node = if ast::is_static(node) {
                            heritage.expression().unwrap()
                        } else {
                            heritage
                        };
                        let heritage_type = ctx.checker.get_type_at_location(heritage_node);
                        if let Some(symbol) = ctx
                            .checker
                            .get_property_of_type(heritage_type, &member_name)
                        {
                            heritage_types.push(ctx.checker.get_type_of_symbol(symbol));
                        }
                    }
                }
            }
        }
        if !heritage_types.is_empty()
            && heritage_types
                .iter()
                .all(|&t| self.unsafe_assignment(ctx.checker, init, t, None))
        {
            self.report(ctx, node, Message::Assignment, &heritage_types);
            self.mark_checked(init);
            return;
        }
        let receiver = ctx.checker.get_type_at_location(node);
        self.check_assignment(ctx, receiver, init, node, Message::Assignment, None);
    }

    fn binary_expression(&mut self, ctx: &mut Ctx, node: P<Node>) {
        if !ast::is_assignment_expression(node, false) {
            return;
        }
        let expr = node.as_binary_expression();
        match expr.operator_token.kind() {
            Kind::EqualsToken
            | Kind::AmpersandAmpersandEqualsToken
            | Kind::AmpersandEqualsToken
            | Kind::QuestionQuestionEqualsToken
            | Kind::CaretEqualsToken
            | Kind::BarEqualsToken
            | Kind::BarBarEqualsToken => {
                let receiver = ctx.checker.get_type_at_location(expr.left);
                self.check_assignment(
                    ctx,
                    receiver,
                    expr.right.get(),
                    node,
                    Message::Assignment,
                    None,
                );
            }
            _ => self.check_mutation(ctx, expr.left, node),
        }
    }

    fn jsx_attribute(&mut self, ctx: &mut Ctx, node: P<Node>) {
        let Some(init) = node
            .initializer()
            .filter(|&init| ast::is_jsx_expression(init))
        else {
            return;
        };
        let Some(expr) = init.expression() else {
            return;
        };
        if let Some(receiver) = ctx.checker.get_contextual_type(expr, ContextFlags::None) {
            self.check_assignment(ctx, receiver, expr, expr, Message::Assignment, None);
        }
    }

    fn array_literal_expression(&mut self, ctx: &mut Ctx, node: P<Node>) {
        if self.checked.contains(&node) {
            return;
        }
        for &element in node.as_array_literal_expression().elements.nodes() {
            if element.kind() == Kind::OmittedExpression {
                continue;
            }
            if let Some(receiver) = ctx.checker.get_contextual_type(element, ContextFlags::None) {
                self.check_assignment(ctx, receiver, element, element, Message::Assignment, None);
            }
        }
    }

    fn object_literal_expression(&mut self, ctx: &mut Ctx, node: P<Node>) {
        if self.checked.contains(&node) {
            return;
        }
        for &property in node.as_object_literal_expression().properties.nodes() {
            let (sender, receiver_node) = match property.kind() {
                Kind::SpreadAssignment => (property.expression().unwrap(), node),
                Kind::PropertyAssignment => {
                    let sender = property.initializer().unwrap();
                    (sender, sender)
                }
                Kind::ShorthandPropertyAssignment => {
                    let sender = property.name().unwrap();
                    (sender, sender)
                }
                _ => continue,
            };
            if let Some(receiver) = ctx
                .checker
                .get_contextual_type(receiver_node, ContextFlags::None)
            {
                self.check_assignment(ctx, receiver, sender, property, Message::Assignment, None);
            }
        }
    }

    fn element_access_expression(&mut self, ctx: &mut Ctx, node: P<Node>) {
        let expr = node.as_element_access_expression();
        let Some(symbol) = ctx.checker.get_symbol_at_location_exported(expr.expression) else {
            return;
        };
        let mut receivers = Vec::new();
        for &declaration in symbol.declarations() {
            receivers.extend(mapped_key_constraint_types(ctx.checker, declaration));
        }
        if receivers.is_empty() {
            return;
        }
        let sender = ctx.checker.get_type_at_location(expr.argument_expression);
        if receivers.iter().all(|&receiver| {
            self.types.mismatch(ctx.checker, sender, receiver)
                || !ctx.checker.is_type_assignable_to(sender, receiver)
        }) {
            self.report(ctx, expr.argument_expression, Message::Access, &receivers);
        }
    }
}

impl RuleVisitor for Visitor {
    fn listeners(&self) -> &[Listener] {
        LISTENERS
    }
    fn visit(&mut self, ctx: &mut Ctx, _l: Listener, node: P<Node>) {
        match node.kind() {
            Kind::VariableDeclaration | Kind::Parameter | Kind::BindingElement => {
                self.check_initializer(ctx, node);
            }
            Kind::ShorthandPropertyAssignment => {
                if let Some(init) = node
                    .as_shorthand_property_assignment()
                    .object_assignment_initializer()
                {
                    let receiver = ctx.checker.get_type_at_location(node.name().unwrap());
                    self.check_assignment(ctx, receiver, init, node, Message::Assignment, None);
                }
            }
            Kind::PropertyDeclaration => self.property_declaration(ctx, node),
            Kind::BinaryExpression => self.binary_expression(ctx, node),
            Kind::PrefixUnaryExpression => {
                let expr = node.as_prefix_unary_expression();
                if matches!(expr.operator, Kind::PlusPlusToken | Kind::MinusMinusToken) {
                    self.check_mutation(ctx, expr.operand, node);
                }
            }
            Kind::PostfixUnaryExpression => {
                self.check_mutation(ctx, node.as_postfix_unary_expression().operand, node);
            }
            Kind::AsExpression | Kind::TypeAssertionExpression => self.check_assertion(ctx, node),
            Kind::SatisfiesExpression => {
                if !self.checked.contains(&node) {
                    let receiver = ctx.checker.get_type_at_location(node.type_node().unwrap());
                    self.check_assignment(
                        ctx,
                        receiver,
                        node.expression().unwrap(),
                        node,
                        Message::Assignment,
                        None,
                    );
                }
            }
            Kind::ReturnStatement => {
                if let Some(expr) = node.expression() {
                    self.check_return(ctx, expr, node);
                }
            }
            Kind::ArrowFunction => {
                if let Some(body) = node.body().filter(|&body| !ast::is_block(body)) {
                    self.check_return(ctx, body, body);
                }
            }
            Kind::CallExpression => self.check_arguments(ctx, node, node.arguments(), false),
            Kind::NewExpression => {
                if node.as_new_expression().arguments.is_some() {
                    self.check_arguments(ctx, node, node.arguments(), false);
                }
            }
            Kind::TaggedTemplateExpression => {
                let template = node.as_tagged_template_expression().template;
                let args: Vec<P<Node>> = if ast::is_template_expression(template) {
                    template
                        .as_template_expression()
                        .template_spans
                        .nodes()
                        .iter()
                        .map(|span| span.expression().unwrap())
                        .collect()
                } else {
                    Vec::new()
                };
                self.check_arguments(ctx, node, &args, true);
            }
            Kind::JsxAttribute => self.jsx_attribute(ctx, node),
            Kind::ArrayLiteralExpression => self.array_literal_expression(ctx, node),
            Kind::ObjectLiteralExpression => self.object_literal_expression(ctx, node),
            Kind::ElementAccessExpression => self.element_access_expression(ctx, node),
            _ => {}
        }
    }
}
