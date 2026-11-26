use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use mago_allocator::{Arena, LocalArena};
use mago_analyzer::Analyzer;
use mago_analyzer::analysis_result::AnalysisResult;
use mago_analyzer::observations::CallScopeRequest;
pub(super) use mago_analyzer::observations::NamedCallScope;
use mago_analyzer::plugin::context::InvocationInfo;
use mago_analyzer::plugin::{
    ExpressionHook, ExpressionHookResult, HookContext, HookResult, Provider, ProviderMeta,
};
use mago_analyzer::property_initialization::{
    PropertyInitializationCertainty, PropertyInitializationRequest,
};
use mago_analyzer::settings::Settings;
use mago_analyzer::template_arguments::{Resolutions, TemplateArgumentMode};
use mago_codex::metadata::CodebaseMetadata;
use mago_codex::reference::SymbolReferences;
use mago_codex::symbol::SymbolKind;
use mago_codex::ttype::union::TUnion;
use mago_database::file::File;
use mago_names::ResolvedNames;
use mago_span::{HasSpan, Span};
use mago_syntax::cst::*;
use mago_syntax::walker::MutWalker;
use pun_phpstan::types::Tri;

use super::{Checker, Switches, assign, dead_code, variables};

#[path = "type_origins.rs"]
mod origins;
pub(super) use origins::{Origin, Origins};

struct TypedExpressions<'n, 'arena> {
    spans: Vec<Span>,
    names: &'n ResolvedNames<'arena>,
    codebase: &'n CodebaseMetadata,
    level: u8,
    php_version: u32,
}

impl<'ast, 'arena> MutWalker<'ast, 'arena, ()> for TypedExpressions<'_, 'arena> {
    fn walk_in_closure(&mut self, closure: &'ast Closure<'arena>, (): &mut ()) {
        if self.level >= 1 && closure.use_clause.is_some() {
            self.spans.push(closure.span());
        }
    }

    fn walk_in_variable(&mut self, v: &'ast Variable<'arena>, (): &mut ()) {
        if self.level >= 1 {
            self.spans.extend(dead_code::typed_variable(v));
        }
    }
    fn walk_in_function_call(&mut self, call: &'ast FunctionCall<'arena>, (): &mut ()) {
        self.spans.push(call.span());
        self.spans
            .extend(dead_code::typed_arguments(call, self.names, self.level));
    }
    fn walk_in_statement_expression(&mut self, s: &'ast ExpressionStatement<'arena>, (): &mut ()) {
        self.spans
            .extend(dead_code::typed_statement(s, self.level, self.php_version));
    }
    fn walk_in_binary(&mut self, b: &'ast Binary<'arena>, (): &mut ()) {
        if matches!(b.operator, BinaryOperator::Instanceof(_))
            && !matches!(b.lhs, Expression::Variable(Variable::Direct(v)) if v.name == b"$this")
            && let Expression::Identifier(id) = b.rhs
            && self
                .codebase
                .get_class_like(self.names.get(&id.span()))
                .is_some_and(|k| k.kind == SymbolKind::Trait)
        {
            self.spans.push(b.lhs.span());
        }
    }
    fn walk_in_instantiation(&mut self, new: &'ast Instantiation<'arena>, (): &mut ()) {
        if !matches!(
            new.class,
            Expression::Identifier(_)
                | Expression::Static(_)
                | Expression::Self_(_)
                | Expression::Parent(_)
                | Expression::AnonymousClass(_)
        ) {
            self.spans.push(new.class.span());
        }
    }
    fn walk_in_method_call(&mut self, call: &'ast MethodCall<'arena>, (): &mut ()) {
        if self.level >= 2 || !matches!(call.method, ClassLikeMemberSelector::Identifier(_)) {
            self.spans.push(call.span());
        }
        self.spans
            .extend(assign::typed_object(call.object, &call.argument_list).map(HasSpan::span));
    }
    fn walk_in_null_safe_method_call(
        &mut self,
        call: &'ast NullSafeMethodCall<'arena>,
        (): &mut (),
    ) {
        if self.level >= 2 {
            self.spans.push(call.span());
        }
    }
    fn walk_in_assignment(&mut self, a: &'ast Assignment<'arena>, (): &mut ()) {
        if let Some(fetch) = assign::referenced_property(a)
            && !matches!(fetch.object, Expression::Variable(Variable::Direct(v)) if v.name == b"$this")
        {
            self.spans.push(fetch.object.span());
        }
    }
    fn walk_in_static_method_call(&mut self, call: &'ast StaticMethodCall<'arena>, (): &mut ()) {
        self.spans.push(call.span());
        self.spans
            .extend(assign::typed_object(call.class, &call.argument_list).map(HasSpan::span));
    }
}

