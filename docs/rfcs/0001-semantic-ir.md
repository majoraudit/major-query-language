# RFC 0001: MQL 0.2 Semantic Intermediate Representation

- Status: Implemented (semantic IR and validation layer)
- Target: MQL 0.2.0
- Catalog: Yale College Programs of Study 2026–2027
- Schema: `../schemas/mql-ir-0.2.schema.json`
- Fixtures: `../fixtures/semantic-ir/reference-fixtures.json`

## 1. Purpose and boundary

This RFC defines the normative, solver-independent semantic intermediate representation (IR) for MQL 0.2. It replaces the consumer-defined meaning of the 0.1 selector tree with versioned data whose counting, activation, scope, sharing, and explanation behavior is explicit.

This RFC does not define surface MQL syntax, a parser implementation, a solver, or multi-term planning. The surface grammar in [RFC 0002](0002-surface-grammar.md) is one producer of this IR. A conforming audit engine consumes the IR, institution schema, catalog, transcript, and profile without reading source comments or Yale prose.

Normative terms MUST, MUST NOT, SHOULD, and MAY have their RFC 2119 meanings.

## 2. Program envelope and versions

An `MqlProgram` contains:

- `schema_version`, exactly `0.2.0` for this RFC;
- a stable `program_id` and catalog year;
- an institution schema reference and version;
- `credit_scale`, the positive integer number of stored units in one course credit;
- a registry of source documents;
- named choices, facts, requirements, constraints, sharing rules, and effects.

All credit quantities in IR and input records are integers in the declared scale. Consumers MUST NOT choose a different scale. For example, with scale 2, one half-credit is 1 unit and one-and-a-half credits is 3 units. Decimal credit values are invalid.

Schema version and package version are distinct values but begin together at `0.2.0`. A breaking wire-format or semantic change increments the schema major or minor version as appropriate.

## 3. Identity, hierarchy, and provenance

Author IDs match `^[a-z][a-z0-9_]*$` within a program namespace. Imported IDs are qualified as `namespace::id`. Requirement and constraint references always use author IDs, never descriptions.

Every semantic node also has a generated `node_id`. After module expansion, a producer walks roots and children in serialized order. Named nodes use `program_id::author_id`; anonymous descendants append `/nN`, where `N` is the zero-based sibling position. Constraint nodes append `/cN`. Generated IDs are deterministic for identical expanded input and unique within a program, but author IDs are the stable cross-revision references.

Every node and constraint has a `SourceSpan` containing a source ID, UTF-8 byte start and end offsets, and one-based start and end line/column positions. Expanded content retains a list of provenance spans: the definition span followed by each use site. Synthetic migration nodes identify the legacy source span and migration operation.

## 4. Requirement nodes

A requirement declares:

- identity, description, roles, source span, and provenance;
- an activation condition;
- an obligation: `REQUIRED`, `OPTIONAL`, or `ADVISORY`;
- a contribution expression and unit;
- a nonnegative lower bound and optional upper bound;
- zero or more children and attached constraints.

Authored roots default to `REQUIRED`. Nested nodes inherit the enclosing obligation unless explicitly marked otherwise.

- Active `REQUIRED` roots determine program completion and default progress.
- Active `OPTIONAL` roots are evaluated normally but do not independently affect completion or default progress. Their assignments MAY participate in an explicitly named total or scope.
- `ADVISORY` roots are informational. They do not receive assignments and are excluded from completion, totals, and progress unless a constraint explicitly names them, in which case schema validation rejects the program unless the constraint type documents advisory support.

## 5. Activation, bounds, satisfaction, and progress

For a node `n`, let `active(n)` be its Boolean activation condition and `q(n)` its nonnegative integer contribution in its declared unit.

If inactive:

```text
q(n) = 0
satisfied(n) = false
```

If active and the upper bound is finite:

```text
0 <= q(n) <= upper(n)
```

Satisfaction is:

```text
satisfied(n) iff active(n) AND q(n) >= lower(n)
```

Upper bounds cap selected assignments, not matching input records. An unselected transcript course cannot make a requirement fail.

Assignments and descendants inherit enclosing activation. No inactive assignment may affect contribution, usage, totals, progress, or explanations except as an explicitly reported rejected candidate.

For each active required root with a positive lower bound:

```text
progress(n) = min(q(n), lower(n)) / lower(n)
```

Progress is an exact rational. Default aggregate progress sums only unsatisfied active required roots. Implementations compare exact rationals using integer arithmetic; floating-point rounding MUST NOT affect the selected witness.

Program completion is a feasibility property. Every ordinary constraint is mandatory while active; informational statements use advisory requirements rather than non-enforced constraints:

```text
program_complete iff every active REQUIRED root can be satisfied
                     AND every active constraint can be enforced
                     simultaneously
```

For incomplete audits, the default explanatory witness lexicographically maximizes:

