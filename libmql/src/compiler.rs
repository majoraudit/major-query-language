//! MQL 0.2 surface-language compiler.
//!
//! Parses `mql2.pest` source and lowers it into a [`SemanticProgram`], then runs
//! the semantic IR validator. Every diagnostic carries a stable code and, where
//! one exists, the source span of the construct it refers to.

use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
};

use pest::{Parser, Span, error::LineColLocation, iterators::Pair};
use pest_derive::Parser;
use std::{iter::Peekable, vec};

use crate::{
    institution::{FieldOperator, Institution},
    semantic_ir::{
        Bound, BoundSide, CardinalityComparison, Choice, ChoiceOwner, ChoiceValueType,
        ComparisonOperator, Condition, Constraint, Contribution, ContributionKind,
        ContributionUnit, CountUnit, CourseCodeMatch, Effect, EffectKind, FactDeclaration,
        FieldRef, FieldType, GroupMode, Metadata, NullableValue, Obligation, Position, Predicate,
        PredicateValue, RecordTarget, Requirement, SEMANTIC_IR_VERSION, ScalarValue, SchemaRef,
        Scope, ScopeKind, Selector, SemanticProgram, SharingMode, SharingRule, SourceDocument,
        SourceSpan, TotalCount,
    },
};

#[derive(Parser)]
#[grammar = "./mql2.pest"]
struct Mql2Parser;

/// Version of the surface language accepted by [`compile`].
pub const SURFACE_LANGUAGE_VERSION: &str = "0.2";

const SOURCE_ID: &str = "main";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub semantic_id: Option<String>,
    pub span: Option<SourceSpan>,
}

/// One or more diagnostics produced while compiling a source file.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub path: String,
    pub diagnostics: Vec<Diagnostic>,
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} failed to compile with {} error(s)",
            self.path,
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            write!(f, "\n{}", self.path)?;
            if let Some(span) = &diagnostic.span {
                write!(f, ":{}:{}", span.start.line, span.start.column)?;
            }
            write!(f, ": {}", diagnostic.code)?;
            if let Some(id) = &diagnostic.semantic_id {
                write!(f, " [{id}]")?;
            }
            write!(f, ": {}", diagnostic.message)?;
        }
        Ok(())
    }
}

impl Error for CompileError {}

/// Compile MQL 0.2 source into validated semantic IR.
///
/// `path` is recorded as the program's source document. Predicates and group
/// fields require `institution`, which supplies field types and operators.
pub fn compile(
    source: &str,
    path: &str,
    institution: Option<&Institution>,
) -> Result<SemanticProgram, CompileError> {
    let fail = |diagnostics| CompileError {
        path: path.to_owned(),
        diagnostics,
    };
    let file = Mql2Parser::parse(Rule::file, source)
        .map_err(|error| fail(vec![syntax_diagnostic(error)]))?
        .next()
        .expect("file rule always produces one pair");

    let mut compiler = Compiler::new(institution);
    let program = compiler.file(file, path);
    if !compiler.diagnostics.is_empty() {
        return Err(fail(compiler.diagnostics));
    }
    if let Err(errors) = program.validate() {
        let diagnostics = errors
            .into_iter()
            .map(|error| Diagnostic {
                span: error
                    .semantic_id
                    .as_ref()
                    .and_then(|id| compiler.spans.get(id).cloned()),
                code: error.code,
                message: error.message,
                semantic_id: error.semantic_id,
            })
            .collect();
        return Err(fail(diagnostics));
    }
    Ok(program)
}

fn syntax_diagnostic(error: pest::error::Error<Rule>) -> Diagnostic {
    let error = error.renamed_rules(rule_name);
    let ((line, column), (end_line, end_column)) = match error.line_col {
        LineColLocation::Pos(position) => (position, position),
        LineColLocation::Span(start, end) => (start, end),
    };
    let (byte, end_byte) = match error.location {
        pest::error::InputLocation::Pos(byte) => (byte, byte),
        pest::error::InputLocation::Span(span) => span,
    };
    Diagnostic {
        code: "MQL0300_SYNTAX",
        message: error.variant.message().into_owned(),
        semantic_id: None,
        span: Some(SourceSpan {
            source_id: SOURCE_ID.to_owned(),
            start: position(byte, line, column),
            end: position(end_byte, end_line, end_column),
        }),
    }
}

fn rule_name(rule: &Rule) -> String {
    match rule {
        Rule::EOI => "end of file",
        Rule::ident => "identifier",
        Rule::string => "string",
        Rule::integer | Rule::number => "number",
        Rule::description => "description string",
        Rule::header => "`program` header",
        Rule::catalog_year => "catalog year (YYYY-YYYY)",
        Rule::choice_decl
        | Rule::fact_decl
        | Rule::requirement
        | Rule::constraint_decl
        | Rule::share_decl
        | Rule::effect_decl => "declaration",
        Rule::obligation => "obligation",
        Rule::count_stmt | Rule::count_kind => "count statement",
        Rule::bounds => "`exactly`, `at least`, `at most`, or `between`",
        Rule::selector => "`from [...]`, `from requirements [...]`, or `where ...`",
        Rule::course => "course code",
        Rule::condition | Rule::cond_and | Rule::cond_not => "condition",
        Rule::predicate | Rule::pred_and | Rule::pred_not => "predicate",
        Rule::compare_op => "comparison operator",
        Rule::field_ref => "field (course.x, offering.x, attempt.x, student.x)",
        Rule::scope => "scope",
        Rule::constraint_body => "constraint",
        Rule::effect_action => "effect action",
        Rule::scalar | Rule::value => "value",
        Rule::set_unit => "`attempts`, `canonical courses`, `credits`, or `assignments`",
        other => return format!("{other:?}"),
    }
    .to_owned()
}

fn position(byte: usize, line: usize, column: usize) -> Position {
    Position {
        byte: byte as u64,
        line: line as u32,
        column: column as u32,
    }
}

fn source_span(span: Span<'_>) -> SourceSpan {
    let (line, column) = span.start_pos().line_col();
    let (end_line, end_column) = span.end_pos().line_col();
    SourceSpan {
        source_id: SOURCE_ID.to_owned(),
        start: position(span.start(), line, column),
        end: position(span.end(), end_line, end_column),
    }
}