/// One parsed file and the project settings that every analyser run over it reads.
#[derive(Clone, Copy)]
pub(super) struct Run<'a, 'arena> {
    pub(super) arena: &'arena LocalArena,
    pub(super) file: &'a File,
    pub(super) program: &'arena Program<'arena>,
    pub(super) names: &'a ResolvedNames<'arena>,
    pub(super) codebase: &'a CodebaseMetadata,
    pub(super) php_version: u32,
    pub(super) switches: Switches,
    pub(super) comments: &'a super::Comments,
    pub(super) functions: &'a [(Span, origins::Function)],
}

/// What the checker's own run over a file asks for beyond [`analyse`]'s expression types.
pub(super) struct Request<'a> {
    pub(super) level: u8,
    pub(super) extra: Vec<Span>,
    pub(super) calls: Vec<CallScopeRequest>,
    pub(super) extensions: &'a [&'a dyn crate::AnalyseExtension],
    pub(super) trait_class: Option<&'a str>,
}

/// What an analyser run found, keyed by expression span.
#[derive(Default)]
pub(super) struct Inferred {
    pub(super) types: HashMap<Span, TUnion>,
    pub(super) defined: HashMap<Span, Tri>,
    pub(super) origins: Origins,
    pub(super) call_terminations: HashMap<(u32, u32), bool>,
    pub(super) named_call_scopes: HashMap<Span, Vec<NamedCallScope>>,
    pub(super) keep_void_types: HashMap<Span, TUnion>,
    pub(super) native_types: HashMap<Span, TUnion>,
    pub(super) nullsafe_method_receiver_types: HashMap<Span, TUnion>,
    pub(super) property_offset_writes: HashMap<Span, TUnion>,
    pub(super) variables: variables::Observations,
    pub(super) variable_writes: HashMap<Span, mago_analyzer::variable_flow::VariableWritesNode>,
    pub(super) property_initializations: HashMap<Span, PropertyInitializationCertainty>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variables {
    Observe,
    Skip,
}

struct Observed<'a> {
    wanted: Vec<Span>,
    watched: Vec<Span>,
    calls: Vec<CallScopeRequest>,
    variables: Variables,
    initializations: Vec<(Span, PropertyInitializationRequest)>,
    by_ref: Vec<(Span, Vec<Vec<u8>>)>,
    extensions: &'a [&'a dyn crate::AnalyseExtension],
    trait_class: Option<&'a str>,
    template_arguments: bool,
}

pub(super) fn infer(run: &Run<'_, '_>, request: Request<'_>) -> Inferred {
    let Run {
        program,
        names,
        codebase,
        php_version,
        ..
    } = *run;
    let level = request.level;
    let mut wanted = TypedExpressions {
        spans: Vec::new(),
        names,
        codebase,
        level,
        php_version,
    };
    wanted.walk_program(program, &mut ());
    let mut wanted = wanted.spans;
    wanted.extend(request.extra);
    wanted.extend(super::unset::expressions(program).0);
    wanted.extend(dead_code::flow_types(program, level));
    let (typed, mut watched, initializations) = dead_code::coalesce_types(program, level);
    wanted.extend(typed);
    if level >= 3 {
        watched.extend(super::types::dim_fetches(program));
    }
    let by_ref = if level >= 3 {
        super::types::param_out::watched(program)
    } else {
        Vec::new()
    };
    wanted.extend(by_ref.iter().map(|(body, _)| *body));
    analyse_observed(
        run,
        Observed {
            wanted,
            watched,
            calls: request.calls,
            variables: Variables::Observe,
            initializations,
            by_ref,
            extensions: request.extensions,
            trait_class: request.trait_class,
            template_arguments: run.switches.contains(Switches::TEMPLATE_ARGUMENTS),
        },
    )
}

pub(super) fn analyse(run: &Run<'_, '_>, wanted: Vec<Span>, watched: Vec<Span>) -> Inferred {
    analyse_observed(
        run,
        Observed {
            wanted,
            watched,
            calls: Vec::new(),
            variables: Variables::Skip,
            initializations: Vec::new(),
            by_ref: Vec::new(),
            extensions: &[],
            trait_class: None,
            template_arguments: false,
        },
    )
}

