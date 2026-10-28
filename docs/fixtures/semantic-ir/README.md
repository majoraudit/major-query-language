# MQL 0.2 Semantic IR reference fixtures

`reference-fixtures.json` contains eight reduced, fully expanded semantic programs and their catalog, transcript, profile, expected result, assignment witness, and normative trace data.

These fixtures are semantic examples, not reproductions of the complete Yale catalog text. Each deliberately isolates a difficult interaction while retaining stable IDs and source provenance. The companion schema is `../../schemas/reference-fixtures-0.2.schema.json`.

The cases collectively include complete, incomplete, indeterminate, rejected-assignment, shared-course, inactive-branch, fact-effect, and equal-optimum outcomes. The later conformance suite must split these interactions into isolated positive and negative tests for every P1–P20 pattern.


`source/` contains each program as MQL 0.2 source ([RFC 0002](../../rfcs/0002-surface-grammar.md)). Compiling a file with the fixture document's `institution` produces that fixture's `program`, apart from source spans, which are illustrative in the JSON.