/// A `set lower/upper of X to N` value whose unit depends on X's contribution.
struct PendingBound {
    effect: usize,
    target: String,
    amount: String,
    span: SourceSpan,
}

struct Compiler<'i> {
    institution: Option<&'i Institution>,
    diagnostics: Vec<Diagnostic>,
    spans: HashMap<String, SourceSpan>,
    program_id: String,
    credit_scale: u32,
    choices: Vec<Choice>,
    facts: Vec<FactDeclaration>,
    requirements: Vec<Requirement>,
    constraints: Vec<Constraint>,
    sharing_rules: Vec<SharingRule>,
    effects: Vec<Effect>,
    pending_bounds: Vec<PendingBound>,
}

impl<'i> Compiler<'i> {
    fn new(institution: Option<&'i Institution>) -> Self {
        Self {
            institution,
            diagnostics: Vec::new(),
            spans: HashMap::new(),
            program_id: String::new(),
            credit_scale: 1,
            choices: Vec::new(),
            facts: Vec::new(),
            requirements: Vec::new(),
            constraints: Vec::new(),
            sharing_rules: Vec::new(),
            effects: Vec::new(),
            pending_bounds: Vec::new(),
        }
    }

    fn error(&mut self, code: &'static str, span: Span<'_>, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            code,
            message: message.into(),
            semantic_id: None,
            span: Some(source_span(span)),
        });
    }

    fn file(&mut self, file: Pair<'_, Rule>, path: &str) -> SemanticProgram {
        let mut pairs = file.children();
        let header = pairs.next().expect("file starts with a header");
        let (catalog_year, institution_schema) = self.header(header);

        for item in pairs {
            match item.as_rule() {
                Rule::choice_decl => self.choice(item),
                Rule::fact_decl => self.fact(item),
                Rule::requirement => {
                    let requirement = self.requirement(item, Obligation::Required);
                    self.requirements.push(requirement);
                }
                Rule::constraint_decl => {
                    let constraint = self.constraint(item, None);
                    self.constraints.push(constraint);
                }
                Rule::share_decl => self.share(item),
                Rule::effect_decl => self.effect(item),
                Rule::EOI => {}
                other => unreachable!("unexpected top-level rule {other:?}"),
            }
        }
        self.resolve_pending_bounds();

        SemanticProgram {
            schema_version: SEMANTIC_IR_VERSION.to_owned(),
            program_id: self.program_id.clone(),
            catalog_year,
            institution_schema,
            credit_scale: self.credit_scale,
            sources: vec![SourceDocument {
                source_id: SOURCE_ID.to_owned(),
                path: path.to_owned(),
                digest: None,
            }],
            choices: std::mem::take(&mut self.choices),
            facts: std::mem::take(&mut self.facts),
            requirements: std::mem::take(&mut self.requirements),
            constraints: std::mem::take(&mut self.constraints),
            sharing_rules: std::mem::take(&mut self.sharing_rules),
            effects: std::mem::take(&mut self.effects),
            migration: None,
        }
    }

    // ------------------------------------------------------------ header --

    fn header(&mut self, header: Pair<'_, Rule>) -> (String, SchemaRef) {
        let header_span = header.as_span();
        let mut pairs = header.children();
        self.program_id = pairs.next().unwrap().as_str().to_owned();

        let mut catalog_year = None;
        let mut institution = None;
        let mut scale = None;
        for entry in pairs {
            let span = entry.as_span();
            let duplicate = match entry.as_rule() {
                Rule::catalog_entry => catalog_year
                    .replace(entry.children().next().unwrap().as_str().to_owned())
                    .is_some(),
                Rule::institution_entry => {
                    let mut inner = entry.children();
                    let id = string_value(inner.next().unwrap());
                    let version = string_value(inner.next().unwrap());
                    institution.replace(SchemaRef { id, version }).is_some()
                }
                Rule::scale_entry => {
                    let raw = entry.children().next().unwrap();
                    match raw.as_str().parse::<u32>() {
                        Ok(value) => scale.replace(value).is_some(),
                        Err(_) => {
                            self.error(
                                "MQL0205_CREDIT_SCALE",
                                raw.as_span(),
                                "credit_scale must be a positive integer",
                            );
                            false
                        }
                    }
                }
                other => unreachable!("unexpected header rule {other:?}"),
            };
            if duplicate {
                self.error(
                    "MQL0310_HEADER",
                    span,
                    "header entry is declared more than once",
                );
            }
        }

        if catalog_year.is_none() {
            self.error(
                "MQL0310_HEADER",
                header_span,
                "program header requires `catalog YYYY-YYYY`",
            );
        }
        if institution.is_none() {
            self.error(
                "MQL0310_HEADER",
                header_span,
                "program header requires `institution \"<schema id>\" @ \"<version>\"`",
            );
        }
        if let Some(scale) = scale {
            self.credit_scale = scale;
        }
        let institution = institution.unwrap_or(SchemaRef {
            id: String::new(),
            version: String::new(),
        });
        if let Some(registry) = self.institution
            && !institution.id.is_empty()
            && (registry.schema_id != institution.id || registry.version != institution.version)
        {
            self.error(
                "MQL0307_INSTITUTION_MISMATCH",
                header_span,
                format!(
                    "program targets {} @ {}, but the supplied institution schema is {} @ {}",
                    institution.id, institution.version, registry.schema_id, registry.version
                ),
            );
        }
        (catalog_year.unwrap_or_default(), institution)
    }

    fn meta(
        &mut self,
        id: Pair<'_, Rule>,
        description: Option<String>,
        span: Span<'_>,
    ) -> Metadata {
        let id = id.as_str().to_owned();
        let span = source_span(span);
        self.spans.insert(id.clone(), span.clone());
        Metadata {
            node_id: format!("{}::{id}", self.program_id),
            id,
            description: description.unwrap_or_default(),
            provenance: vec![span.clone()],
            span,
        }
    }

    /// Splits a declaration into its ID, optional description, and the rest.
    fn identity<'a>(
        &mut self,
        pair: Pair<'a, Rule>,
    ) -> (Metadata, Peekable<vec::IntoIter<Pair<'a, Rule>>>) {
        let span = pair.as_span();
        let mut inner = pair.children();
        if inner.peek().map(Pair::as_rule) == Some(Rule::obligation) {
            // The caller reads the obligation from `span` before calling us.
            inner.next();
        }
        if inner.peek().map(Pair::as_rule) == Some(Rule::share_mode) {
            inner.next();
        }
        let id = inner.next().expect("declaration has an identifier");
        let description = inner
            .next_if(|pair| pair.as_rule() == Rule::description)
            .map(|pair| string_value(pair.children().next().unwrap()));
        (self.meta(id, description, span), inner)
    }

    // --------------------------------------------------- choices and facts --

    fn choice(&mut self, pair: Pair<'_, Rule>) {
        let (meta, mut inner) = self.identity(pair);
        let owner_pair = inner.next().unwrap().children().next().unwrap();
        let (owner, fact_id) = match owner_pair.as_rule() {
            Rule::owner_declared => (ChoiceOwner::Declared, None),
            Rule::owner_solver => (ChoiceOwner::Solver, None),
            Rule::owner_planning => (ChoiceOwner::Planning, None),
            Rule::owner_fact => (
                ChoiceOwner::Fact,
                Some(owner_pair.children().next().unwrap().as_str().to_owned()),
            ),
            other => unreachable!("unexpected choice owner {other:?}"),
        };
        let alternatives: Vec<ScalarValue> = inner.map(scalar).collect();
        let value_type = match alternatives.first() {
            Some(ScalarValue::Integer(_)) => ChoiceValueType::Integer,
            Some(ScalarValue::Boolean(_)) => ChoiceValueType::Boolean,
            _ => ChoiceValueType::String,
        };
        self.choices.push(Choice {
            meta,
            owner,
            value_type,
            alternatives,
            fact_id,
        });
    }

    fn fact(&mut self, pair: Pair<'_, Rule>) {
        let (meta, mut inner) = self.identity(pair);
        let approval = inner
            .next_if(|pair| pair.as_rule() == Rule::approval)
            .is_some();
        let fact_type = inner.next().unwrap();
        let mut parameters = serde_json::Map::new();
        let mut parameter_pairs = Vec::new();
        if let Some(params) = inner.next() {
            for param in params.children() {
                let span = param.as_span();
                let mut kv = param.children();
                let key = kv.next().unwrap().as_str().to_owned();
                let value = scalar_json(scalar(kv.next().unwrap()));
                if parameters.insert(key.clone(), value).is_some() {
                    self.error(
                        "MQL0230_DUPLICATE_ENTRY",
                        span,
                        format!("duplicate fact parameter {key}"),
                    );
                }
                parameter_pairs.push((key, span));
            }
        }

        if let Some(institution) = self.institution {
            let span = fact_type.as_span();
            match institution.fact(fact_type.as_str()) {
                None => self.error(
                    "MQL0308_UNKNOWN_FACT_TYPE",
                    span,
                    format!(
                        "institution schema declares no fact type {}",
                        fact_type.as_str()
                    ),
                ),
                Some(declared) => {
                    if declared.approval != approval {
                        let expected = if declared.approval {
                            "is an approval; write `approval` before the fact type"
                        } else {
                            "is not an approval; remove `approval`"
                        };
                        self.error(
                            "MQL0308_UNKNOWN_FACT_TYPE",
                            span,
                            format!("fact type {} {expected}", fact_type.as_str()),
                        );
                    }
                    for (key, key_span) in &parameter_pairs {
                        if !declared.parameters.contains_key(key) {
                            self.error(
                                "MQL0308_UNKNOWN_FACT_TYPE",
                                *key_span,
                                format!("fact type {} has no parameter {key}", fact_type.as_str()),
                            );
                        }
                    }
                }
            }
        }

        self.facts.push(FactDeclaration {
            meta,
            fact_type: fact_type.as_str().to_owned(),
            parameters,
            approval,
        });
    }

    // ------------------------------------------------------- requirements --

    fn requirement(&mut self, pair: Pair<'_, Rule>, inherited: Obligation) -> Requirement {
        let span = pair.as_span();
        let obligation = pair
            .clone()
            .children()
            .next()
            .filter(|first| first.as_rule() == Rule::obligation)
            .map(|first| obligation(first))
            .unwrap_or(inherited);
        let (meta, mut inner) = self.identity(pair);
        let roles = inner
            .next_if(|pair| pair.as_rule() == Rule::roles)
            .map(|roles| id_list(roles.children().next().unwrap()))
            .unwrap_or_default();
        let active_when = inner
            .next_if(|pair| pair.as_rule() == Rule::when_clause)
            .map(|when| self.condition(when.children().next().unwrap()))
            .unwrap_or(Condition::Constant { value: true });

        let mut count = Vec::new();
        let mut children = Vec::new();
        let mut constraint_ids = Vec::new();
        for item in inner {
            match item.as_rule() {
                Rule::count_stmt => count.push(item),
                Rule::requirement => children.push(self.requirement(item, obligation)),
                Rule::constraint_decl => {
                    let default_scope = Scope {
                        kind: ScopeKind::Direct,
                        requirement_id: Some(meta.id.clone()),
                        requirement_ids: Vec::new(),
                        roles: Vec::new(),
                        choice_id: None,
                        branch: None,
                    };
                    let constraint = self.constraint(item, Some(default_scope));
                    constraint_ids.push(constraint.meta().id.clone());
                    self.constraints.push(constraint);
                }
                other => unreachable!("unexpected requirement item {other:?}"),
            }
        }

        let mut extra = count.iter().skip(1);
        if let Some(duplicate) = extra.next() {
            self.error(
                "MQL0301_COUNT_STATEMENT",
                duplicate.as_span(),
                format!("requirement {} has more than one count statement", meta.id),
            );
        }
        let child_ids: Vec<String> = children.iter().map(|child| child.meta.id.clone()).collect();
        let (contribution, lower, upper, selector) = match count.into_iter().next() {
            Some(statement) => self.count_statement(statement, &child_ids),
            None => {
                self.error(
                    "MQL0301_COUNT_STATEMENT",
                    span,
                    format!(
                        "requirement {} needs a count statement such as `courses exactly 1 from [...]`",
                        meta.id
                    ),
                );
                (
                    Contribution {
                        kind: ContributionKind::Slot,
                        unit: ContributionUnit::Slots,
                        fixed_credit_units: None,
                    },
                    Bound::Fixed { value: 0 },
                    None,
                    Selector::ChildRequirements { child_ids },
                )
            }
        };

        Requirement {
            meta,
            roles,
            obligation,
            active_when,
            contribution,
            lower,
            upper,
            selector,
            children,
            constraint_ids,
        }
    }

    fn count_statement(
        &mut self,
        statement: Pair<'_, Rule>,
        child_ids: &[String],
    ) -> (Contribution, Bound, Option<Bound>, Selector) {
        let span = statement.as_span();
        let mut inner = statement.children();
        let kind_pair = inner.next().unwrap().children().next().unwrap();
        let (kind, unit) = match kind_pair.as_rule() {
            Rule::ck_slots => (ContributionKind::Slot, ContributionUnit::Slots),
            Rule::ck_courses => (
                ContributionKind::DistinctAttempts,
                ContributionUnit::Courses,
            ),
            Rule::ck_canonical => (
                ContributionKind::DistinctCanonicalCourses,
                ContributionUnit::Courses,
            ),
            Rule::ck_credits => (ContributionKind::Credits, ContributionUnit::CreditUnits),
            Rule::ck_assignments => (ContributionKind::Assignments, ContributionUnit::Assignments),
            Rule::ck_children => (
                ContributionKind::NestedSatisfaction,
                ContributionUnit::BooleanChildren,
            ),
            other => unreachable!("unexpected count kind {other:?}"),
        };
        let credits = kind == ContributionKind::Credits;

        let (lower, upper) = match inner.next_if(|pair| pair.as_rule() == Rule::bounds) {
            Some(bounds) => self.bounds(bounds, credits),
            None if kind == ContributionKind::Slot => {
                (Bound::Fixed { value: 1 }, Some(Bound::Fixed { value: 1 }))
            }
            None => {
                self.error(
                    "MQL0302_MISSING_BOUND",
                    span,
                    "count statement needs `exactly`, `at least`, `at most`, or `between`",
                );
                (Bound::Fixed { value: 0 }, None)
            }
        };

        let selector = match inner.next_if(|pair| pair.as_rule() == Rule::selector) {
            Some(selector) => self.selector(selector),
            None if !child_ids.is_empty() => Selector::ChildRequirements {
                child_ids: child_ids.to_vec(),
            },
            None => {
                self.error(
                    "MQL0304_MISSING_SELECTOR",
                    span,
                    "count statement needs `from [...]`, `from requirements [...]`, `where ...`, or nested requirements",
                );
                Selector::CourseList {
                    canonical_course_ids: Vec::new(),
                }
            }
        };

        let fixed_credit_units = inner
            .next()
            .map(|each| self.amount_units(each.children().next().unwrap(), true));

        (
            Contribution {
                kind,
                unit,
                fixed_credit_units,
            },
            lower,
            upper,
            selector,
        )
    }

    fn bounds(&mut self, bounds: Pair<'_, Rule>, credits: bool) -> (Bound, Option<Bound>) {
        let bound = bounds.children().next().unwrap();
        let rule = bound.as_rule();
        let mut amounts = bound.children().map(|amount| self.amount(amount, credits));
        let first = amounts.next().unwrap();
        match rule {
            Rule::b_exactly => (first.clone(), Some(first)),
            Rule::b_at_least => (first, None),
            Rule::b_at_most => (Bound::Fixed { value: 0 }, Some(first)),
            Rule::b_between => {
                let second = amounts.next().unwrap();
                (first, Some(second))
            }
            other => unreachable!("unexpected bound {other:?}"),
        }
    }

    fn amount(&mut self, amount: Pair<'_, Rule>, credits: bool) -> Bound {
        let inner = amount.children().next().unwrap();
        match inner.as_rule() {
            Rule::number => Bound::Fixed {
                value: self.amount_units(inner, credits),
            },
            Rule::choice_table => {
                let mut pairs = inner.children();
                let choice_id = pairs.next().unwrap().as_str().to_owned();
                let mut cases = BTreeMap::new();
                for case in pairs {
                    let span = case.as_span();
                    let mut kv = case.children();
                    let key = scalar_key(&scalar(kv.next().unwrap()));
                    let value = self.amount_units(kv.next().unwrap(), credits);
                    if cases.insert(key.clone(), value).is_some() {
                        self.error(
                            "MQL0230_DUPLICATE_ENTRY",
                            span,
                            format!("choice table repeats case {key}"),
                        );
                    }
                }
                Bound::ChoiceTable { choice_id, cases }
            }
            other => unreachable!("unexpected amount {other:?}"),
        }
    }

    /// Converts a written amount to IR units: credits are scaled by
    /// `credit_scale` and must land on a whole unit; other counts are integers.
    fn amount_units(&mut self, number: Pair<'_, Rule>, credits: bool) -> u64 {
        let span = number.as_span();
        let scale = if credits { self.credit_scale } else { 1 };
        match scaled_units(number.as_str(), scale) {
            Some(units) => units,
            None => {
                let message = if credits {
                    format!(
                        "{} credits is not a whole number of units at credit_scale {}",
                        number.as_str(),
                        self.credit_scale
                    )
                } else {
                    format!("{} must be a whole number", number.as_str())
                };
                self.error("MQL0303_AMOUNT", span, message);
                0
            }
        }
    }

    fn selector(&mut self, selector: Pair<'_, Rule>) -> Selector {
        let inner = selector.children().next().unwrap();
        match inner.as_rule() {
            Rule::course_list => Selector::CourseList {
                canonical_course_ids: inner.children().map(course_id).collect(),
            },
            Rule::requirement_list => Selector::ChildRequirements {
                child_ids: id_list(inner.children().next().unwrap()),
            },
            Rule::where_pred => Selector::Predicate {
                predicate: self.predicate(inner.children().next().unwrap()),
            },
            other => unreachable!("unexpected selector {other:?}"),
        }
    }

    // -------------------------------------------------------- constraints --

    fn constraint(&mut self, pair: Pair<'_, Rule>, default_scope: Option<Scope>) -> Constraint {
        let span = pair.as_span();
        let (meta, mut inner) = self.identity(pair);
        let active_when = inner
            .next_if(|pair| pair.as_rule() == Rule::when_clause)
            .map(|when| self.condition(when.children().next().unwrap()))
            .unwrap_or(Condition::Constant { value: true });
        let scope = match inner.next_if(|pair| pair.as_rule() == Rule::scope_clause) {
            Some(scope) => self.scope(scope.children().next().unwrap()),
            None => default_scope.unwrap_or_else(|| {
                self.error(
                    "MQL0309_MISSING_SCOPE",
                    span,
                    format!(
                        "top-level constraint {} needs a scope such as `in program` or `in roles [...]`",
                        meta.id
                    ),
                );
                scope_of(ScopeKind::Program)
            }),
        };

        let body = inner.next().unwrap().children().next().unwrap();
        let rule = body.as_rule();
        let mut parts = body.children();
        match rule {
            Rule::c_selected => {
                let comparison = match parts.next().unwrap().children().next().unwrap().as_rule() {
                    Rule::cmp_at_least => CardinalityComparison::AtLeast,
                    Rule::cmp_at_most => CardinalityComparison::AtMost,
                    Rule::cmp_exactly => CardinalityComparison::Exactly,
                    other => unreachable!("unexpected comparison {other:?}"),
                };
                let number = parts.next().unwrap();
                let unit = set_unit(parts.next().unwrap());
                let amount = self.amount_units(number, unit == CountUnit::CreditUnits);
                let predicate = self.predicate(parts.next().unwrap());
                Constraint::SelectedSet {
                    meta,
                    active_when,
                    scope,
                    comparison,
                    amount,
                    unit,
                    predicate,
                }
            }
            Rule::c_all => Constraint::SelectedSet {
                meta,
                active_when,
                scope,
                comparison: CardinalityComparison::All,
                amount: 0,
                unit: CountUnit::Attempts,
                predicate: self.predicate(parts.next().unwrap()),
            },
            Rule::c_group => {
                let field = self.field_ref(parts.next().unwrap(), None);
                let mode = parts.next().unwrap().children().next().unwrap();
                let (mode, amount) = match mode.as_rule() {
                    Rule::g_same => (GroupMode::Same, 1),
                    rule => {
                        let amount = self.amount_units(mode.children().next().unwrap(), false);
                        let mode = match rule {
                            Rule::g_distinct_at_least => GroupMode::DistinctAtLeast,
                            Rule::g_distinct_exactly => GroupMode::DistinctExactly,
                            Rule::g_at_most_per => GroupMode::MaxPerGroup,
                            Rule::g_at_least_per => GroupMode::MinPerGroup,
                            other => unreachable!("unexpected group mode {other:?}"),
                        };
                        (mode, amount)
                    }
                };
                Constraint::Group {
                    meta,
                    active_when,
                    scope,
                    mode,
                    field,
                    amount,
                }
            }
            Rule::c_limit => {
                let number = parts.next().unwrap();
                let unit = set_unit(parts.next().unwrap());
                let maximum = self.amount_units(number, unit == CountUnit::CreditUnits);
                Constraint::Limit {
                    meta,
                    active_when,
                    scope,
                    maximum,
                    unit,
                    predicate: self.predicate(parts.next().unwrap()),
                }
            }
            Rule::c_total => {
                let number = parts.next().unwrap();
                let unit = set_unit(parts.next().unwrap());
                let minimum = self.amount_units(number, unit == CountUnit::CreditUnits);
                let count = match unit {
                    CountUnit::Attempts => TotalCount::DistinctAttempts,
                    CountUnit::CanonicalCourses => TotalCount::DistinctCanonicalCourses,
                    CountUnit::CreditUnits => TotalCount::CreditUnits,
                    CountUnit::Assignments => TotalCount::Assignments,
                };
                Constraint::Total {
                    meta,
                    active_when,
                    scope,
                    minimum,
                    count,
                    predicate: parts.next().map(|predicate| self.predicate(predicate)),
                }
            }
            Rule::c_repeat => {
                let maximum_attempts = self.amount_units(parts.next().unwrap(), false);
                Constraint::Repeat {
                    meta,
                    active_when,
                    scope,
                    maximum_attempts,
                    predicate: self.predicate(parts.next().unwrap()),
                }
            }
            Rule::c_order => Constraint::Ordering {
                meta,
                active_when,
                scope,
                before: self.predicate(parts.next().unwrap()),
                after: self.predicate(parts.next().unwrap()),
                strict: true,
            },
            other => unreachable!("unexpected constraint body {other:?}"),
        }
    }

    fn scope(&mut self, scope: Pair<'_, Rule>) -> Scope {
        let inner = scope.children().next().unwrap();
        let rule = inner.as_rule();
        let mut parts = inner.children();
        match rule {
            Rule::s_direct | Rule::s_subtree => Scope {
                requirement_id: Some(parts.next().unwrap().as_str().to_owned()),
                ..scope_of(if rule == Rule::s_direct {
                    ScopeKind::Direct
                } else {
                    ScopeKind::Subtree
                })
            },
            Rule::s_requirements => Scope {
                requirement_ids: id_list(parts.next().unwrap()),
                ..scope_of(ScopeKind::Requirements)
            },
            Rule::s_roles => Scope {
                roles: id_list(parts.next().unwrap()),
                ..scope_of(ScopeKind::Roles)
            },
            Rule::s_branch => Scope {
                choice_id: Some(parts.next().unwrap().as_str().to_owned()),
                branch: Some(scalar_key(&scalar(parts.next().unwrap()))),
                ..scope_of(ScopeKind::ActiveBranch)
            },
            Rule::s_program => scope_of(ScopeKind::Program),
            other => unreachable!("unexpected scope {other:?}"),
        }
    }

    // ---------------------------------------------------- sharing/effects --

    fn share(&mut self, pair: Pair<'_, Rule>) {
        let mode = match pair.clone().children().next().unwrap().children().next() {
            Some(mode) if mode.as_rule() == Rule::mode_exclusive => SharingMode::Exclusive,
            _ => SharingMode::Share,
        };
        let (meta, mut inner) = self.identity(pair);
        let active_when = inner
            .next_if(|pair| pair.as_rule() == Rule::when_clause)
            .map(|when| self.condition(when.children().next().unwrap()))
            .unwrap_or(Condition::Constant { value: true });
        self.sharing_rules.push(SharingRule {
            meta,
            active_when,
            mode,
            requirement_ids: inner.map(|id| id.as_str().to_owned()).collect(),
        });
    }

    fn effect(&mut self, pair: Pair<'_, Rule>) {
        let (meta, mut inner) = self.identity(pair);
        let when = self.condition(inner.next().unwrap());
        let action = inner.next().unwrap().children().next().unwrap();
        let rule = action.as_rule();
        let mut parts = action.children();
        let target_id = parts.next().map(|pair| pair.as_str().to_owned());
        let mut effect = Effect {
            meta,
            when,
            kind: EffectKind::Waive,
            target_id: String::new(),
            active: None,
            bound: None,
            value: None,
            replacement_id: None,
            obligation: None,
            predicate: None,
        };
        match rule {
            Rule::e_waive => {
                effect.target_id = target_id.unwrap();
            }
            Rule::e_activate | Rule::e_deactivate => {
                effect.kind = EffectKind::SetActive;
                effect.target_id = target_id.unwrap();
                effect.active = Some(rule == Rule::e_activate);
            }
            Rule::e_set_bound => {
                // `set <side> of <target> to <n>`: the first part is the side.
                let side = target_id.unwrap();
                effect.kind = EffectKind::ReplaceBound;
                effect.bound = Some(if side == "lower" {
                    BoundSide::Lower
                } else {
                    BoundSide::Upper
                });
                effect.target_id = parts.next().unwrap().as_str().to_owned();
                let amount = parts.next().unwrap();
                self.pending_bounds.push(PendingBound {
                    effect: self.effects.len(),
                    target: effect.target_id.clone(),
                    amount: amount.as_str().to_owned(),
                    span: source_span(amount.as_span()),
                });
            }
            Rule::e_substitute => {
                effect.kind = EffectKind::SubstituteNode;
                effect.target_id = target_id.unwrap();
                effect.replacement_id = Some(parts.next().unwrap().as_str().to_owned());
            }
            Rule::e_make => {
                effect.kind = EffectKind::AddObligation;
                effect.target_id = target_id.unwrap();
                effect.obligation = Some(obligation(parts.next().unwrap()));
            }
            Rule::e_permit => {
                effect.kind = EffectKind::PermitAssignment;
                effect.target_id = target_id.unwrap();
                effect.predicate = Some(self.predicate(parts.next().unwrap()));
            }
            other => unreachable!("unexpected effect {other:?}"),
        }
        self.effects.push(effect);
    }

    /// Bound replacements use the target requirement's unit, which is only
    /// known once every requirement has been lowered.
    fn resolve_pending_bounds(&mut self) {
        for pending in std::mem::take(&mut self.pending_bounds) {
            let credits = find_requirement(&self.requirements, &pending.target)
                .is_some_and(|target| target.contribution.kind == ContributionKind::Credits);
            let scale = if credits { self.credit_scale } else { 1 };
            match scaled_units(&pending.amount, scale) {
                Some(value) => self.effects[pending.effect].value = Some(value),
                None => self.diagnostics.push(Diagnostic {
                    code: "MQL0303_AMOUNT",
                    message: format!(
                        "{} is not a whole number of units for {}",
                        pending.amount, pending.target
                    ),
                    semantic_id: Some(self.effects[pending.effect].meta.id.clone()),
                    span: Some(pending.span),
                }),
            }
        }
    }

    // --------------------------------------------------------- conditions --

    fn condition(&mut self, pair: Pair<'_, Rule>) -> Condition {
        match pair.as_rule() {
            Rule::condition | Rule::cond_and => {
                let is_or = pair.as_rule() == Rule::condition;
                let mut operands: Vec<Condition> =
                    pair.children().map(|pair| self.condition(pair)).collect();
                if operands.len() == 1 {
                    operands.pop().unwrap()
                } else if is_or {
                    Condition::Or { operands }
                } else {
                    Condition::And { operands }
                }
            }
            Rule::cond_not => self.condition(pair.children().next().unwrap()),
            Rule::negation => Condition::Not {
                operand: Box::new(self.condition(pair.children().next().unwrap())),
            },
            Rule::boolean_true => Condition::Constant { value: true },
            Rule::boolean_false => Condition::Constant { value: false },
            Rule::cond_fact => Condition::FactPresent {
                fact_id: pair.children().next().unwrap().as_str().to_owned(),
            },
            Rule::cond_choice => {
                let mut parts = pair.children();
                let choice_id = parts.next().unwrap().as_str().to_owned();
                let negated =
                    parts.next().unwrap().children().next().unwrap().as_rule() == Rule::op_ne;
                let equals = Condition::ChoiceEquals {
                    choice_id,
                    equals: scalar(parts.next().unwrap()),
                };
                if negated {
                    Condition::Not {
                        operand: Box::new(equals),
                    }
                } else {
                    equals
                }
            }
            other => unreachable!("unexpected condition {other:?}"),
        }
    }

    // --------------------------------------------------------- predicates --

    fn predicate(&mut self, pair: Pair<'_, Rule>) -> Predicate {
        match pair.as_rule() {
            Rule::predicate | Rule::pred_and => {
                let is_or = pair.as_rule() == Rule::predicate;
                let mut operands: Vec<Predicate> =
                    pair.children().map(|pair| self.predicate(pair)).collect();
                if operands.len() == 1 {
                    operands.pop().unwrap()
                } else if is_or {
                    Predicate::Or { operands }
                } else {
                    Predicate::And { operands }
                }
            }
            Rule::pred_not => self.predicate(pair.children().next().unwrap()),
            Rule::pred_negation => Predicate::Not {
                operand: Box::new(self.predicate(pair.children().next().unwrap())),
            },
            Rule::pred_is => {
                let mut parts = pair.children();
                let field_pair = parts.next().unwrap();
                let test = parts.next().unwrap();
                let (operator, null) = if test.as_rule() == Rule::is_null {
                    (FieldOperator::IsNull, true)
                } else {
                    (FieldOperator::IsUnknown, false)
                };
                let field = self.field_ref(field_pair, Some((operator, test.as_span())));
                if null {
                    Predicate::IsNull { field }
                } else {
                    Predicate::IsUnknown { field }
                }
            }
            Rule::pred_compare => {
                let mut parts = pair.children();
                let field_pair = parts.next().unwrap();
                let operator_pair = parts.next().unwrap();
                let (operator, institution_operator) =
                    compare_operator(operator_pair.clone().children().next().unwrap().as_rule());
                let mut field = self.field_ref(
                    field_pair,
                    Some((institution_operator, operator_pair.as_span())),
                );
                let value_pair = parts.next().unwrap();
                let value_span = value_pair.as_span();
                let value = predicate_value(value_pair);
                self.check_values(&field, &value, value_span);
                if let Some(code_match) = parts.next() {
                    field.course_code_match =
                        Some(match code_match.children().next().unwrap().as_rule() {
                            Rule::match_canonical => CourseCodeMatch::Canonical,
                            Rule::match_enrolled => CourseCodeMatch::EnrolledAs,
                            _ => CourseCodeMatch::Either,
                        });
                }
                Predicate::Compare {
                    field,
                    operator,
                    value,
                }
            }
            other => unreachable!("unexpected predicate {other:?}"),
        }
    }

    /// Resolves `target.field` against the institution schema and, when an
    /// operator is given, checks that the schema allows it for that field.
    fn field_ref(
        &mut self,
        pair: Pair<'_, Rule>,
        operator: Option<(FieldOperator, Span<'_>)>,
    ) -> FieldRef {
        let span = pair.as_span();
        let mut parts = pair.children();
        let target_pair = parts.next().unwrap();
        let target = match target_pair.children().next().unwrap().as_rule() {
            Rule::t_course => RecordTarget::CatalogCourse,
            Rule::t_offering => RecordTarget::Offering,
            Rule::t_attempt => RecordTarget::TranscriptAttempt,
            _ => RecordTarget::StudentProfile,
        };
        let name = parts.next().unwrap().as_str().to_owned();
        let mut field = FieldRef {
            target,
            field: name.clone(),
            field_type: FieldType::String,
            course_code_match: None,
        };

        let Some(institution) = self.institution else {
            self.error(
                "MQL0305_UNKNOWN_FIELD",
                span,
                format!(
                    "{} needs an institution schema to resolve its type (CLI: --institution FILE)",
                    span.as_str()
                ),
            );
            return field;
        };
        let Some(declared) = institution.field(target, &name) else {
            self.error(
                "MQL0305_UNKNOWN_FIELD",
                span,
                format!(
                    "institution schema {} declares no field {}",
                    institution.schema_id,
                    span.as_str()
                ),
            );
            return field;
        };
        field.field_type = declared.field_type;
        if let Some((operator, operator_span)) = operator
            && !declared.operators.contains(&operator)
        {
            self.error(
                "MQL0306_OPERATOR_NOT_ALLOWED",
                operator_span,
                format!(
                    "the institution schema does not allow {operator:?} on {}",
                    span.as_str()
                ),
            );
        }
        field
    }

    /// Checks enum members and grades against the institution schema.
    fn check_values(&mut self, field: &FieldRef, value: &PredicateValue, span: Span<'_>) {
        let Some(institution) = self.institution else {
            return;
        };
        let allowed: Option<&[String]> = match field.field_type {
            FieldType::Grade => Some(&institution.grade_order),
            FieldType::Enum => institution
                .field(field.target, &field.field)
                .and_then(|declared| declared.enum_name.as_ref())
                .and_then(|name| institution.enums.get(name))
                .map(Vec::as_slice),
            _ => None,
        };
        let Some(allowed) = allowed else {
            return;
        };
        let values: Vec<&NullableValue> = match value {
            PredicateValue::Scalar(value) => vec![value],
            PredicateValue::List(values) => values.iter().collect(),
        };
        for value in values {
            if let NullableValue::String(text) = value
                && !allowed.contains(text)
            {
                self.error(
                    "MQL0311_UNKNOWN_VALUE",
                    span,
                    format!(
                        "{text:?} is not a declared value of {}.{} (expected one of {})",
                        target_name(field.target),
                        field.field,
                        allowed.join(", ")
                    ),
                );
            }
        }
    }
}