1. the number of satisfied active required roots;
2. the exact sum of normalized progress of unsatisfied active required roots;
3. assignments witnessed by course attempts rather than manual approvals.

Further deterministic tie breakers MAY be used but cannot change completion or per-root progress. The legacy `priority` field is display-only.

## 6. Contribution kinds

Every requirement has one declared contribution kind:

- `SLOT`: 0 or 1 for an assigned attempt or satisfied alternative.
- `DISTINCT_ATTEMPTS`: number of distinct assigned transcript attempts.
- `DISTINCT_CANONICAL_COURSES`: number of canonical course identities represented.
- `CREDITS`: sum of credit units after scope deduplication.
- `ASSIGNMENTS`: number of assignment edges, including explicitly permitted sharing.
- `NESTED_SATISFACTION`: sum of satisfied child Booleans.

An ordinary selector occurrence is a slot and is filled at most once. A course pool selects multiple distinct attempts. A credit pool sums assigned credit units. A nested requirement used as one alternative contributes its satisfaction Boolean unless its parent explicitly declares a flattened child contribution.

Audit credit comes from `TranscriptAttempt.earned_credit_units`. Planning credit comes from an offering's catalog credit. A requirement's fixed-credit override takes precedence. Failed, withdrawn, or otherwise non-credit-bearing attempts contribute zero and are ineligible unless a rule explicitly permits them.

## 7. Course identity and assignment

`canonical_course_id` identifies a catalog course across cross-listed and renamed codes. `attempt_id` identifies one transcript attempt. `enrolled_under_code` records the code actually used by the student.

Assignment and sharing operate on attempts. By default:

- an attempt fills at most one slot inside a named requirement;
- an attempt is exclusive across named requirements;
- repeated attempts are distinct but remain subject to canonical-course and repeat rules.

Canonical-course restrictions are separate constraints. Credits are properties of attempts and are not independently shared.

## 8. Predicates and eligibility

Predicates are typed expression trees, never expression strings. A field reference names its record target and schema-declared field. Supported targets are catalog course, offering, transcript attempt, and student profile.

Field types are Boolean, integer, normalized credit, string, enum, ordered grade, course code, canonical course ID, term, or sets of those values. The institution schema declares nullability, normalization, enum members, grade order, aliases, and supported joins.

Operators are restricted by type:

- equality and inequality for all comparable scalar types;
- ordering for integer, credit, grade, and term types;
- `IN`, `CONTAINS`, and set overlap for compatible scalar/set pairs;
- `AND`, `OR`, and `NOT` for Boolean expressions;
- `IS_NULL` and `IS_UNKNOWN` only for nullable or unavailable fields.

An ordinary predicate evaluates false when required data is missing. Strings compare exactly after schema-defined normalization. Grades compare by institution ordering, never lexical order.

Course-code predicates declare `CANONICAL`, `ENROLLED_AS`, or `EITHER` matching. A catalog/transcript join is legal only when listed in the institution schema.

## 9. Scopes and selected-set constraints

Every constraint declares an activation condition and a scope:

- `DIRECT`: attempts directly contributing to one requirement;
- `SUBTREE`: assignments beneath one requirement;
- `REQUIREMENTS`: a named requirement set;
- `ROLES`: all active requirements with any named role;
- `ACTIVE_BRANCH`: one selected choice branch;
- `PROGRAM`: all active program assignments.

The default selected-set scope is distinct attempts directly assigned to and contributing to the named requirement. It excludes ancestors, descendants, siblings, and inactive branches. Sharing an attempt into another requirement does not duplicate it within one selected-set scope.

Selected-set constraints support at least, at most, and exactly counts; credit subtotals; named subgroup membership; and all-selected predicate enforcement. Descendant assignments require an explicit `SUBTREE` or other named scope.

## 10. Grouping, limits, and totals

Grouping constraints operate on a typed attribute of the scoped selected set:

- minimum or exact number of distinct groups;
- exactly one common group (`SAME`);
- per-group minimum or maximum.

Missing group attributes do not satisfy a group and are rejected unless explicitly tested as unknown.

Limits always declare a scope. Unscoped limits are invalid in 0.2.

Totals declare both a scope and a counting mode: distinct attempts, distinct canonical courses, credits, or assignments. Conceptually:

```text
used(item, scope) = OR of qualifying assignments in scope
```

Shared attempts count once in distinct-attempt and scoped-credit totals, once per canonical identity in canonical totals, and once per permitted edge in assignment totals.

## 11. Sharing

The program default is `EXCLUSIVE`. A sharing rule has an activation condition and names two or more requirement scopes. `SHARE a, b, c` permits every pair and permits the same attempt to appear in all three scopes.

Descendants inherit the nearest enclosing named requirement identity unless they declare their own author ID. Sharing never permits reuse in two slots within the same named requirement. Repeat, canonical-course, grade, and other constraints can still forbid an otherwise shareable assignment.

