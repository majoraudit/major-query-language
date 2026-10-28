# RFC 0002: MQL 0.2 Surface Grammar

- Status: Implemented in package 0.3.0
- Language version: MQL 0.2 (emits semantic IR `schema_version` `0.2.0`)
- Grammar: `../../libmql/src/mql2.pest`
- Compiler: `libmql::compiler::compile`, `mql --compile`, `pylibmql.compile_program`
- Examples: `../fixtures/semantic-ir/source/` (the eight RFC 0001 fixtures as source)

## 1. Purpose

This RFC defines the authoring syntax for MQL 0.2. Every construct lowers to exactly one RFC 0001 IR node, so the IR's semantics apply unchanged. The compiler runs the IR validator on its output, so a compiled program obeys every rule a hand-written IR document does.

The 0.2 grammar is a separate language from 0.1.12. The tools never infer the version from file contents (RFC 0001 §18): `mql FILE` parses legacy 0.1 source, and `mql --compile FILE` compiles 0.2 source.

## 2. Example

```mql
program cs_systems {
  catalog 2026-2027
  institution "https://yale.example/mql/institution" @ "2026-2027"
  credit_scale 2
}

choice systems "Systems sequence": solver one of "split", "combined";

requirement systems_split "Split systems sequence" [core] when systems = "split" {
  courses exactly 2 from [CPSC 3231, CPSC 3232];
}

requirement electives "Electives vary by systems choice" [elective] {
  courses at least (systems: "split" => 1, "combined" => 2) from [CPSC 3650, CPSC 3660];
}
```

## 3. Lexical rules

- Whitespace is insignificant. Comments are `// …` and `/* … */`.
- Keywords are lowercase and must end at a word boundary: `intro` is an identifier, not `in` + `tro`.
- Identifiers become author IDs and must match `^[a-z][a-z0-9_]*$` (RFC 0001 §3).
- Strings use double quotes with `\"` and `\\` escapes.
- A course code is 2–5 uppercase letters or `&`, optional spaces, and 3–4 digits with an optional uppercase suffix: `CPSC 3231` becomes the canonical ID `CPSC3231`. A quoted string gives a canonical ID verbatim.
- The reserved words `and or not true false null fact in is where from when before match` cannot be identifiers.

## 4. Program header

```ebnf
file    ::= header item*
header  ::= "program" ident "{" ( "catalog" YYYY-YYYY
                                | "institution" string "@" string
                                | "credit_scale" integer )* "}"
item    ::= choice | fact | requirement | constraint | share | effect
```

`catalog` and `institution` are required, and each entry may appear once. `credit_scale` defaults to 1. If the compiler is given an institution schema, its `schema_id` and `version` must match the header.

## 5. Declarations

Every declaration starts with an ID and an optional description string. The ID becomes `meta.id`, the generated `node_id` is `program_id::id`, and the declaration's source span becomes both `meta.span` and the single provenance entry.

### Choices and facts

```ebnf
choice ::= "choice" ident string? ":" owner "one" "of" scalar ("," scalar)* ";"
owner  ::= "declared" | "solver" | "planning" | "fact" ident
fact   ::= "fact" ident string? ":" "approval"? ident ( "{" ident ":" scalar, … "}" )? ";"
```

A choice's `value_type` comes from its first alternative. The fact type (the second identifier) must exist in the institution schema when one is supplied. `approval` must match the schema's declaration, and parameters must be declared there.

### Requirements

```ebnf
requirement ::= obligation? "requirement" ident string? roles? ("when" condition)?
                "{" ( count | requirement | constraint )* "}"
obligation  ::= "required" | "optional" | "advisory"
roles       ::= "[" ident, … "]"
count       ::= kind bounds? selector? ("each" number "credits")? ";"
kind        ::= "slot" | "courses" | "canonical" "courses" | "credits" | "assignments" | "children"
bounds      ::= "exactly" amount | "at" "least" amount | "at" "most" amount
              | "between" amount "and" amount
amount      ::= number | "(" ident ":" scalar "=>" number, … ")"
selector    ::= "from" "[" course, … "]" | "from" "requirements" "[" ident, … "]"
              | "where" predicate
```

A requirement body has exactly one count statement.

| Count kind | Contribution | Unit |
| --- | --- | --- |
| `slot` | `SLOT` | `SLOTS` |
| `courses` | `DISTINCT_ATTEMPTS` | `COURSES` |
| `canonical courses` | `DISTINCT_CANONICAL_COURSES` | `COURSES` |
| `credits` | `CREDITS` | `CREDIT_UNITS` |
| `assignments` | `ASSIGNMENTS` | `ASSIGNMENTS` |
| `children` | `NESTED_SATISFACTION` | `BOOLEAN_CHILDREN` |

Bounds lower as follows. `exactly n` sets lower and upper to `n`. `at least n` leaves upper `null`. `at most n` sets lower to 0. `between a and b` sets lower `a` and upper `b`. A parenthesized amount is a `CHOICE_TABLE` bound. `slot` without bounds means exactly 1, and every other kind requires bounds.

