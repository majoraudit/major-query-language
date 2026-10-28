//! Solver-independent semantic representation introduced by MQL 0.2.
//!
//! This module intentionally does not alter the legacy 0.1 surface parser. It
//! provides the versioned wire types and structural/semantic validation that a
//! future grammar, audit engine, or SDK can share.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
};

use serde::{Deserialize, Serialize};

pub const SEMANTIC_IR_VERSION: &str = "0.2.0";
const LEGACY_LANGUAGE_VERSION: &str = "0.1.12";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProgram {
    pub schema_version: String,
    pub program_id: String,
    pub catalog_year: String,
    pub institution_schema: SchemaRef,
    pub credit_scale: u32,
    pub sources: Vec<SourceDocument>,
    pub choices: Vec<Choice>,
    pub facts: Vec<FactDeclaration>,
    pub requirements: Vec<Requirement>,
    pub constraints: Vec<Constraint>,
    pub sharing_rules: Vec<SharingRule>,
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration: Option<Migration>,
}

impl SemanticProgram {
    /// Deserialize and semantically validate an MQL 0.2 IR document.
    pub fn from_json(input: &str) -> Result<Self, SemanticIrError> {
        let program: Self = serde_json::from_str(input).map_err(SemanticIrError::Json)?;
        program.validate().map_err(SemanticIrError::Validation)?;
        Ok(program)
    }

    /// Validate identities, references, activation metadata, scopes, and
    /// effect conflicts that JSON Schema alone cannot express.
    pub fn validate(&self) -> Result<(), Vec<ValidationError>> {
        Validator::new(self).validate()
    }
}

#[derive(Debug)]
pub enum SemanticIrError {
    Json(serde_json::Error),
    Validation(Vec<ValidationError>),
}

