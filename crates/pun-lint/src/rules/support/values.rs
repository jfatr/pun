use mago_phpdoc_syntax::PHPDocParser;
use mago_phpdoc_syntax::cst::TagValue;
use mago_syntax::cst::*;
use pun_tokens::php_compat::intval;

use crate::Context;
use crate::problem::Span;
use crate::rules::support::docblocks::doc_comment;
use crate::rules::support::expressions::named;
use crate::rules::support::names::class_name;
use crate::rules::support::statement_edits::previous_token_end;
use crate::syntax::{node_span, unparenthesize};

pub(crate) fn is_object_of(
    ancestors: &[Node<'_, '_>],
    expr: &Expression<'_>,
    class: &str,
    at: usize,
) -> bool {
    match known(ancestors, expr, at) {
        Some(Known::Hint(Hint::Identifier(id))) => {
            class_name(ancestors, id).eq_ignore_ascii_case(class)
        }
        Some(Known::Value(Expression::Instantiation(new))) => instantiates(ancestors, new, class),
        _ => false,
    }
}

pub(crate) enum Known<'a> {
    Hint(&'a Hint<'a>),
    Value(&'a Expression<'a>),
}

pub(crate) fn known<'a>(
    ancestors: &[Node<'a, '_>],
    expr: &'a Expression<'a>,
    at: usize,
) -> Option<Known<'a>> {
    known_assigned(ancestors, expr, at).map(|(known, _)| known)
}

/// `known`, with the span of the assignment the value comes from.
fn known_assigned<'a>(
    ancestors: &[Node<'a, '_>],
    expr: &'a Expression<'a>,
    at: usize,
) -> Option<(Known<'a>, Option<Span>)> {
    let expr = unparenthesize(expr);
    let Expression::Variable(Variable::Direct(var)) = expr else {
        return Some((Known::Value(expr), None));
    };
    let name = var.name;
    let mut at = at;
    for scope in ancestors.iter().rev() {
        let (params, inherits) = match scope {
            Node::Function(f) => (Some(&f.parameter_list), false),
            Node::Method(m) => (Some(&m.parameter_list), false),
            Node::Closure(c) => (
                Some(&c.parameter_list),
                c.use_clause.as_ref().is_some_and(|u| {
                    u.variables
                        .iter()
                        .any(|v| v.ampersand.is_none() && v.variable.name == name)
                }),
            ),
            Node::ArrowFunction(a) => (Some(&a.parameter_list), true),
            Node::Program(_) => (None, false),
            _ => continue,
        };
        if let Some(param) =
            params.and_then(|p| p.parameters.iter().find(|p| p.variable.name == name))
        {
            let mut writes = Writes::default();
            writes_of(*scope, name, &mut writes);
            return (writes.other.is_none()
                && writes.assignments.is_empty()
                && param.ellipsis.is_none())
            .then_some(param.hint.as_ref().map(|h| (Known::Hint(h), None)))
            .flatten();
        }
        if inherits {
            at = node_span(scope).start;
            continue;
        }
        if matches!(scope, Node::Closure(_) | Node::ArrowFunction(_)) {
            return None;
        }
        let mut writes = Writes::default();
        writes_of(*scope, name, &mut writes);
        if writes.other.is_some() {
            return None;
        }
        let plain = |a: &Assignment<'_>| matches!(a.operator, AssignmentOperator::Assign(_));
        return match writes.assignments.as_slice() {
            [assignment] if node_span(*assignment).end <= at && plain(assignment) => Some((
                Known::Value(unparenthesize(assignment.rhs)),
                Some(node_span(*assignment)),
            )),
            assignments if assignments.iter().all(|a| plain(a) && top_level(scope, a)) => {
                assignments
                    .iter()
                    .rev()
                    .find(|a| node_span(**a).end <= at)
                    .map(|a| (Known::Value(unparenthesize(a.rhs)), Some(node_span(*a))))
            }
            _ => None,
        };
    }
    None
}