/// Child pairs without keyword tokens. Keywords are atomic grammar rules (so
/// their word-boundary check sees no skipped whitespace) and carry no data.
trait Children<'a> {
    fn children(self) -> Peekable<vec::IntoIter<Pair<'a, Rule>>>;
}

impl<'a> Children<'a> for Pair<'a, Rule> {
    fn children(self) -> Peekable<vec::IntoIter<Pair<'a, Rule>>> {
        self.into_inner()
            .filter(|pair| !is_keyword(pair.as_rule()))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable()
    }
}

fn is_keyword(rule: Rule) -> bool {
    matches!(
        rule,
        Rule::kw_activate
            | Rule::kw_all
            | Rule::kw_and
            | Rule::kw_at
            | Rule::kw_before
            | Rule::kw_between
            | Rule::kw_branch
            | Rule::kw_canonical
            | Rule::kw_catalog
            | Rule::kw_choice
            | Rule::kw_constraint
            | Rule::kw_credit_scale
            | Rule::kw_credit_word
            | Rule::kw_deactivate
            | Rule::kw_direct
            | Rule::kw_distinct
            | Rule::kw_each
            | Rule::kw_effect
            | Rule::kw_exactly
            | Rule::kw_fact
            | Rule::kw_from
            | Rule::kw_group
            | Rule::kw_in
            | Rule::kw_institution
            | Rule::kw_is
            | Rule::kw_least
            | Rule::kw_limit
            | Rule::kw_make
            | Rule::kw_match
            | Rule::kw_most
            | Rule::kw_not
            | Rule::kw_of
            | Rule::kw_one
            | Rule::kw_or
            | Rule::kw_order
            | Rule::kw_per
            | Rule::kw_permit
            | Rule::kw_program
            | Rule::kw_repeat
            | Rule::kw_requirement
            | Rule::kw_requirements
            | Rule::kw_roles
            | Rule::kw_set
            | Rule::kw_substitute
            | Rule::kw_subtree
            | Rule::kw_times
            | Rule::kw_to
            | Rule::kw_total
            | Rule::kw_waive
            | Rule::kw_when
            | Rule::kw_where
            | Rule::kw_with
    )
}