impl fmt::Display for SemanticIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid semantic IR JSON: {error}"),
            Self::Validation(errors) => {
                write!(
                    f,
                    "semantic IR validation failed with {} error(s)",
                    errors.len()
                )?;
                for error in errors {
                    write!(f, "\n- {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl Error for SemanticIrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Validation(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub code: &'static str,
    pub semantic_id: Option<String>,
    pub message: String,
}

impl ValidationError {
    fn new(
        code: &'static str,
        semantic_id: impl Into<Option<String>>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            semantic_id: semantic_id.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.semantic_id {
            Some(id) => write!(f, "{} [{id}]: {}", self.code, self.message),
            None => write!(f, "{}: {}", self.code, self.message),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaRef {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceDocument {
    pub source_id: String,
    pub path: String,
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub byte: u64,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    pub source_id: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub id: String,
    pub node_id: String,
    pub description: String,
    pub span: SourceSpan,
    pub provenance: Vec<SourceSpan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Condition {
    Constant {
        value: bool,
    },
    ChoiceEquals {
        choice_id: String,
        equals: ScalarValue,
    },
    FactPresent {
        fact_id: String,
    },
    And {
        operands: Vec<Condition>,
    },
    Or {
        operands: Vec<Condition>,
    },
    Not {
        operand: Box<Condition>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ScalarValue {
    String(String),
    Integer(i64),
    Boolean(bool),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Bound {
    Fixed {
        value: u64,
    },
    ChoiceTable {
        choice_id: String,
        cases: BTreeMap<String, u64>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Obligation {
    Required,
    Optional,
    Advisory,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContributionKind {
    Slot,
    DistinctAttempts,
    DistinctCanonicalCourses,
    Credits,
    Assignments,
    NestedSatisfaction,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContributionUnit {
    Slots,
    Courses,
    CreditUnits,
    Assignments,
    BooleanChildren,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    pub kind: ContributionKind,
    pub unit: ContributionUnit,
    #[serde(default)]
    pub fixed_credit_units: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub meta: Metadata,
    pub roles: Vec<String>,
    pub obligation: Obligation,
    pub active_when: Condition,
    pub contribution: Contribution,
    pub lower: Bound,
    /// Present on the wire; `null` means unbounded.
    #[serde(deserialize_with = "Option::deserialize")]
    pub upper: Option<Bound>,
    pub selector: Selector,
    pub children: Vec<Requirement>,
    pub constraint_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Selector {
    Predicate { predicate: Predicate },
    CourseList { canonical_course_ids: Vec<String> },
    ChildRequirements { child_ids: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Predicate {
    Compare {
        field: FieldRef,
        operator: ComparisonOperator,
        value: PredicateValue,
    },
    IsNull {
        field: FieldRef,
    },
    IsUnknown {
        field: FieldRef,
    },
    And {
        operands: Vec<Predicate>,
    },
    Or {
        operands: Vec<Predicate>,
    },
    Not {
        operand: Box<Predicate>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PredicateValue {
    Scalar(NullableValue),
    List(Vec<NullableValue>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum NullableValue {
    String(String),
    Integer(i64),
    Boolean(bool),
    Null,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldRef {
    pub target: RecordTarget,
    pub field: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    #[serde(default)]
    pub course_code_match: Option<CourseCodeMatch>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecordTarget {
    CatalogCourse,
    Offering,
    TranscriptAttempt,
    StudentProfile,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FieldType {
    Boolean,
    Integer,
    Credit,
    String,
    Enum,
    Grade,
    CourseCode,
    CanonicalCourseId,
    Term,
    Set,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CourseCodeMatch {
    Canonical,
    EnrolledAs,
    Either,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Contains,
    Overlaps,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScopeKind {
    Direct,
    Subtree,
    Requirements,
    Roles,
    ActiveBranch,
    Program,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub kind: ScopeKind,
    #[serde(default)]
    pub requirement_id: Option<String>,
    #[serde(default)]
    pub requirement_ids: Vec<String>,
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub choice_id: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Constraint {
    SelectedSet {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        comparison: CardinalityComparison,
        amount: u64,
        unit: CountUnit,
        predicate: Predicate,
    },
    Group {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        mode: GroupMode,
        field: FieldRef,
        amount: u64,
    },
    Limit {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        maximum: u64,
        unit: CountUnit,
        predicate: Predicate,
    },
    Total {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        minimum: u64,
        count: TotalCount,
        #[serde(default)]
        predicate: Option<Predicate>,
    },
    Repeat {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        maximum_attempts: u64,
        predicate: Predicate,
    },
    Ordering {
        meta: Metadata,
        active_when: Condition,
        scope: Scope,
        before: Predicate,
        after: Predicate,
        strict: bool,
    },
}

impl Constraint {
    pub fn meta(&self) -> &Metadata {
        match self {
            Self::SelectedSet { meta, .. }
            | Self::Group { meta, .. }
            | Self::Limit { meta, .. }
            | Self::Total { meta, .. }
            | Self::Repeat { meta, .. }
            | Self::Ordering { meta, .. } => meta,
        }
    }
    pub fn active_when(&self) -> &Condition {
        match self {
            Self::SelectedSet { active_when, .. }
            | Self::Group { active_when, .. }
            | Self::Limit { active_when, .. }
            | Self::Total { active_when, .. }
            | Self::Repeat { active_when, .. }
            | Self::Ordering { active_when, .. } => active_when,
        }
    }
    pub fn scope(&self) -> &Scope {
        match self {
            Self::SelectedSet { scope, .. }
            | Self::Group { scope, .. }
            | Self::Limit { scope, .. }
            | Self::Total { scope, .. }
            | Self::Repeat { scope, .. }
            | Self::Ordering { scope, .. } => scope,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CardinalityComparison {
    AtLeast,
    AtMost,
    Exactly,
    All,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CountUnit {
    Attempts,
    CanonicalCourses,
    CreditUnits,
    Assignments,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TotalCount {
    DistinctAttempts,
    DistinctCanonicalCourses,
    CreditUnits,
    Assignments,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupMode {
    DistinctAtLeast,
    DistinctExactly,
    Same,
    MaxPerGroup,
    MinPerGroup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SharingRule {
    pub meta: Metadata,
    pub active_when: Condition,
    pub mode: SharingMode,
    pub requirement_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SharingMode {
    Exclusive,
    Share,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    pub meta: Metadata,
    pub owner: ChoiceOwner,
    pub value_type: ChoiceValueType,
    pub alternatives: Vec<ScalarValue>,
    #[serde(default)]
    pub fact_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChoiceOwner {
    Declared,
    Solver,
    Fact,
    Planning,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChoiceValueType {
    String,
    Integer,
    Boolean,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactDeclaration {
    pub meta: Metadata,
    pub fact_type: String,
    pub parameters: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub meta: Metadata,
    pub when: Condition,
    pub kind: EffectKind,
    pub target_id: String,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub bound: Option<BoundSide>,
    #[serde(default)]
    pub value: Option<u64>,
    #[serde(default)]
    pub replacement_id: Option<String>,
    #[serde(default)]
    pub obligation: Option<Obligation>,
    #[serde(default)]
    pub predicate: Option<Predicate>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EffectKind {
    SetActive,
    ReplaceBound,
    Waive,
    SubstituteNode,
    AddObligation,
    PermitAssignment,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BoundSide {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migration {
    pub from_version: String,
    pub operations: Vec<serde_json::Value>,
}

struct Validator<'a> {
    program: &'a SemanticProgram,
    errors: Vec<ValidationError>,
    ids: HashSet<String>,
    node_ids: HashSet<String>,
    sources: HashSet<String>,
    requirement_obligations: HashMap<String, Obligation>,
    requirement_ids: HashSet<String>,
    constraint_ids: HashSet<String>,
    choice_ids: HashSet<String>,
    fact_ids: HashSet<String>,
}

impl<'a> Validator<'a> {
    fn new(program: &'a SemanticProgram) -> Self {
        Self {
            program,
            errors: Vec::new(),
            ids: HashSet::new(),
            node_ids: HashSet::new(),
            sources: HashSet::new(),
            requirement_obligations: HashMap::new(),
            requirement_ids: HashSet::new(),
            constraint_ids: HashSet::new(),
            choice_ids: HashSet::new(),
            fact_ids: HashSet::new(),
        }
    }

    fn validate(mut self) -> Result<(), Vec<ValidationError>> {
        if self.program.schema_version != SEMANTIC_IR_VERSION {
            self.error(
                "MQL0200_SCHEMA_VERSION",
                None,
                format!(
                    "expected schema version {SEMANTIC_IR_VERSION}, found {}",
                    self.program.schema_version
                ),
            );
        }
        if self.program.credit_scale == 0 {
            self.error(
                "MQL0205_CREDIT_SCALE",
                None,
                "credit_scale must be positive",
            );
        }
        if !is_catalog_year(&self.program.catalog_year) {
            self.error(
                "MQL0229_CATALOG_YEAR",
                None,
                format!(
                    "catalog_year must be YYYY-YYYY, found {}",
                    self.program.catalog_year
                ),
            );
        }
        if self.program.institution_schema.version.is_empty() {
            self.error(
                "MQL0228_EMPTY_VALUE",
                None,
                "institution_schema.version cannot be empty",
            );
        }
        if self.program.sources.is_empty() {
            self.error("MQL0228_EMPTY_VALUE", None, "sources cannot be empty");
        }
        if self.program.requirements.is_empty() {
            self.error("MQL0228_EMPTY_VALUE", None, "requirements cannot be empty");
        }
        if let Some(migration) = &self.program.migration
            && migration.from_version != LEGACY_LANGUAGE_VERSION
        {
            self.error(
                "MQL0200_SCHEMA_VERSION",
                None,
                format!(
                    "migration.from_version must be {LEGACY_LANGUAGE_VERSION}, found {}",
                    migration.from_version
                ),
            );
        }
        if !is_qualified_id(&self.program.program_id) {
            self.error(
                "MQL0206_INVALID_ID",
                Some(self.program.program_id.clone()),
                "invalid program ID",
            );
        }
        for source in &self.program.sources {
            if !is_author_id(&source.source_id) {
                self.error(
                    "MQL0206_INVALID_ID",
                    Some(source.source_id.clone()),
                    "invalid source ID",
                );
            }
            if source.path.is_empty() {
                self.error(
                    "MQL0228_EMPTY_VALUE",
                    Some(source.source_id.clone()),
                    "source path cannot be empty",
                );
            }
            if !self.sources.insert(source.source_id.clone()) {
                self.error(
                    "MQL0207_DUPLICATE_ID",
                    Some(source.source_id.clone()),
                    "duplicate source ID",
                );
            }
        }

        for choice in &self.program.choices {
            self.register_meta(&choice.meta);
            self.choice_ids.insert(choice.meta.id.clone());
        }
        for fact in &self.program.facts {
            self.register_meta(&fact.meta);
            self.fact_ids.insert(fact.meta.id.clone());
            if fact.fact_type.is_empty() {
                self.error(
                    "MQL0228_EMPTY_VALUE",
                    Some(fact.meta.id.clone()),
                    "fact_type cannot be empty",
                );
            }
        }
        for requirement in &self.program.requirements {
            self.register_requirement(requirement);
        }
        for constraint in &self.program.constraints {
            self.register_meta(constraint.meta());
            self.constraint_ids.insert(constraint.meta().id.clone());
        }
        for sharing in &self.program.sharing_rules {
            self.register_meta(&sharing.meta);
        }
        for effect in &self.program.effects {
            self.register_meta(&effect.meta);
        }

        for choice in &self.program.choices {
            if choice.alternatives.len() < 2 {
                self.error(
                    "MQL0210_CHOICE_CARDINALITY",
                    Some(choice.meta.id.clone()),
                    "a choice needs at least two alternatives",
                );
            }
            self.require_unique(
                choice.alternatives.iter().map(|alternative| {
                    serde_json::to_string(alternative).expect("scalar is serializable")
                }),
                &choice.meta.id,
                "alternatives",
            );
            for alternative in &choice.alternatives {
                if !scalar_matches_type(alternative, choice.value_type) {
                    self.error(
                        "MQL0223_CHOICE_TYPE",
                        Some(choice.meta.id.clone()),
                        "choice alternative does not match value_type",
                    );
                }
            }
            if matches!(choice.owner, ChoiceOwner::Fact) {
                match &choice.fact_id {
                    Some(id) => self.require_fact(id, &choice.meta.id),
                    None => self.error(
                        "MQL0211_FACT_CHOICE",
                        Some(choice.meta.id.clone()),
                        "FACT-owned choice requires fact_id",
                    ),
                }
            }
        }
        for requirement in &self.program.requirements {
            self.validate_requirement(requirement);
        }
        for constraint in &self.program.constraints {
            self.validate_condition(constraint.active_when(), &constraint.meta().id);
            self.validate_scope(constraint.scope(), &constraint.meta().id);
            self.validate_constraint_payload(constraint);
        }
        for sharing in &self.program.sharing_rules {
            self.validate_condition(&sharing.active_when, &sharing.meta.id);
            if sharing.requirement_ids.len() < 2 {
                self.error(
                    "MQL0212_SHARING_CARDINALITY",
                    Some(sharing.meta.id.clone()),
                    "sharing rule needs at least two requirements",
                );
            }
            self.require_unique(
                sharing.requirement_ids.iter().cloned(),
                &sharing.meta.id,
                "requirement_ids",
            );
            for id in &sharing.requirement_ids {
                self.require_requirement(id, &sharing.meta.id);
                if self.requirement_obligations.get(id) == Some(&Obligation::Advisory) {
                    self.error(
                        "MQL0204_ADVISORY_SCOPE",
                        Some(sharing.meta.id.clone()),
                        format!("sharing references advisory requirement {id}"),
                    );
                }
            }
        }
        for effect in &self.program.effects {
            self.validate_effect(effect);
        }
        self.validate_effect_conflicts();

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn register_requirement(&mut self, requirement: &Requirement) {
        self.register_meta(&requirement.meta);
        self.requirement_ids.insert(requirement.meta.id.clone());
        self.requirement_obligations
            .insert(requirement.meta.id.clone(), requirement.obligation);
        for child in &requirement.children {
            self.register_requirement(child);
        }
    }

    fn register_meta(&mut self, meta: &Metadata) {
        if !is_qualified_id(&meta.id) {
            self.error(
                "MQL0206_INVALID_ID",
                Some(meta.id.clone()),
                "invalid author ID",
            );
        }
        if !self.ids.insert(meta.id.clone()) {
            self.error(
                "MQL0207_DUPLICATE_ID",
                Some(meta.id.clone()),
                "duplicate semantic ID",
            );
        }
        if meta.node_id.is_empty() {
            self.error(
                "MQL0206_INVALID_ID",
                Some(meta.id.clone()),
                "node_id cannot be empty",
            );
        } else if !self.node_ids.insert(meta.node_id.clone()) {
            self.error(
                "MQL0207_DUPLICATE_ID",
                Some(meta.id.clone()),
                format!("duplicate node ID {}", meta.node_id),
            );
        }
        let expected_node_id = format!("{}::{}", self.program.program_id, meta.id);
        if meta.node_id != expected_node_id {
            self.error(
                "MQL0226_NODE_ID_MISMATCH",
                Some(meta.id.clone()),
                format!(
                    "expected generated node ID {expected_node_id}, found {}",
                    meta.node_id
                ),
            );
        }
        self.validate_span(&meta.span, &meta.id);
        if meta.provenance.is_empty() {
            self.error(
                "MQL0213_PROVENANCE",
                Some(meta.id.clone()),
                "provenance cannot be empty",
            );
        }
        for span in &meta.provenance {
            self.validate_span(span, &meta.id);
        }
    }

    fn validate_span(&mut self, span: &SourceSpan, owner: &str) {
        if !self.sources.contains(&span.source_id) {
            self.error(
                "MQL0201_UNRESOLVED_ID",
                Some(owner.to_owned()),
                format!("unknown source {}", span.source_id),
            );
        }
        if span.start.byte > span.end.byte
            || span.start.line == 0
            || span.end.line == 0
            || span.start.column == 0
            || span.end.column == 0
        {
            self.error(
                "MQL0214_INVALID_SPAN",
                Some(owner.to_owned()),
                "invalid source span ordering or position",
            );
        }
    }

    fn validate_requirement(&mut self, requirement: &Requirement) {
        let owner = &requirement.meta.id;
        self.validate_author_ids(&requirement.roles, owner, "role");
        self.require_unique(requirement.roles.iter().cloned(), owner, "roles");
        self.require_unique(
            requirement.constraint_ids.iter().cloned(),
            owner,
            "constraint_ids",
        );
        self.validate_condition(&requirement.active_when, owner);
        if !contribution_unit_matches(requirement.contribution.kind, requirement.contribution.unit)
        {
            self.error(
                "MQL0224_INCOMPATIBLE_UNIT",
                Some(owner.clone()),
                "contribution kind and unit are incompatible",
            );
        }
        self.validate_bound(&requirement.lower, owner);
        if let Some(upper) = &requirement.upper {
            self.validate_bound(upper, owner);
        }
        if let (Bound::Fixed { value: lower }, Some(Bound::Fixed { value: upper })) =
            (&requirement.lower, &requirement.upper)
            && lower > upper
        {
            self.error(
                "MQL0215_INVALID_BOUNDS",
                Some(owner.clone()),
                format!("lower bound {lower} exceeds upper bound {upper}"),
            );
        }
        match &requirement.selector {
            Selector::CourseList {
                canonical_course_ids,
            } => {
                if canonical_course_ids.is_empty() {
                    self.error(
                        "MQL0216_EMPTY_SELECTOR",
                        Some(owner.clone()),
                        "course list cannot be empty",
                    );
                }
                if canonical_course_ids.iter().any(String::is_empty) {
                    self.error(
                        "MQL0228_EMPTY_VALUE",
                        Some(owner.clone()),
                        "canonical course ID cannot be empty",
                    );
                }
                self.require_unique(
                    canonical_course_ids.iter().cloned(),
                    owner,
                    "canonical_course_ids",
                );
            }
            Selector::ChildRequirements { child_ids } => {
                self.require_unique(child_ids.iter().cloned(), owner, "child_ids");
                for id in child_ids {
                    self.require_requirement(id, owner);
                }
            }
            Selector::Predicate { predicate } => self.validate_predicate(predicate, owner),
        }
        for id in &requirement.constraint_ids {
            if !self.constraint_ids.contains(id) {
                self.error(
                    "MQL0201_UNRESOLVED_ID",
                    Some(owner.clone()),
                    format!("unknown constraint {id}"),
                );
            }
        }
        for child in &requirement.children {
            self.validate_requirement(child);
        }
    }

    fn validate_bound(&mut self, bound: &Bound, owner: &str) {
        if let Bound::ChoiceTable { choice_id, cases } = bound {
            self.require_choice(choice_id, owner);
            if cases.is_empty() {
                self.error(
                    "MQL0217_EMPTY_CHOICE_TABLE",
                    Some(owner.to_owned()),
                    "choice bound has no cases",
                );
            }
            if let Some(choice) = self
                .program
                .choices
                .iter()
                .find(|choice| choice.meta.id == *choice_id)
            {
                for alternative in &choice.alternatives {
                    let key = scalar_key(alternative);
                    if !cases.contains_key(&key) {
                        self.error(
                            "MQL0218_INCOMPLETE_CHOICE_TABLE",
                            Some(owner.to_owned()),
                            format!("choice bound lacks case {key}"),
                        );
                    }
                }
            }
        }
    }

    fn validate_condition(&mut self, condition: &Condition, owner: &str) {
        match condition {
            Condition::Constant { .. } => {}
            Condition::ChoiceEquals { choice_id, equals } => {
                self.require_choice(choice_id, owner);
                if let Some(choice) = self
                    .program
                    .choices
                    .iter()
                    .find(|choice| choice.meta.id == *choice_id)
                    && !choice.alternatives.contains(equals)
                {
                    self.error(
                        "MQL0219_UNKNOWN_CHOICE_VALUE",
                        Some(owner.to_owned()),
                        format!(
                            "{} is not an alternative of {choice_id}",
                            scalar_key(equals)
                        ),
                    );
                }
            }
            Condition::FactPresent { fact_id } => self.require_fact(fact_id, owner),
            Condition::And { operands } | Condition::Or { operands } => {
                if operands.is_empty() {
                    self.error(
                        "MQL0220_EMPTY_BOOLEAN",
                        Some(owner.to_owned()),
                        "boolean condition needs operands",
                    );
                }
                for operand in operands {
                    self.validate_condition(operand, owner);
                }
            }
            Condition::Not { operand } => self.validate_condition(operand, owner),
        }
    }

    fn validate_predicate(&mut self, predicate: &Predicate, owner: &str) {
        match predicate {
            Predicate::Compare {
                field, operator, ..
            } => {
                self.validate_field_ref(field, owner);
                let ordered = matches!(
                    field.field_type,
                    FieldType::Integer | FieldType::Credit | FieldType::Grade | FieldType::Term
                );
                if matches!(
                    operator,
                    ComparisonOperator::Lt
                        | ComparisonOperator::Le
                        | ComparisonOperator::Gt
                        | ComparisonOperator::Ge
                ) && !ordered
                {
                    self.error(
                        "MQL0225_INVALID_OPERATOR",
                        Some(owner.to_owned()),
                        format!("ordering operator is invalid for {:?}", field.field_type),
                    );
                }
                if matches!(
                    operator,
                    ComparisonOperator::Contains | ComparisonOperator::Overlaps
                ) && field.field_type != FieldType::Set
                {
                    self.error(
                        "MQL0225_INVALID_OPERATOR",
                        Some(owner.to_owned()),
                        "set operator requires a SET field",
                    );
                }
                if field.course_code_match.is_some() && field.field_type != FieldType::CourseCode {
                    self.error(
                        "MQL0225_INVALID_OPERATOR",
                        Some(owner.to_owned()),
                        "course_code_match is valid only for COURSE_CODE fields",
                    );
                }
            }
            Predicate::IsNull { field } | Predicate::IsUnknown { field } => {
                self.validate_field_ref(field, owner);
            }
            Predicate::And { operands } | Predicate::Or { operands } => {
                if operands.is_empty() {
                    self.error(
                        "MQL0220_EMPTY_BOOLEAN",
                        Some(owner.to_owned()),
                        "boolean predicate needs operands",
                    );
                }
                for operand in operands {
                    self.validate_predicate(operand, owner);
                }
            }
            Predicate::Not { operand } => self.validate_predicate(operand, owner),
        }
    }

    fn validate_scope(&mut self, scope: &Scope, owner: &str) {
        self.validate_author_ids(&scope.roles, owner, "role");
        self.require_unique(scope.roles.iter().cloned(), owner, "scope roles");
        self.require_unique(
            scope.requirement_ids.iter().cloned(),
            owner,
            "scope requirement_ids",
        );
        let mut scoped_requirements = Vec::new();
        match scope.kind {
            ScopeKind::Direct | ScopeKind::Subtree => match &scope.requirement_id {
                Some(id) => scoped_requirements.push(id.as_str()),
                None => self.error(
                    "MQL0221_INVALID_SCOPE",
                    Some(owner.to_owned()),
                    "DIRECT/SUBTREE scope requires requirement_id",
                ),
            },
            ScopeKind::Requirements => {
                if scope.requirement_ids.is_empty() {
                    self.error(
                        "MQL0221_INVALID_SCOPE",
                        Some(owner.to_owned()),
                        "REQUIREMENTS scope cannot be empty",
                    );
                }
                scoped_requirements.extend(scope.requirement_ids.iter().map(String::as_str));
            }
            ScopeKind::Roles if scope.roles.is_empty() => self.error(
                "MQL0221_INVALID_SCOPE",
                Some(owner.to_owned()),
                "ROLES scope cannot be empty",
            ),
            ScopeKind::ActiveBranch => {
                match &scope.choice_id {
                    Some(id) => self.require_choice(id, owner),
                    None => self.error(
                        "MQL0221_INVALID_SCOPE",
                        Some(owner.to_owned()),
                        "ACTIVE_BRANCH scope requires choice_id",
                    ),
                }
                if scope.branch.is_none() {
                    self.error(
                        "MQL0221_INVALID_SCOPE",
                        Some(owner.to_owned()),
                        "ACTIVE_BRANCH scope requires branch",
                    );
                }
            }
            _ => {}
        }
        for id in scoped_requirements {
            self.require_requirement(id, owner);
            if self.requirement_obligations.get(id) == Some(&Obligation::Advisory) {
                self.error(
                    "MQL0204_ADVISORY_SCOPE",
                    Some(owner.to_owned()),
                    format!("scope references advisory requirement {id}"),
                );
            }
        }
    }

    fn validate_constraint_payload(&mut self, constraint: &Constraint) {
        let owner = &constraint.meta().id;
        match constraint {
            Constraint::SelectedSet { predicate, .. }
            | Constraint::Limit { predicate, .. }
            | Constraint::Repeat { predicate, .. } => self.validate_predicate(predicate, owner),
            Constraint::Total { predicate, .. } => {
                if let Some(predicate) = predicate {
                    self.validate_predicate(predicate, owner);
                }
            }
            Constraint::Ordering {
                before,
                after,
                strict,
                ..
            } => {
                self.validate_predicate(before, owner);
                self.validate_predicate(after, owner);
                if !strict {
                    self.error(
                        "MQL0227_NON_STRICT_ORDERING",
                        Some(owner.clone()),
                        "MQL 0.2 BEFORE ordering must be strict",
                    );
                }
            }
            Constraint::Group { field, .. } => self.validate_field_ref(field, owner),
        }
    }

    fn validate_effect(&mut self, effect: &Effect) {
        let owner = &effect.meta.id;
        self.validate_condition(&effect.when, owner);
        self.require_requirement(&effect.target_id, owner);
        let valid = match effect.kind {
            EffectKind::SetActive => effect.active.is_some(),
            EffectKind::ReplaceBound => effect.bound.is_some() && effect.value.is_some(),
            EffectKind::Waive => true,
            EffectKind::SubstituteNode => effect
                .replacement_id
                .as_ref()
                .is_some_and(|id| self.requirement_ids.contains(id)),
            EffectKind::AddObligation => effect.obligation.is_some(),
            EffectKind::PermitAssignment => effect.predicate.is_some(),
        };
        if !valid {
            self.error(
                "MQL0222_INVALID_EFFECT",
                Some(owner.clone()),
                "effect payload does not match its kind",
            );
        }
        if let Some(predicate) = &effect.predicate {
            self.validate_predicate(predicate, owner);
        }
    }

    fn validate_effect_conflicts(&mut self) {
        let mut seen: HashMap<(String, EffectKind, Option<BoundSide>), (&Condition, String)> =
            HashMap::new();
        for effect in &self.program.effects {
            let key = (effect.target_id.clone(), effect.kind, effect.bound);
            let payload = serde_json::to_string(&(
                effect.active,
                effect.bound,
                effect.value,
                &effect.replacement_id,
                effect.obligation,
                &effect.predicate,
            ))
            .expect("effect payload is serializable");
            if let Some((condition, previous)) = seen.get(&key) {
                if *condition == &effect.when && previous != &payload {
                    self.error(
                        "MQL0203_CONFLICTING_EFFECTS",
                        Some(effect.meta.id.clone()),
                        format!("conflicts with another effect on {}", effect.target_id),
                    );
                }
            } else {
                seen.insert(key, (&effect.when, payload));
            }
        }
    }

    fn validate_field_ref(&mut self, field: &FieldRef, owner: &str) {
        if !is_author_id(&field.field) {
            self.error(
                "MQL0206_INVALID_ID",
                Some(owner.to_owned()),
                format!("invalid field name {}", field.field),
            );
        }
    }

    fn validate_author_ids(&mut self, ids: &[String], owner: &str, what: &str) {
        for id in ids {
            if !is_author_id(id) {
                self.error(
                    "MQL0206_INVALID_ID",
                    Some(owner.to_owned()),
                    format!("invalid {what} {id}"),
                );
            }
        }
    }

    fn require_unique(
        &mut self,
        values: impl IntoIterator<Item = String>,
        owner: &str,
        what: &str,
    ) {
        let mut seen = HashSet::new();
        for value in values {
            if !seen.insert(value.clone()) {
                self.error(
                    "MQL0230_DUPLICATE_ENTRY",
                    Some(owner.to_owned()),
                    format!("duplicate entry {value} in {what}"),
                );
            }
        }
    }

    fn require_requirement(&mut self, id: &str, owner: &str) {
        if !self.requirement_ids.contains(id) {
            self.error(
                "MQL0201_UNRESOLVED_ID",
                Some(owner.to_owned()),
                format!("unknown requirement {id}"),
            );
        }
    }
    fn require_choice(&mut self, id: &str, owner: &str) {
        if !self.choice_ids.contains(id) {
            self.error(
                "MQL0201_UNRESOLVED_ID",
                Some(owner.to_owned()),
                format!("unknown choice {id}"),
            );
        }
    }
    fn require_fact(&mut self, id: &str, owner: &str) {
        if !self.fact_ids.contains(id) {
            self.error(
                "MQL0201_UNRESOLVED_ID",
                Some(owner.to_owned()),
                format!("unknown fact {id}"),
            );
        }
    }
    fn error(
        &mut self,
        code: &'static str,
        id: impl Into<Option<String>>,
        message: impl Into<String>,
    ) {
        self.errors.push(ValidationError::new(code, id, message));
    }
}

fn scalar_key(value: &ScalarValue) -> String {
    match value {
        ScalarValue::String(value) => value.clone(),
        ScalarValue::Integer(value) => value.to_string(),
        ScalarValue::Boolean(value) => value.to_string(),
    }
}

fn scalar_matches_type(value: &ScalarValue, value_type: ChoiceValueType) -> bool {
    matches!(
        (value, value_type),
        (ScalarValue::String(_), ChoiceValueType::String)
            | (ScalarValue::Integer(_), ChoiceValueType::Integer)
            | (ScalarValue::Boolean(_), ChoiceValueType::Boolean)
    )
}

fn contribution_unit_matches(kind: ContributionKind, unit: ContributionUnit) -> bool {
    matches!(
        (kind, unit),
        (ContributionKind::Slot, ContributionUnit::Slots)
            | (
                ContributionKind::DistinctAttempts,
                ContributionUnit::Courses
            )
            | (
                ContributionKind::DistinctCanonicalCourses,
                ContributionUnit::Courses
            )
            | (ContributionKind::Credits, ContributionUnit::CreditUnits)
            | (ContributionKind::Assignments, ContributionUnit::Assignments)
            | (
                ContributionKind::NestedSatisfaction,
                ContributionUnit::BooleanChildren
            )
    )
}

fn is_author_id(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('a'..='z'))
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

fn is_catalog_year(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 9
        && bytes[4] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || byte.is_ascii_digit())
}

fn is_qualified_id(value: &str) -> bool {
    value.split("::").all(is_author_id)
}