Credit amounts, including `each n credits` (`fixed_credit_units`), are written in course credits and multiplied by `credit_scale`. A result that is not a whole unit is an error: `1.25` credits at scale 2 is rejected. Other amounts must be whole numbers.

Nested `requirement` blocks become `children` and inherit the parent's obligation unless they declare their own. A count statement without a selector selects its nested children (`CHILD_REQUIREMENTS`). `when` defaults to `CONSTANT true`.

### Constraints

```ebnf
constraint ::= "constraint" ident string? ("when" condition)? ("in" scope)? ":" body ";"
scope      ::= "direct" ident | "subtree" ident | "requirements" "[" ident, … "]"
             | "roles" "[" ident, … "]" | "branch" ident "=" scalar | "program"
body       ::= ("at" "least" | "at" "most" | "exactly") number unit "where" predicate   (SELECTED_SET)
             | "all" "where" predicate                                                  (SELECTED_SET ALL)
             | "group" field ( "distinct" "at" "least" n | "distinct" "exactly" n | "same"
                             | "at" "most" n "per" "group" | "at" "least" n "per" "group" )
             | "limit" number unit "where" predicate
             | "total" "at" "least" number unit ("where" predicate)?
             | "repeat" "at" "most" n "times" "where" predicate
             | "order" predicate "before" predicate
unit       ::= "attempts" | "canonical" "courses" | "credits" | "assignments"
```

A constraint declared inside a requirement defaults to `in direct <that requirement>` and is added to its `constraint_ids`. A top-level constraint must declare its scope, because 0.2 has no unscoped constraints (RFC 0001 §10). `order` lowers to a strict `ORDERING`.

### Sharing and effects

```ebnf
share  ::= ("share" | "exclusive") ident string? ("when" condition)? ":" ident ("," ident)+ ";"
effect ::= "effect" ident string? "when" condition ":" action ";"
action ::= "waive" ident | "activate" ident | "deactivate" ident
         | "set" ("lower" | "upper") "of" ident "to" number
         | "substitute" ident "with" ident | "make" ident obligation
         | "permit" ident "where" predicate
```

`set … to n` is measured in the target requirement's unit, so a credit requirement's value is scaled by `credit_scale`.

## 6. Conditions and predicates

```ebnf
condition ::= condition "or" condition | condition "and" condition | "not" condition
            | "(" condition ")" | "true" | "false" | "fact" ident | ident ("=" | "!=") scalar
predicate ::= predicate "or" predicate | predicate "and" predicate | "not" predicate
            | "(" predicate ")" | field "is" ("null" | "unknown")
            | field op value ("match" ("canonical" | "enrolled_as" | "either"))?
field     ::= ("course" | "offering" | "attempt" | "student") "." ident
op        ::= "=" | "!=" | "<" | "<=" | ">" | ">=" | "in" | "contains" | "overlaps"
value     ::= scalar | "null" | "[" value, … "]"
```

`not` binds tighter than `and`, and `and` binds tighter than `or`. Chains flatten into one `AND`/`OR` node. `choice != v` lowers to `NOT CHOICE_EQUALS`.

The targets `course`, `offering`, `attempt`, and `student` map to `CATALOG_COURSE`, `OFFERING`, `TRANSCRIPT_ATTEMPT`, and `STUDENT_PROFILE`. A field's type comes from the institution schema, so predicates and group fields need one. The compiler also checks that the schema allows the operator for that field, and that enum and grade literals are declared members.

## 7. Diagnostics

Compilation reports every diagnostic it finds, each with a stable code, a message, and a source span. IR validation codes (`MQL02xx`) carry the span of the declaration they name.

| Code | Meaning |
| --- | --- |
| `MQL0300_SYNTAX` | Source does not match the grammar |
| `MQL0301_COUNT_STATEMENT` | A requirement has zero or several count statements |
| `MQL0302_MISSING_BOUND` | A non-`slot` count statement has no bounds |
| `MQL0303_AMOUNT` | A credit amount is not a whole unit, or a count is fractional |
| `MQL0304_MISSING_SELECTOR` | A count statement has neither a selector nor nested requirements |
| `MQL0305_UNKNOWN_FIELD` | A field is undeclared, or no institution schema was supplied |
| `MQL0306_OPERATOR_NOT_ALLOWED` | The institution schema does not allow the operator on that field |
| `MQL0307_INSTITUTION_MISMATCH` | The supplied institution schema is not the one the header names |
| `MQL0308_UNKNOWN_FACT_TYPE` | The fact type, approval flag, or a parameter is not declared |
| `MQL0309_MISSING_SCOPE` | A top-level constraint has no `in` scope |
| `MQL0310_HEADER` | A header entry is missing or repeated |
| `MQL0311_UNKNOWN_VALUE` | An enum or grade literal is not declared |

## 8. Out of scope

Module imports and composition (RFC 0001 §17), and `mql migrate` from 0.1.12 (RFC 0001 §18), remain future work. Every node in 0.2 source has an author ID, so the anonymous `/nN` node IDs are not produced.