fn observe_template_arguments<'arena>(
    run: &Run<'_, 'arena>,
    program: &'arena Program<'arena>,
    extensions: &[&dyn crate::AnalyseExtension],
    settings: &Settings,
) -> Resolutions {
    let Run {
        arena,
        file,
        names,
        codebase,
        php_version,
        ..
    } = *run;
    let mut registry = super::return_types::registry(&file.contents, names, extensions);
    super::regex_shapes::register(&mut registry, php_version);
    registry.register_function_provider(super::ini_get::IniGet { php_version });
    let mut settings = settings.clone();
    settings.template_arguments = TemplateArgumentMode::Observe;
    settings.call_scope_observations = Vec::new();
    settings.property_initialization_observations = Vec::new();
    settings.track_variable_flow = false;
    settings.track_native_types = false;
    let analyzer = Analyzer::new(arena, file, names, codebase, &registry, settings);
    let mut result = AnalysisResult::new(SymbolReferences::new());
    let observed = crate::mago::contained("analyser's template argument pass", file, || {
        analyzer.analyze_with_artifacts(program, &mut result)
    });
    match observed {
        Some(Ok(artifacts)) => artifacts.template_arguments.solve(codebase),
        _ => Resolutions::default(),
    }
}

fn analyse_observed(run: &Run<'_, '_>, observed: Observed<'_>) -> Inferred {
    let Run {
        arena,
        file,
        program,
        names,
        codebase,
        php_version,
        switches,
        comments,
        functions,
    } = *run;
    let Observed {
        mut wanted,
        watched,
        mut calls,
        variables: observe_variables,
        initializations,
        by_ref,
        extensions,
        trait_class,
        template_arguments,
    } = observed;
    let observe_variables = observe_variables == Variables::Observe;
    wanted.extend(
        calls
            .iter()
            .flat_map(|call| call.expressions.iter().copied()),
    );
    let (compact_calls, offset_names) = if observe_variables {
        let (typed, compact_calls, offset_names) = variables::typed_expressions(program, names);
        wanted.extend(typed);
        (compact_calls, offset_names)
    } else {
        (HashSet::new(), HashSet::new())
    };
    if wanted.is_empty() {
        return Inferred::default();
    }
    let origins = origins::collect(file, comments, functions, program, names, codebase);
    for call in &mut calls {
        call.expressions
            .extend(origins::dependencies(&origins, &call.expressions));
    }
    wanted.extend(origins::dependencies(&origins, &wanted));
    let mut children = ArrayChildren {
        wanted: &wanted,
        spans: Vec::new(),
    };
    children.walk_program(program, &mut ());
    let children = children.spans;
    wanted.extend(children);
    wanted.extend(origins::dependencies(&origins, &wanted));
    let mut registry = super::return_types::registry(&file.contents, names, extensions);
    super::regex_shapes::register(&mut registry, php_version);
    registry.register_function_provider(super::ini_get::IniGet { php_version });
    let found = Arc::new(Mutex::new(Found {
        allowed: if observe_variables {
            variables::allowed_expressions(program)
        } else {
            HashSet::default()
        },
        ..Found::default()
    }));
    if !watched.is_empty() || observe_variables {
        let ids = watched
            .into_iter()
            .map(|s| {
                let text = &file.contents[s.start.offset as usize..s.end.offset as usize];
                (
                    s,
                    text.iter()
                        .filter(|b| !b.is_ascii_whitespace())
                        .copied()
                        .collect(),
                )
            })
            .collect();
        let (_, unset_scopes, unset_roots) = super::unset::expressions(program);
        let mut assignments = Assignments::default();
        assignments.walk_program(program, &mut ());
        registry.register_expression_hook(Definedness {
            assignments: assignments.0,
            ids,
            wanted: wanted.iter().copied().collect(),
            compact_calls,
            offset_names,
            builtins: super::reflection::builtins(php_version),
            unset_scopes,
            unset_roots,
            found: Arc::clone(&found),
        });
    }
    let param_out = (!by_ref.is_empty()).then(|| super::types::param_out::Observer {
        bodies: Arc::new(by_ref),
        found: Arc::default(),
    });
    if let Some(o) = &param_out {
        registry.register_statement_hook(o.clone());
        registry.register_expression_hook(o.clone());
    }
    let mut settings = Settings::new(crate::codebase::php_version(php_version));
    settings.pollute_scope_with_loop_initial_assignments =
        switches.contains(Switches::POLLUTE_SCOPE_WITH_LOOP_INITIAL_ASSIGNMENTS);
    settings.pollute_scope_with_always_iterable_foreach =
        switches.contains(Switches::POLLUTE_SCOPE_WITH_ALWAYS_ITERABLE_FOREACH);
    settings.pollute_scope_with_block = switches.contains(Switches::POLLUTE_SCOPE_WITH_BLOCK);
    settings.track_native_types = true;
    settings.track_variable_flow = observe_variables;
    settings.call_scope_observations = calls;
    settings.trait_context_class = trait_class.map(|class| mago_word::word(class.as_bytes()));
    settings.mixin_receiver_binding = false;
    for extension in extensions {
        extension.settings(&mut settings);
    }
    let retain_methods = !initializations.is_empty();
    settings.property_initialization_observations = initializations
        .iter()
        .map(|(_, request)| *request)
        .collect();
    let program = arena.alloc(Program {
        statements: Prune {
            arena,
            wanted: &wanted,
            retain_methods,
        }
        .statements(&program.statements),
        ..program.clone()
    });
    if template_arguments {
        settings.template_arguments = TemplateArgumentMode::Resolve(Arc::new(
            observe_template_arguments(run, program, extensions, &settings),
        ));
    }
    let analyzer = Analyzer::new(arena, file, names, codebase, &registry, settings);
    let mut result = AnalysisResult::new(SymbolReferences::new());
    let analysed = crate::mago::contained("analyser", file, || {
        analyzer.analyze_with_artifacts(program, &mut result)
    });
    let Some(Ok(artifacts)) = analysed else {
        return Inferred::default();
    };
    let mut keep_void_types: HashMap<Span, TUnion> = wanted
        .iter()
        .filter_map(|span| {
            Some((
                *span,
                artifacts.get_keep_void_expression_type(span)?.clone(),
            ))
        })
        .collect();
    let mut native_types: HashMap<Span, TUnion> = wanted
        .iter()
        .filter_map(|span| Some((*span, artifacts.get_native_expression_type(span)?.clone())))
        .collect();
    let mut types: HashMap<Span, TUnion> = wanted
        .into_iter()
        .filter_map(|span| Some((span, artifacts.get_expression_type(&span)?.clone())))
        .collect();
    let mut found =
        std::mem::take(&mut *found.lock().expect("no hook panics while holding the lock"));
    types.extend(found.read);
    types.extend(artifacts.late_expression_types.clone());
    keep_void_types.extend(found.keep_void_types);
    for (span, ty) in found.native_read {
        if let Some(ty) = ty {
            native_types.insert(span, ty);
        } else {
            native_types.remove(&span);
        }
    }
    let mut inferred = Inferred {
        call_terminations: artifacts.call_terminations.into_iter().collect(),
        named_call_scopes: artifacts.call_scope_observations.into_iter().collect(),
        nullsafe_method_receiver_types: artifacts
            .nullsafe_method_receiver_types
            .into_iter()
            .collect(),
        property_offset_writes: artifacts.property_offset_writes.into_iter().collect(),
        ..Inferred::default()
    };
    if observe_variables {
        inferred.variables = std::mem::take(&mut found.variables);
        inferred.variables.allowed = std::mem::take(&mut found.allowed);
        if let Some(o) = param_out {
            inferred.variables.param_out = std::mem::take(
                &mut *o
                    .found
                    .lock()
                    .expect("no hook panics while holding the lock"),
            );
        }
        inferred.variable_writes = artifacts.variable_writes.into_iter().collect();
        inferred.property_initializations = initializations
            .into_iter()
            .filter_map(|(operand, request)| {
                Some((
                    operand,
                    *artifacts
                        .property_initialization_observations
                        .get(&request.expression)?
                        .get(&request.property)?,
                ))
            })
            .collect();
    }
    Inferred {
        types,
        defined: found.defined,
        origins,
        keep_void_types,
        native_types,
        ..inferred
    }
}