fn scope_of(kind: ScopeKind) -> Scope {
    Scope {
        kind,
        requirement_id: None,
        requirement_ids: Vec::new(),
        roles: Vec::new(),
        choice_id: None,
        branch: None,
    }
}

fn find_requirement<'a>(requirements: &'a [Requirement], id: &str) -> Option<&'a Requirement> {
    requirements.iter().find_map(|requirement| {
        if requirement.meta.id == id {
            Some(requirement)
        } else {
            find_requirement(&requirement.children, id)
        }
    })
}

fn obligation(pair: Pair<'_, Rule>) -> Obligation {
    match pair.children().next().unwrap().as_rule() {
        Rule::ob_required => Obligation::Required,
        Rule::ob_optional => Obligation::Optional,
        _ => Obligation::Advisory,
    }
}

fn set_unit(pair: Pair<'_, Rule>) -> CountUnit {
    match pair.children().next().unwrap().as_rule() {
        Rule::u_canonical => CountUnit::CanonicalCourses,
        Rule::u_attempts => CountUnit::Attempts,
        Rule::u_credits => CountUnit::CreditUnits,
        _ => CountUnit::Assignments,
    }
}

fn compare_operator(rule: Rule) -> (ComparisonOperator, FieldOperator) {
    match rule {
        Rule::op_eq => (ComparisonOperator::Eq, FieldOperator::Eq),
        Rule::op_ne => (ComparisonOperator::Ne, FieldOperator::Ne),
        Rule::op_lt => (ComparisonOperator::Lt, FieldOperator::Lt),
        Rule::op_le => (ComparisonOperator::Le, FieldOperator::Le),
        Rule::op_gt => (ComparisonOperator::Gt, FieldOperator::Gt),
        Rule::op_ge => (ComparisonOperator::Ge, FieldOperator::Ge),
        Rule::op_in => (ComparisonOperator::In, FieldOperator::In),
        Rule::op_contains => (ComparisonOperator::Contains, FieldOperator::Contains),
        Rule::op_overlaps => (ComparisonOperator::Overlaps, FieldOperator::Overlaps),
        other => unreachable!("unexpected operator {other:?}"),
    }
}