Each receiving requirement gets the attempt's applicable credit contribution. A distinct scoped total containing several receiving requirements counts that attempt once.

## 12. Choices and conditions

A choice declares a value type, finite alternatives, exactly-one cardinality, owner, and source span:

- `DECLARED`: supplied by the profile; missing data makes evaluation indeterminate.
- `SOLVER`: selected among catalog-equivalent alternatives during audit.
- `FACT`: determined by a typed fact condition.
- `PLANNING`: available only to future-plan construction and unable to alter the current audit.

An audit never changes a declared choice to improve completion. Conditions can compare choice values, facts, and typed profile fields. Branch activation is derived from the condition and inherited by every descendant.

## 13. Facts, approvals, and effects

Facts and approvals are validated against the institution schema. Approvals identify their authority and target and remain visible as non-course witnesses. Planning cannot invent an approval or external fact.

Version 0.2 supports a closed set of declarative effects:

1. activate or deactivate a requirement or branch;
2. replace a lower or upper bound with a typed value;
3. waive a requirement while preserving the fact or approval witness;
4. substitute one requirement node for another;
5. add a named obligation;
6. permit a particular assignment or selector as a substitution.

Effects are evaluated from the original program state, collected, and applied as a set; source order has no meaning. Identical effects are idempotent. Different simultaneous effects writing the same activation, bound, substitution target, or obligation are conflicts and make the program evaluation invalid. Every applied or conflicting effect appears in the explanation trace.

## 14. Repeats and ordering

Repeat constraints declare a canonical-course selector, maximum count, and scope. Attempts remain distinct, but no more than the permitted number may contribute in that scope.

Audit ordering compares transcript terms. `BEFORE(a,b)` is satisfied only when a contributing attempt for `a` has a strictly earlier ordered term than the contributing attempt for `b`. Missing or incomparable terms do not satisfy the rule.

Future planning will use planned-term variables and must separately define prerequisites, concurrency, offering uncertainty, term caps, and plan ranking. Those decisions are outside this RFC.

## 15. Explanations and candidate usefulness

Every eligibility decision, assignment, rejection, contribution, constraint result, effect, and objective component references its constraint or node ID and source span.

A candidate is useful for requirement `R` iff an assignment exists that uses the candidate, preserves the complete baseline optimal objective vector, and improves satisfaction or exact progress for `R`. The engine fixes objective values, not a particular baseline assignment. It reports whether the candidate counts directly, counts after reassignment, or is blocked, with named blocking constraints.

## 16. Institution data contracts

The companion schemas define catalog, offering, transcript, profile, fact, grade-order, alias, and field registries. Institution-specific tags, departments, aliases, facts, and enum members are schema data rather than parser constants.

The semantic IR references the institution schema by stable ID and version. A consumer rejects absent fields used by a predicate, incompatible field types, unresolved fact names, and unavailable joins before solving.

## 17. Composition boundary

The IR reserves namespaces, provenance, and fully qualified IDs for deterministic module expansion. The expanded 0.2 IR contains no unresolved imports. Module syntax, parameters, extension/replacement rules, and cycle handling are defined by a later RFC and cannot alter the semantics in this document.

## 18. Compatibility and migration

MQL 0.2.0 is a breaking release. The future 0.2 parser accepts only v2 source and emits only schema-versioned 0.2 IR. It does not auto-detect or silently reinterpret 0.1.12 source.

A future command has this contract:

```text
mql migrate --from 0.1.12 --to 0.2.0 INPUT
```

It parses with frozen 0.1.12 semantics and emits proposed v2 source plus a structured migration report. It refuses to guess ambiguous selector-list, `LIMIT`, priority, or `PLACEMENT` meaning. Refused constructs identify source spans, possible interpretations, and required author action. The command is implemented only after v2 grammar approval.

## 19. Validation and diagnostics

Validation occurs before evaluation and rejects:

- duplicate, malformed, or unresolved IDs;
- invalid or advisory-only scopes;
- cyclic conditions or effects;
- incompatible contribution and bound units;
- absent institution fields or illegal predicate operators;
- unscoped limits;
- conflicting effects;
- assignments or totals that reference inactive or nonexistent requirements;
- attempts to plan approvals as courses;
- unsupported schema versions.

Diagnostics include a stable code, message, primary span, related spans, and referenced semantic IDs.

## 20. Reference artifacts and approval

The schemas and fixtures adjacent to this RFC are normative examples. The traceability matrix maps P1–P20 to RFC rules and cases.

This RFC is approved only when:

- all schemas are valid Draft 2020-12 documents;
- reference data validates against them;
- expected traces follow these rules without unstated choices;
- every mandatory requirement and P1–P20 has positive and negative coverage planned or present;
- no consequential semantic question remains open.