#[derive(Default)]
struct Assignments(HashMap<Span, (Span, Vec<u8>)>);

impl<'ast, 'arena> MutWalker<'ast, 'arena, ()> for Assignments {
    fn walk_in_assignment(&mut self, a: &'ast Assignment<'arena>, (): &mut ()) {
        if !a.operator.is_assign() {
            return;
        }
        let mut root = a.lhs;
        while let Expression::ArrayAccess(access) = root {
            root = access.array;
        }
        if let Expression::Variable(Variable::Direct(v)) = root {
            self.0.insert(a.rhs.span(), (a.lhs.span(), v.name.to_vec()));
        }
    }
}

struct ArrayChildren<'s> {
    wanted: &'s [Span],
    spans: Vec<Span>,
}

impl<'ast, 'arena> MutWalker<'ast, 'arena, ()> for ArrayChildren<'_> {
    fn walk_in_expression(&mut self, e: &'ast Expression<'arena>, (): &mut ()) {
        let elements = match e {
            Expression::Array(a) => &a.elements,
            Expression::LegacyArray(a) => &a.elements,
            _ => return,
        };
        let span = e.span();
        if !self
            .wanted
            .iter()
            .any(|w| w.start.offset <= span.start.offset && w.end.offset >= span.end.offset)
        {
            return;
        }
        self.spans.push(span);
        for element in elements {
            if let ArrayElement::KeyValue(kv) = element {
                self.spans.push(kv.key.span());
            }
            self.spans.extend(element.get_value().map(HasSpan::span));
        }
    }
}

