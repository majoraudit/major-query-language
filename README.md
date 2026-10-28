# major-requirement-parser

## MQL 0.2

Package 0.3.0 compiles MQL 0.2 programs ([RFC 0002](docs/rfcs/0002-surface-grammar.md)) into the versioned, solver-independent semantic IR ([RFC 0001](docs/rfcs/0001-semantic-ir.md)):

```mql
program ee_electives {
  catalog 2026-2027
  institution "https://yale.example/mql/institution" @ "2026-2027"
  credit_scale 2
}

requirement electives "Four electives, three advanced" [elective] {
  courses exactly 4 where course.subject = "ECE" and course.format != "research";

  constraint three_advanced "At least three at level 4000":
    at least 3 attempts where course.level >= 4000;
}
```

```console
mql --compile ee.mql --institution examples/yale-institution.json -o ee.json
```

Field types come from the institution schema, which is required when a program uses predicates (`where …`) or group fields. Programs that only use course lists compile without one. Errors are reported as `file:line:column: CODE: message`. The [fixture sources](docs/fixtures/semantic-ir/source/) show every construct.

The compiled IR has `schema_version` `0.2.0`. That version is independent of the package version.

Rust:

```rust
let institution = libmql::institution::Institution::from_json(&institution_json)?;
let program = libmql::compiler::compile(&source, "ee.mql", Some(&institution))?;
```

Python:

```py
ir = pylibmql.compile_program(source, "ee.mql", institution_json)
ir.json_pretty()
```

Existing IR documents are validated and canonicalized with `mql --validate-ir program.json` or `pylibmql.validate_semantic_ir(json)`. This package does not perform degree audits or multi-term planning.

The legacy 0.1 language is still the default for `mql FILE` and `pylibmql.parse(...)`. Its output reports language version `0.1.12`.

## Components
- Command Line Interface
- Rust Library
- Python Library (FFI bindings)

### Command Line Usage

```
Usage: mql [OPTIONS] [INPUT]

Arguments:
  [INPUT]
          Path to the input file

Options:
  -o, --output <OUTPUT>
          Path to the output file [Default: output to `stdout`]

      --compile
          Compile an MQL 0.2 source file to validated semantic IR JSON

      --institution <FILE>
          Institution schema JSON that types predicate fields and checks facts

      --validate-ir
          Validate and canonicalize an MQL 0.2 semantic IR JSON document

  -D, --var <NAME=EXPR>
          Inject a legacy (0.1) parser external value

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

### Python Installation

```bash
# minimum version 3.8
pip install pylibmql
```
Ready to use right away.
```py
import pylibmql
```

`pylibmql.parse(...)` retains the legacy 0.1 language. New programs should be written in MQL 0.2 and compiled with `pylibmql.compile_program(...)`.

## Legacy MQL 0.1 Syntax

The MQL 0.2 grammar is specified in [RFC 0002](docs/rfcs/0002-surface-grammar.md). The 0.1 grammar below is retained for `mql FILE` and `pylibmql.parse`.

```ebnf
File              ::= SpecialStatement ";" { SpecialStatement ";" } ;

SpecialStatement  ::= Statement ":" String [ ":" QuantitySingle ] ;

Statement         ::= "SELECT" Quantity "FROM" Selector ;

Selector          ::= SelectorList | SelectorSingle ;
SelectorList      ::= "[" SelectorSingle { "," SelectorSingle } "]" ;
SelectorSingle    ::= Statement | XYZ ;

XYZ               ::= QueryName "(" QueryArgument { "," QueryArgument } ")" ;
QueryName         ::= "CLASS" | "class" | "PLACEMENT" | "placement"
                    | "TAG" | "tag" | "RANGE" | "range" ;
QueryArgument     ::= String | ClassArgument ;

ClassArgument     ::= DepartmentId ClassId ;
DepartmentId      ::= UppercaseLetter UppercaseLetter UppercaseLetter UppercaseLetter ;
ClassId           ::= Digit Digit Digit Digit ;

Quantity          ::= QuantityMany | QuantitySingle ;
QuantityMany      ::= QuantitySingle "-" QuantitySingle ;
QuantitySingle    ::= Digit { Digit } ;  (* one or more digits; non-empty *)

String            ::= "\"" { "\"\"" | (AnyCharExcept["\"", "\r", "\n"]) } "\"" ;

(* Lexical *)
UppercaseLetter   ::= "A" | ... | "Z" ;
Digit             ::= "0" | ... | "9" ;
```