fn target_name(target: RecordTarget) -> &'static str {
    match target {
        RecordTarget::CatalogCourse => "course",
        RecordTarget::Offering => "offering",
        RecordTarget::TranscriptAttempt => "attempt",
        RecordTarget::StudentProfile => "student",
    }
}

fn id_list(pair: Pair<'_, Rule>) -> Vec<String> {
    pair.children().map(|id| id.as_str().to_owned()).collect()
}

fn course_id(course: Pair<'_, Rule>) -> String {
    let inner = course.children().next().unwrap();
    match inner.as_rule() {
        Rule::course_code => inner.children().map(|part| part.as_str()).collect(),
        _ => string_value(inner),
    }
}

/// Unescapes the contents of a `string` pair.
fn string_value(pair: Pair<'_, Rule>) -> String {
    let raw = pair.children().next().map_or("", |inner| inner.as_str());
    let mut value = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            value.extend(chars.next());
        } else {
            value.push(character);
        }
    }
    value
}

fn scalar(pair: Pair<'_, Rule>) -> ScalarValue {
    let inner = pair.children().next().unwrap();
    match inner.as_rule() {
        Rule::string => ScalarValue::String(string_value(inner)),
        Rule::boolean_true => ScalarValue::Boolean(true),
        Rule::boolean_false => ScalarValue::Boolean(false),
        // The grammar admits arbitrarily long digit strings; saturate rather
        // than panic, and let the schema range checks report it.
        _ => ScalarValue::Integer(inner.as_str().parse().unwrap_or(i64::MAX)),
    }
}