struct Definedness {
    assignments: HashMap<Span, (Span, Vec<u8>)>,
    ids: HashMap<Span, Vec<u8>>,
    wanted: HashSet<Span>,
    compact_calls: HashSet<Span>,
    offset_names: HashSet<variables::OffsetName>,
    builtins: &'static pun_phpstan::symbols::Symbols,
    unset_scopes: HashSet<Span>,
    unset_roots: HashMap<Span, Vec<(Span, Vec<u8>)>>,
    found: Arc<Mutex<Found>>,
}

#[derive(Default)]
struct Found {
    defined: HashMap<Span, Tri>,
    read: HashMap<Span, TUnion>,
    keep_void_types: HashMap<Span, TUnion>,
    native_read: HashMap<Span, Option<TUnion>>,
    variables: variables::Observations,
    allowed: HashSet<Span>,
    arrows: HashMap<Span, variables::Certainty>,
    reference_sources: HashMap<Span, variables::ReferenceSource>,
    possible_defined: HashSet<Span>,
    possible_reads: HashSet<Span>,
    possible_read_types: HashSet<Span>,
}

fn merge(
    entry: &mut Tri,
    now: Tri,
    span: Span,
    impossible: bool,
    possible: &mut HashSet<Span>,
    inserted: bool,
) {
    if impossible {
        if !inserted && !possible.contains(&span) && *entry != now {
            *entry = Tri::Maybe;
        }
    } else if possible.insert(span) && !inserted {
        *entry = now;
    } else if *entry != now {
        *entry = Tri::Maybe;
    }
}

impl Provider for Definedness {
    fn meta() -> &'static ProviderMeta {
        static META: ProviderMeta =
            ProviderMeta::new("pun-definedness", "definedness", "variable certainty");
        &META
    }
}

impl ExpressionHook for Definedness {
    fn is_observer(&self) -> bool {
        true
    }

    fn before_arguments(
        &self,
        invocation: &InvocationInfo<'_, '_, '_>,
        context: &mut HookContext<'_, '_>,
    ) -> HookResult<()> {
        let mut found = self
            .found
            .lock()
            .expect("no hook panics while holding the lock");
        let mut non_creating = HashSet::new();
        variables::allow_arguments(
            invocation,
            self.builtins,
            &mut found.allowed,
            &mut non_creating,
        );
        if !non_creating.is_empty() {
            context.mark_non_creating_references(non_creating);
        }
        Ok(())
    }