fn top_level(scope: &Node<'_, '_>, assignment: &Assignment<'_>) -> bool {
    let statements = match scope {
        Node::Function(f) => f.body.statements.as_slice(),
        Node::Method(Method {
            body: MethodBody::Concrete(b),
            ..
        }) => b.statements.as_slice(),
        Node::Program(p) => p.statements.as_slice(),
        _ => &[],
    };
    let span = node_span(assignment);
    statements.iter().any(|s| {
        matches!(s, Statement::Expression(e) if node_span(unparenthesize(e.expression)) == span)
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Constant {
    Int(i64),
    Float(f64),
    String(Vec<u8>),
    Bool(bool),
    Null,
}

pub(crate) fn constant(
    ancestors: &[Node<'_, '_>],
    expr: &Expression<'_>,
    at: usize,
) -> Option<Constant> {
    let expr = match known(ancestors, expr, at)? {
        Known::Value(v) => v,
        Known::Hint(_) => return None,
    };
    match expr {
        Expression::Literal(Literal::Integer(i)) => Some(Constant::Int(i.value? as i64)),
        Expression::Literal(Literal::Float(f)) => Some(Constant::Float(f.value.into_inner())),
        Expression::Literal(Literal::String(s)) => Some(Constant::String(s.value?.to_vec())),
        Expression::Literal(Literal::True(_)) => Some(Constant::Bool(true)),
        Expression::Literal(Literal::False(_)) => Some(Constant::Bool(false)),
        Expression::Literal(Literal::Null(_)) => Some(Constant::Null),
        Expression::UnaryPrefix(u)
            if matches!(
                u.operator,
                UnaryPrefixOperator::Negation(_) | UnaryPrefixOperator::Plus(_)
            ) =>
        {
            let negate = matches!(u.operator, UnaryPrefixOperator::Negation(_));
            match constant(ancestors, u.operand, at)? {
                Constant::Int(v) if negate => Some(
                    v.checked_neg()
                        .map_or(Constant::Float(-(v as f64)), Constant::Int),
                ),
                Constant::Float(v) if negate => Some(Constant::Float(-v)),
                c @ (Constant::Int(_) | Constant::Float(_)) => Some(c),
                _ => None,
            }
        }
        Expression::UnaryPrefix(u) if matches!(u.operator, UnaryPrefixOperator::BitwiseNot(_)) => {
            match constant(ancestors, u.operand, at)? {
                Constant::Int(v) => Some(Constant::Int(!v)),
                Constant::Float(v) => Some(Constant::Int(!(v as i64))),
                Constant::String(s) => Some(Constant::String(s.iter().map(|b| !b).collect())),
                _ => None,
            }
        }
        Expression::ConstantAccess(c) => {
            let name = String::from_utf8_lossy(c.name.value());
            match name.trim_start_matches('\\') {
                "PHP_INT_MAX" => Some(Constant::Int(i64::MAX)),
                "PHP_INT_MIN" => Some(Constant::Int(i64::MIN)),
                "PHP_INT_SIZE" => Some(Constant::Int(8)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn instantiates(ancestors: &[Node<'_, '_>], new: &Instantiation<'_>, class: &str) -> bool {
    match new.class {
        Expression::Identifier(id) => class_name(ancestors, id).eq_ignore_ascii_case(class),
        _ => false,
    }
}

#[derive(Default)]
struct Writes<'ast, 'arena> {
    assignments: Vec<&'ast Assignment<'arena>>,
    other: Option<usize>,
}

fn writes_of<'ast, 'arena>(node: Node<'ast, 'arena>, name: &[u8], out: &mut Writes<'ast, 'arena>) {
    node.visit_children(|child| match child {
        Node::Function(_)
        | Node::Class(_)
        | Node::Interface(_)
        | Node::Trait(_)
        | Node::Enum(_)
        | Node::AnonymousClass(_)
        | Node::ArrowFunction(_) => {}
        Node::Closure(c) => {
            if let Some(uses) = &c.use_clause
                && uses
                    .variables
                    .iter()
                    .any(|u| u.ampersand.is_some() && u.variable.name == name)
            {
                out.other = Some(node_span(c).start);
            }
        }
        Node::Assignment(a) => {
            match a.lhs {
                Expression::Variable(Variable::Direct(v)) if v.name == name => {
                    out.assignments.push(a);
                }
                lhs if mentions(Node::Expression(lhs), name) => {
                    out.other = Some(node_span(a).start);
                }
                _ => {}
            }
            if let Expression::UnaryPrefix(u) = a.rhs
                && u.operator.is_reference()
                && mentions(Node::Expression(u.operand), name)
            {
                out.other = Some(node_span(a).start);
            }
            writes_of(child, name, out);
        }
        Node::Foreach(f) => {
            let target = match &f.target {
                ForeachTarget::Value(t) => mentions(Node::Expression(t.value), name),
                ForeachTarget::KeyValue(t) => {
                    mentions(Node::Expression(t.key), name)
                        || mentions(Node::Expression(t.value), name)
                }
            };
            if target {
                out.other = Some(node_span(f).start);
            }
            writes_of(child, name, out);
        }
        Node::Global(_) | Node::Static(_) | Node::Unset(_) => {
            if mentions(child, name) {
                out.other = Some(node_span(&child).start);
            }
            writes_of(child, name, out);
        }
        _ => writes_of(child, name, out),
    });
}

fn mentions(node: Node<'_, '_>, name: &[u8]) -> bool {
    if let Node::DirectVariable(v) = node {
        return v.name == name;
    }
    let mut found = false;
    node.visit_children(|child| found = found || mentions(child, name));
    found
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Native {
    String,
    Int,
}

pub(crate) fn native(
    ancestors: &[Node<'_, '_>],
    expr: &Expression<'_>,
    at: usize,
) -> Option<Native> {
    match known(ancestors, expr, at)? {
        Known::Hint(Hint::String(_)) => Some(Native::String),
        Known::Hint(Hint::Integer(_)) => Some(Native::Int),
        Known::Hint(_) => None,
        Known::Value(value) => match value {
            Expression::Literal(Literal::String(_)) | Expression::CompositeString(_) => {
                Some(Native::String)
            }
            Expression::Literal(Literal::Integer(i)) => i
                .value
                .and_then(|v| i64::try_from(v).ok())
                .map(|_| Native::Int),
            Expression::Binary(b) if matches!(b.operator, BinaryOperator::StringConcat(_)) => {
                Some(Native::String)
            }
            Expression::UnaryPrefix(u) => match u.operator {
                UnaryPrefixOperator::StringCast(..) | UnaryPrefixOperator::BinaryCast(..) => {
                    Some(Native::String)
                }
                UnaryPrefixOperator::IntCast(..) | UnaryPrefixOperator::IntegerCast(..) => {
                    Some(Native::Int)
                }
                UnaryPrefixOperator::Negation(_) | UnaryPrefixOperator::Plus(_) => {
                    matches!(constant(ancestors, u.operand, at), Some(Constant::Int(_)))
                        .then_some(Native::Int)
                }
                _ => None,
            },
            Expression::Variable(_) => {
                let at = node_span(value).start;
                native(ancestors, value, at)
            }
            _ => None,
        },
    }
}

const ARRAY_FUNCTIONS: &[&str] = &[
    "array_change_key_case",
    "array_chunk",
    "array_column",
    "array_combine",
    "array_count_values",
    "array_diff",
    "array_diff_assoc",
    "array_diff_key",
    "array_diff_uassoc",
    "array_diff_ukey",
    "array_fill",
    "array_fill_keys",
    "array_filter",
    "array_flip",
    "array_intersect",
    "array_intersect_assoc",
    "array_intersect_key",
    "array_intersect_uassoc",
    "array_intersect_ukey",
    "array_keys",
    "array_map",
    "array_merge",
    "array_merge_recursive",
    "array_pad",
    "array_replace",
    "array_replace_recursive",
    "array_reverse",
    "array_slice",
    "array_splice",
    "array_udiff",
    "array_udiff_assoc",
    "array_udiff_uassoc",
    "array_uintersect",
    "array_uintersect_assoc",
    "array_uintersect_uassoc",
    "array_unique",
    "array_values",
    "bcdivmod",
    "bzerror",
    "cal_from_jd",
    "cal_info",
    "compact",
    "date_parse",
    "date_parse_from_format",
    "date_sun_info",
    "dba_handlers",
    "dba_list",
    "debug_backtrace",
    "explode",
    "filter_list",
    "func_get_args",
    "gc_status",
    "gd_info",
    "get_class_methods",
    "get_class_vars",
    "get_declared_classes",
    "get_declared_interfaces",
    "get_declared_traits",
    "get_defined_constants",
    "get_defined_functions",
    "get_defined_vars",
    "get_html_translation_table",
    "get_included_files",
    "get_loaded_extensions",
    "get_mangled_object_vars",
    "get_object_vars",
    "get_required_files",
    "get_resources",
    "getdate",
    "gmp_div_qr",
    "gmp_gcdext",
    "gmp_rootrem",
    "gmp_sqrtrem",
    "hash_algos",
    "hash_hmac_algos",
    "headers_list",
    "imagecolorsforindex",
    "imagegetclip",
    "imap_rfc822_parse_adrlist",
    "intlcal_get_available_locales",
    "iterator_to_array",
    "ldap_get_attributes",
    "libxml_get_errors",
    "localeconv",
    "localtime",
    "mb_encoding_aliases",
    "mb_list_encodings",
    "mb_str_split",
    "mysqli_error_list",
    "mysqli_fetch_all",
    "mysqli_fetch_fields",
    "mysqli_get_client_stats",
    "mysqli_get_connection_stats",
    "mysqli_get_links_stats",
    "mysqli_stmt_error_list",
    "ob_get_status",
    "ob_list_handlers",
    "openssl_get_cert_locations",
    "openssl_get_cipher_methods",
    "openssl_get_md_methods",
    "password_algos",
    "password_get_info",
    "pdo_drivers",
    "pg_fetch_all",
    "pg_fetch_all_columns",
    "pg_jit",
    "pg_version",
    "proc_get_status",
    "range",
    "readline_list_history",
    "realpath_cache_get",
    "request_parse_body",
    "session_get_cookie_params",
    "socket_addrinfo_explain",
    "socket_get_status",
    "sodium_crypto_kx_client_session_keys",
    "sodium_crypto_kx_server_session_keys",
    "sodium_crypto_secretstream_xchacha20poly1305_init_push",
    "spl_autoload_functions",
    "spl_classes",
    "str_getcsv",
    "str_split",
    "stream_context_get_options",
    "stream_context_get_params",
    "stream_get_filters",
    "stream_get_meta_data",
    "stream_get_transports",
    "stream_get_wrappers",
    "timezone_abbreviations_list",
    "timezone_identifiers_list",
    "token_get_all",
];

pub(crate) fn is_native_array(
    ctx: &Context<'_>,
    ancestors: &[Node<'_, '_>],
    expr: &Expression<'_>,
    at: usize,
) -> bool {
    let Some((known, assignment)) = known_assigned(ancestors, expr, at) else {
        return false;
    };
    if let (Expression::Variable(Variable::Direct(var)), Some(assignment)) =
        (unparenthesize(expr), assignment)
        && var_tagged(ctx, assignment.start, var.name)
    {
        return false;
    }
    match known {
        Known::Hint(Hint::Array(_)) => true,
        Known::Value(value) => match value {
            Expression::Array(_) | Expression::LegacyArray(_) => true,
            Expression::UnaryPrefix(u) => matches!(u.operator, UnaryPrefixOperator::ArrayCast(..)),
            Expression::Variable(_) => {
                is_native_array(ctx, ancestors, value, node_span(value).start)
            }
            Expression::Call(Call::Function(call)) => {
                ARRAY_FUNCTIONS.iter().any(|f| named(call, f))
            }
            _ => false,
        },
        Known::Hint(_) => false,
    }
}

/// PHPStan's `processStmtVarAnnotation()`: a `@var` tag on the statement that assigns `$name`, naming it or no
/// variable, types the variable from the tag and leaves it without an array native type.
fn var_tagged(ctx: &Context<'_>, start: usize, name: &[u8]) -> bool {
    let Some(doc) = doc_comment(ctx.source, previous_token_end(ctx, 0, start), start) else {
        return false;
    };
    let arena = mago_allocator::LocalArena::new();
    let content = &ctx.source.as_bytes()[doc.start..doc.end];
    let document = PHPDocParser::parse(&arena, mago_database::file::FileId::zero(), content);
    document.tags().any(|tag| match tag.value {
        TagValue::Var(value) => {
            tag.name.value == b"var" && value.variable.is_none_or(|v| v.value == name)
        }
        _ => false,
    })
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Constant(String),
}

pub(crate) fn value(expr: &Expression<'_>) -> Option<Value> {
    Some(match unparenthesize(expr) {
        Expression::Literal(Literal::Integer(i)) => Value::Int(i64::try_from(i.value?).ok()?),
        Expression::Literal(Literal::Float(f)) => Value::Float(f.value.0),
        Expression::Literal(Literal::String(s)) => {
            Value::Str(String::from_utf8(s.value?.to_vec()).ok()?)
        }
        Expression::Literal(Literal::True(_)) => Value::Bool(true),
        Expression::Literal(Literal::False(_)) => Value::Bool(false),
        Expression::Literal(Literal::Null(_)) => Value::Null,
        Expression::ConstantAccess(c) => {
            let name = c.name.value();
            let name = name.strip_prefix(b"\\").unwrap_or(name);
            let name = String::from_utf8(name.to_vec()).ok()?;
            match name.to_ascii_lowercase().as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                _ => Value::Constant(name),
            }
        }
        Expression::UnaryPrefix(u) if matches!(u.operator, UnaryPrefixOperator::Negation(_)) => {
            match value(u.operand)? {
                Value::Int(i) => Value::Int(-i),
                Value::Float(f) => Value::Float(-f),
                _ => return None,
            }
        }
        _ => return None,
    })
}

pub(crate) fn int_version(version: &str) -> i64 {
    let version = version.split('-').next().unwrap_or(version);
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        return intval(parts[0]) * 10000 + intval(parts[1]) * 100;
    }
    intval(version)
}

pub(crate) fn native_array(expr: &Expression<'_>, ancestors: &[Node<'_, '_>]) -> bool {
    match unparenthesize(expr) {
        Expression::Array(_) | Expression::LegacyArray(_) => true,
        Expression::Access(Access::Property(fetch)) => {
            let (
                Expression::Variable(Variable::Direct(this)),
                ClassLikeMemberSelector::Identifier(name),
            ) = (fetch.object, &fetch.property)
            else {
                return false;
            };
            this.name == b"$this" && array_property(ancestors, name.value)
        }
        Expression::Variable(Variable::Direct(v)) => {
            let Some((params, body)) = function_like(ancestors) else {
                return false;
            };
            let array_param = |name: &[u8]| {
                params.parameters.iter().any(|p| {
                    p.variable.name == name
                        && matches!(p.hint, Some(Hint::Array(_)))
                        && p.ampersand.is_none()
                        && p.ellipsis.is_none()
                        && !p.default_value.as_ref().is_some_and(|d| {
                            matches!(
                                unparenthesize(d.value),
                                Expression::Literal(Literal::Null(_))
                            )
                        })
                        && variable_writes(body, name, &|_| false) == Some(0)
                })
            };
            // a local assigned an array: a literal, an unwritten `array` parameter or an array function's result
            let array_value = |e: &Expression<'_>| match unparenthesize(e) {
                Expression::Variable(Variable::Direct(other)) => array_param(other.name),
                Expression::Call(Call::Function(call)) => {
                    ARRAY_FUNCTIONS.iter().any(|f| named(call, f))
                }
                _ => false,
            };
            if params.parameters.iter().any(|p| p.variable.name == v.name) {
                return array_param(v.name);
            }
            variable_writes(body, v.name, &array_value).is_some_and(|n| n > 0)
        }
        _ => false,
    }
}

fn array_property(ancestors: &[Node<'_, '_>], name: &[u8]) -> bool {
    let members = ancestors.iter().rev().find_map(|n| match n {
        Node::Class(c) => Some(&c.members),
        Node::AnonymousClass(c) => Some(&c.members),
        _ => None,
    });
    let Some(members) = members else { return false };
    members.iter().any(|m| match m {
        ClassLikeMember::Property(Property::Plain(p)) => {
            matches!(p.hint, Some(Hint::Array(_)))
                && p.items.iter().any(|i| &i.variable().name[1..] == name)
        }
        ClassLikeMember::Method(m) if m.name.value.eq_ignore_ascii_case(b"__construct") => {
            m.parameter_list.parameters.iter().any(|p| {
                !p.modifiers.is_empty()
                    && matches!(p.hint, Some(Hint::Array(_)))
                    && &p.variable.name[1..] == name
            })
        }
        _ => false,
    })
}

fn function_like<'a>(
    ancestors: &[Node<'a, 'a>],
) -> Option<(&'a FunctionLikeParameterList<'a>, Node<'a, 'a>)> {
    ancestors.iter().rev().find_map(|n| match n {
        Node::Method(
            m @ Method {
                body: MethodBody::Concrete(b),
                ..
            },
        ) => Some((&m.parameter_list, Node::Block(b))),
        Node::Function(f) => Some((&f.parameter_list, Node::Block(&f.body))),
        Node::Closure(c) => Some((&c.parameter_list, Node::Block(&c.body))),
        _ => None,
    })
}

fn variable_writes(
    node: Node<'_, '_>,
    name: &[u8],
    array_value: &dyn Fn(&Expression<'_>) -> bool,
) -> Option<usize> {
    let is_var = |e: &Expression<'_>| matches!(unparenthesize(e), Expression::Variable(Variable::Direct(v)) if v.name == name);
    let mut count = 0;
    match node {
        Node::Assignment(a) if is_var(a.lhs) => {
            if !matches!(a.operator, AssignmentOperator::Assign(_))
                || !(matches!(
                    unparenthesize(a.rhs),
                    Expression::Array(_) | Expression::LegacyArray(_)
                ) || array_value(a.rhs))
            {
                return None;
            }
            count += 1;
            return variable_writes(Node::Expression(a.rhs), name, array_value).map(|n| n + count);
        }
        Node::Assignment(a)
            if matches!(
                unparenthesize(a.lhs),
                Expression::Array(_) | Expression::LegacyArray(_) | Expression::List(_)
            ) =>
        {
            if uses_variable(Node::Expression(a.lhs), name) {
                return None;
            }
        }
        Node::Assignment(a)
            if matches!(
                unparenthesize(a.lhs),
                Expression::Variable(Variable::Indirect(_) | Variable::Nested(_))
            ) =>
        {
            return None;
        }
        Node::UnaryPrefix(u) if u.operator.is_reference() && is_var(u.operand) => return None,
        Node::UnaryPrefix(u)
            if matches!(
                u.operator,
                UnaryPrefixOperator::PreIncrement(_) | UnaryPrefixOperator::PreDecrement(_)
            ) && is_var(u.operand) =>
        {
            return None;
        }
        Node::UnaryPostfix(u) if is_var(u.operand) => return None,
        Node::Foreach(f) => {
            let targets: Vec<&Expression> = match &f.target {
                ForeachTarget::Value(t) => vec![t.value],
                ForeachTarget::KeyValue(t) => vec![t.key, t.value],
            };
            if targets
                .iter()
                .any(|t| uses_variable(Node::Expression(t), name))
            {
                return None;
            }
        }
        Node::Global(_) | Node::Static(_) | Node::Unset(_) if uses_variable(node, name) => {
            return None;
        }
        Node::Closure(c) => {
            let captured = c.use_clause.as_ref().is_some_and(|u| {
                u.variables
                    .iter()
                    .any(|v| v.variable.name == name && v.ampersand.is_some())
            });
            return if captured { None } else { Some(0) };
        }
        Node::Function(_)
        | Node::ArrowFunction(_)
        | Node::Class(_)
        | Node::AnonymousClass(_)
        | Node::Interface(_)
        | Node::Trait(_)
        | Node::Enum(_) => return Some(0),
        _ => {}
    }
    let mut failed = false;
    node.visit_children(|child| {
        if failed {
            return;
        }
        match variable_writes(child, name, array_value) {
            Some(n) => count += n,
            None => failed = true,
        }
    });
    (!failed).then_some(count)
}

fn uses_variable(node: Node<'_, '_>, name: &[u8]) -> bool {
    if let Node::DirectVariable(v) = node {
        return v.name == name;
    }
    let mut found = false;
    node.visit_children(|child| found = found || uses_variable(child, name));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_version_is_php_version_factory() {
        assert_eq!(int_version("7.2"), 70200);
        assert_eq!(int_version("8.1.12"), 80100);
        assert_eq!(int_version("8.3-dev"), 80300);
        assert_eq!(int_version("80100"), 80100);
        assert_eq!(int_version("x"), 0);
        assert_eq!(int_version(" -5"), 0);
        assert_eq!(int_version(" 7"), 7);
        assert_eq!(int_version("+7.4"), 70400);
    }
}
