# Changelog

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