    fn before_expression(
        &self,
        expr: &Expression<'_>,
        context: &mut HookContext<'_, '_>,
    ) -> HookResult<ExpressionHookResult> {
        let span = expr.span();
        {
            let mut found = self
                .found
                .lock()
                .expect("no hook panics while holding the lock");
            if let Some(roots) = self.unset_roots.get(&span) {
                for (at, name) in roots {
                    let ty = context
                        .get_variable_type(name)
                        .map_or_else(mago_codex::ttype::get_error, |ty| (**ty).clone());
                    found.variables.unset_entries.insert(*at, ty);
                    found
                        .variables
                        .scopes
                        .insert(*at, variables::Certainty::of(context));
                }
            }
            if self.unset_scopes.contains(&span) {
                found
                    .variables
                    .scopes
                    .entry(span)
                    .or_insert_with(|| variables::Certainty::of(context));
                super::unset::observe(expr, context, &mut found.read);
            }
            match expr {
                Expression::Construct(Construct::Isset(i)) => {
                    for value in &i.values {
                        super::unset::observe(value, context, &mut found.read);
                    }
                }
                Expression::Construct(Construct::Empty(e)) => {
                    super::unset::observe(e.value, context, &mut found.read);
                }
                Expression::Array(a) => {
                    literal_reference_reads(a.elements.as_slice(), &mut found.allowed);
                }
                Expression::LegacyArray(a) => {
                    literal_reference_reads(a.elements.as_slice(), &mut found.allowed);
                }
                Expression::Variable(v) => observe_variable(v, context, &mut found),
                Expression::ArrayAccess(a) => {
                    if let Expression::Variable(Variable::Direct(v)) = a.array
                        && let Some(ty) = context.get_variable_type(v.name)
                    {
                        found
                            .variables
                            .dim_receivers
                            .insert(v.span(), (**ty).clone());
                    }
                }
                Expression::Access(Access::StaticProperty(p)) => {
                    if let Variable::Nested(v) = &p.property {
                        observe_variable(v.variable, context, &mut found);
                    }
                }
                Expression::Call(Call::Function(_)) if self.compact_calls.contains(&span) => {
                    found
                        .variables
                        .compact_scopes
                        .insert(span, variables::Certainty::of(context));
                }
                Expression::Closure(c) => {
                    if let Some(uses) = &c.use_clause {
                        for capture in &uses.variables {
                            if capture.ampersand.is_none() {
                                observe_direct(&capture.variable, context, &mut found);
                            }
                        }
                    }
                }
                Expression::ArrowFunction(f) => {
                    found
                        .arrows
                        .insert(f.span(), variables::Certainty::of(context));
                }
                Expression::Assignment(a) => {
                    if let Some(source) = variables::ReferenceSource::of(a, context) {
                        found.reference_sources.insert(span, source);
                    }
                }
                _ => {}
            }
        }
        if let Some(id) = self.ids.get(&span) {
            let block = context.block_context();
            let name = mago_word::word(id);
            let variable = matches!(expr, Expression::Variable(_));
            let local = block.locals.get(&name);
            let now = if variable {
                let certainty = block.variable_certainty(name);
                if certainty.is_defined() {
                    Tri::Yes
                } else if certainty.is_absent() {
                    Tri::No
                } else {
                    Tri::Maybe
                }
            } else {
                match local {
                    Some(t) if t.possibly_undefined() || t.possibly_undefined_from_try() => {
                        Tri::Maybe
                    }
                    Some(_) => Tri::Yes,
                    None => Tri::No,
                }
            };
            let mut found = self
                .found
                .lock()
                .expect("no hook panics while holding the lock");
            if let Some(t) = local.filter(|_| variable) {
                found.read.insert(span, (**t).clone());
            }
            let impossible = block.variable_state.impossible;
            let found = &mut *found;
            let inserted = !found.defined.contains_key(&span);
            let entry = found.defined.entry(span).or_insert(now);
            merge(
                entry,
                now,
                span,
                impossible,
                &mut found.possible_defined,
                inserted,
            );
        }
        Ok(ExpressionHookResult::Continue)
    }

