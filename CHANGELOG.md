# Changelog

## 0.3.0

Adds the MQL 0.2 surface grammar ([RFC 0002](docs/rfcs/0002-surface-grammar.md)).

- Adds `libmql::compiler::compile`, which turns MQL 0.2 source into validated semantic IR. Diagnostics have stable `MQL03xx` codes and source positions.
- Adds `mql --compile program.mql [--institution schema.json]` and `pylibmql.compile_program()`.
- Adds `libmql::institution::Institution`. The compiler uses it to type predicate fields and to check operators, enum and grade values, and fact types.
- Ships the eight RFC 0001 reference fixtures as MQL 0.2 source. CI compiles them and checks the output against the JSON Schema.
- Fixes IR serialization so absent optional fields are omitted instead of written as `null`. 0.2.0 wrote `null`, which the schema rejects.
- Rejects `SELECTED_SET` constraints that count `ASSIGNMENTS`, matching the schema.
- The IR `schema_version` stays `0.2.0`. The package and IR versions now move independently.
- `mql FILE` still parses legacy 0.1 source; 0.2 source requires `--compile`.

## 0.2.0

Initial semantic-IR release.

- Adds the versioned, solver-independent MQL 0.2 semantic IR.
- Adds Rust deserialization and semantic validation with stable diagnostics.
- Aligns Rust validation with the JSON Schema: every schema-invalid document is also rejected in Rust (see the RFC 0001 validation report).
- Adds `pylibmql.validate_semantic_ir()` and `pylibmql.__version__`.
- Adds CLI validation and canonicalization with `mql --validate-ir program.json`.
- Publishes normative JSON Schemas and eight reduced Yale reference fixtures.
- Retains the legacy parser without relabeling its output; legacy documents continue to report language version `0.1.12`.
- Does not yet include the MQL 0.2 surface grammar, an audit solver, or a planning engine.