fn scalar_key(value: &ScalarValue) -> String {
    match value {
        ScalarValue::String(value) => value.clone(),
        ScalarValue::Integer(value) => value.to_string(),
        ScalarValue::Boolean(value) => value.to_string(),
    }
}

fn scalar_json(value: ScalarValue) -> serde_json::Value {
    match value {
        ScalarValue::String(value) => value.into(),
        ScalarValue::Integer(value) => value.into(),
        ScalarValue::Boolean(value) => value.into(),
    }
}

fn nullable(pair: Pair<'_, Rule>) -> NullableValue {
    if pair.as_rule() == Rule::null {
        return NullableValue::Null;
    }
    match scalar(pair) {
        ScalarValue::String(value) => NullableValue::String(value),
        ScalarValue::Integer(value) => NullableValue::Integer(value),
        ScalarValue::Boolean(value) => NullableValue::Boolean(value),
    }
}

fn predicate_value(pair: Pair<'_, Rule>) -> PredicateValue {
    let inner = pair.children().next().unwrap();
    match inner.as_rule() {
        Rule::value_list => PredicateValue::List(
            inner
                .children()
                .map(|item| nullable(item.children().next().unwrap()))
                .collect(),
        ),
        _ => PredicateValue::Scalar(nullable(inner)),
    }
}

/// Exact decimal-to-unit conversion: `"1.5"` at scale 2 is `Some(3)`;
/// `"1.25"` at scale 2 is `None`.
fn scaled_units(text: &str, scale: u32) -> Option<u64> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = u32::try_from(fraction.len()).ok()?;
    let denominator = 10u128.checked_pow(digits)?;
    let numerator = format!("{whole}{fraction}").parse::<u128>().ok()?;
    let scaled = numerator.checked_mul(u128::from(scale))?;
    if scaled % denominator != 0 {
        return None;
    }
    u64::try_from(scaled / denominator).ok()
}

#[cfg(test)]
mod tests {
    use super::scaled_units;

    #[test]
    fn credit_amounts_scale_exactly() {
        assert_eq!(scaled_units("4", 2), Some(8));
        assert_eq!(scaled_units("1.5", 2), Some(3));
        assert_eq!(scaled_units("0.50", 2), Some(1));
        assert_eq!(scaled_units("1.25", 2), None);
        assert_eq!(scaled_units("3", 1), Some(3));
        assert_eq!(scaled_units("1.5", 1), None);
    }
}