    fn after_expression(
        &self,
        expr: &Expression<'_>,
        context: &mut HookContext<'_, '_>,
    ) -> HookResult<()> {
        if let Some((target, name)) = self.assignments.get(&expr.span()) {
            let mut found = self
                .found
                .lock()
                .expect("no hook panics while holding the lock");
            if context
                .block_context()
                .variable_certainty(mago_word::word(name))
                .is_defined()
                && let Some(ty) = context.get_variable_type(name)
            {
                let native = context.get_native_variable_type(name).cloned();
                found
                    .variables
                    .assigned_roots
                    .insert(*target, ((**ty).clone(), native));
            } else {
                found.variables.assigned_roots.remove(target);
            }
        }
        if let Expression::Assignment(assignment) = expr {
            for name in &self.offset_names {
                name.assigned(assignment, context);
            }
        }
        if self.wanted.contains(&expr.span())
            && !self.unset_scopes.contains(&expr.span())
            && !(self.ids.contains_key(&expr.span()) && matches!(expr, Expression::Variable(_)))
            && let Some(ty) = context.get_expression_type(expr)
        {
            let impossible = context.block_context().variable_state.impossible;
            let mut found = self
                .found
                .lock()
                .expect("no hook panics while holding the lock");
            let found = &mut *found;
            let span = expr.span();
            let answered = found.possible_read_types.contains(&span);
            match found.read.entry(span) {
                std::collections::hash_map::Entry::Occupied(_) if impossible && answered => {}
                std::collections::hash_map::Entry::Occupied(mut previous)
                    if impossible || answered =>
                {
                    let mut combined = mago_codex::ttype::combine_union_types(
                        previous.get(),
                        ty,
                        context.codebase(),
                        Default::default(),
                    );
                    let nullsafe = |t: &TUnion| !t.has_null() || t.has_nullsafe_null();
                    if combined.has_null() && nullsafe(previous.get()) && nullsafe(ty) {
                        combined.set_nullsafe_null(true);
                    }
                    previous.insert(combined);
                }
                std::collections::hash_map::Entry::Occupied(mut previous) => {
                    previous.insert(ty.clone());
                }
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(ty.clone());
                }
            }
            if !impossible {
                found.possible_read_types.insert(span);
            }
        }
        if self.wanted.contains(&expr.span())
            && let Some(ty) = context.get_keep_void_expression_type(expr)
        {
            self.found
                .lock()
                .expect("no hook panics while holding the lock")
                .keep_void_types
                .insert(expr.span(), ty.clone());
        }
        if self.wanted.contains(&expr.span()) {
            self.found
                .lock()
                .expect("no hook panics while holding the lock")
                .native_read
                .insert(
                    expr.span(),
                    context.get_native_expression_type(expr).cloned(),
                );
        }
        {
            if let Some(source) = self
                .found
                .lock()
                .expect("no hook panics while holding the lock")
                .reference_sources
                .remove(&expr.span())
            {
                source.restore(context);
            }
            if let Expression::Variable(Variable::Indirect(i)) = expr {
                let mut found = self
                    .found
                    .lock()
                    .expect("no hook panics while holding the lock");
                if !found.allowed.contains(&expr.span()) {
                    found.variables.scopes.insert(
                        expr.span(),
                        variables::Certainty::for_name(i.expression, context),
                    );
                }
            }
        }
        Ok(())
    }
}

