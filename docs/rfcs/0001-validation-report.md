# RFC 0001 Validation Report

Validated against the artifacts in this repository on 2026-10-28.

## Results

- All seven JSON documents under `docs/schemas/` parse as JSON.
- All seven schemas pass `Draft202012Validator.check_schema`.
- The institution registry validates against its schema.
- All eight expanded programs validate against the MQL IR schema.
- All eight catalog inputs validate against the catalog schema.
- Every transcript, profile, and expected audit trace validates against its corresponding schema.
- The four schema-negative mutations in `negative-cases.json` fail validation for the expected keyword.
- The Rust validator also rejects every schema-negative mutation.
- Of the four semantic-negative cases, the Rust validator covers `MQL0201_UNRESOLVED_ID`, `MQL0203_CONFLICTING_EFFECTS`, and `MQL0204_ADVISORY_SCOPE`. `MQL0202_DECLARED_CHOICE_MISSING` is an audit-time diagnostic and is reserved for the future audit engine. All four codes are normative.

The positive validation covered 8 fixture families and reported 0 errors.

## Reproduction

Use any JSON Schema Draft 2020-12 implementation. Resolve each object in `reference-fixtures.json` against these schemas:

| Object | Schema |
| --- | --- |
| `institution` | `institution-schema-0.2.schema.json` |
| `fixtures[*].program` | `mql-ir-0.2.schema.json` |
| `fixtures[*].catalog` | `catalog-0.2.schema.json` |
| `cases[*].transcript` | `transcript-0.2.schema.json` |
| `cases[*].profile` | `profile-0.2.schema.json` |
| `cases[*].expected` | `audit-evaluation-0.2.schema.json` |

Apply the RFC 6902-style `add`, `remove`, and `replace` operations in `negative-cases.json` to a deep copy of the referenced base object. The descriptive semantic operations are intentionally deferred to the future semantic validator.

## Schema and Rust validator alignment

`mql-ir-0.2.schema.json` is the structural contract, and the Rust validator applies the schema rules plus semantic rules. Any document the schema rejects is also rejected by `SemanticProgram::from_json`, `pylibmql.validate_semantic_ir()`, and `mql --validate-ir`. Rust reports schema-level rules with these stable codes:

| Code | Schema rule |
| --- | --- |
| `MQL0200_SCHEMA_VERSION` | `schema_version` and `migration.from_version` constants |
| `MQL0206_INVALID_ID` | ID patterns, including roles and field names |
| `MQL0228_EMPTY_VALUE` | `minItems` / `minLength` on sources, requirements, and required strings |
| `MQL0229_CATALOG_YEAR` | `catalog_year` pattern |
| `MQL0230_DUPLICATE_ENTRY` | `uniqueItems` |

One known difference remains. JSON Schema treats an integral number such as `2.0` as an integer, but Rust rejects it during deserialization. Producers must write integers without a fractional part.

## Implementation boundary

The Rust IR types, JSON deserialization, semantic reference validation, Python binding, and CLI validation mode are implemented. Their unit tests load these normative fixtures directly.

The MQL 0.2 surface grammar, degree-audit evaluator, CP-SAT compiler, and planning engine remain outside this implementation. The legacy 0.1 parser continues to operate independently until the grammar RFC is approved.