fn literal_reference_reads(elements: &[ArrayElement<'_>], allowed: &mut HashSet<Span>) {
    for element in elements {
        let value = match element {
            ArrayElement::KeyValue(item) => item.value,
            ArrayElement::Value(item) => item.value,
            _ => continue,
        };
        if let Expression::UnaryPrefix(reference) = value
            && reference.operator.is_reference()
        {
            let mut targets = HashSet::new();
            variables::allow_target(reference.operand, &mut targets);
            allowed.retain(|span| !targets.contains(span));
        }
    }
}

fn observe_variable(v: &Variable<'_>, context: &HookContext<'_, '_>, found: &mut Found) {
    match v {
        Variable::Direct(v) => observe_direct(v, context, found),
        Variable::Nested(v) => {
            if !found.allowed.contains(&v.span()) {
                found.variables.scopes.insert(
                    v.span(),
                    variables::Certainty::for_name(
                        &Expression::Variable(v.variable.clone()),
                        context,
                    ),
                );
            }
            if let Variable::Direct(name) = v.variable
                && let Some(ty) = context.get_variable_type(name.name)
            {
                found.read.insert(name.span(), (**ty).clone());
            }
            observe_variable(v.variable, context, found);
        }
        Variable::Indirect(_) => {}
    }
}

fn observe_direct(v: &DirectVariable<'_>, context: &HookContext<'_, '_>, found: &mut Found) {
    let name = String::from_utf8_lossy(v.name);
    if found.allowed.contains(&v.span()) {
        if v.name != b"$this" && variables::defined(context, &name).no() {
            found.variables.undefined_allowed.insert(v.span());
        }
        return;
    }
    let certainty = context
        .scope()
        .get_function_like()
        .filter(|_| v.name != b"$this" && context.get_variable_type(v.name).is_none())
        .and_then(|f| found.arrows.get(&f.span))
        .map_or_else(
            || variables::defined(context, &name),
            |parent| parent.captured(&name[1..]),
        );
    let inserted = !found.variables.reads.contains_key(&v.span());
    let entry = found.variables.reads.entry(v.span()).or_insert_with(|| {
        (
            name[1..].to_string(),
            certainty,
            variables::is_main(context),
        )
    });
    let impossible = context.block_context().variable_state.impossible;
    merge(
        &mut entry.1,
        certainty,
        v.span(),
        impossible,
        &mut found.possible_reads,
        inserted,
    );
}

struct Prune<'w, 'arena> {
    arena: &'arena LocalArena,
    wanted: &'w [Span],
    retain_methods: bool,
}

impl<'arena> Prune<'_, 'arena> {
    fn holds(&self, span: Span) -> bool {
        self.wanted
            .iter()
            .any(|w| span.start.offset <= w.start.offset && w.end.offset <= span.end.offset)
    }

    fn statements(
        &self,
        statements: &Sequence<'arena, Statement<'arena>>,
    ) -> Sequence<'arena, Statement<'arena>> {
        let kept = statements.iter().filter_map(|s| match s {
            Statement::Function(_)
            | Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Trait(_)
            | Statement::Enum(_)
                if !self.holds(s.span()) =>
            {
                None
            }
            Statement::Namespace(n) => Some(Statement::Namespace(Namespace {
                body: match &n.body {
                    NamespaceBody::Implicit(b) => NamespaceBody::Implicit(NamespaceImplicitBody {
                        statements: self.statements(&b.statements),
                        ..b.clone()
                    }),
                    NamespaceBody::BraceDelimited(b) => NamespaceBody::BraceDelimited(Block {
                        statements: self.statements(&b.statements),
                        ..b.clone()
                    }),
                },
                ..n.clone()
            })),
            Statement::Class(c) => Some(Statement::Class(Class {
                members: self.members(&c.members),
                ..c.clone()
            })),
            Statement::Trait(t) => Some(Statement::Trait(Trait {
                members: self.members(&t.members),
                ..t.clone()
            })),
            Statement::Enum(e) => Some(Statement::Enum(Enum {
                members: self.members(&e.members),
                ..e.clone()
            })),
            s => Some(s.clone()),
        });
        Sequence::from_slice(self.arena.alloc_slice_fill_iter(kept))
    }

    fn members(
        &self,
        members: &Sequence<'arena, ClassLikeMember<'arena>>,
    ) -> Sequence<'arena, ClassLikeMember<'arena>> {
        let kept = members
            .iter()
            .filter(|m| {
                !matches!(m, ClassLikeMember::Method(_))
                    || self.retain_methods
                    || self.holds(m.span())
            })
            .cloned();
        Sequence::from_slice(self.arena.alloc_slice_fill_iter(kept))
    }
}

impl Checker<'_, '_> {
    pub(super) fn type_of(&self, expr: &Expression) -> Option<&TUnion> {
        self.types.get(&expr.span())
    }

    /// A `$this` that a `@var` tag gave another class than the class's own `$this`, on every path or on some
    /// (var_tags::typed_expressions asks for its type only after such a tag).
    pub(super) fn this_retyped(&self, this: &Expression) -> bool {
        use mago_codex::ttype::atomic::TAtomic;
        use mago_codex::ttype::atomic::object::TObject;
        let named = |a: &TAtomic| match a {
            TAtomic::Object(TObject::Named(n)) => {
                Some(!n.is_this && self.codebase.class_like_exists(n.name.as_bytes()))
            }
            _ => None,
        };
        self.this_tagged()
            && self.type_of(this).is_some_and(|t| {
                t.types
                    .iter()
                    .all(|a| a.is_null() || named(a).is_some_and(|known| known || a.is_this()))
                    && t.types.iter().any(|a| named(a) == Some(true))
            })
    }

    pub(super) fn keep_void_type_at(&self, span: Span) -> Option<&TUnion> {
        let value = self.types.get(&span)?;
        if value
            .types
            .iter()
            .any(mago_codex::ttype::atomic::TAtomic::is_null)
        {
            self.keep_void_types.get(&span).or(Some(value))
        } else {
            Some(value)
        }
    }

    pub(super) fn call_terminates(&self, expr: &Expression) -> bool {
        let span = expr.span();
        self.call_terminations
            .get(&(span.start.offset, span.end.offset))
            .copied()
            .unwrap_or(false)
    }
}
