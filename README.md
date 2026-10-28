# major-requirement-parser

## MQL 0.2 semantic IR

Release 0.2.0 exposes the solver-independent MQL 0.2 semantic IR alongside the legacy 0.1 surface parser. The grammar itself has not been migrated yet. Legacy parse results continue to identify their language semantics as `0.1.12`; `pylibmql.__version__` identifies the installed package as `0.2.0`.

Rust callers can deserialize and validate a versioned IR document with:

```rust
use libmql::semantic_ir::SemanticProgram;

let program = SemanticProgram::from_json(json)?;
assert_eq!(program.schema_version, "0.2.0");
```

Python callers use `pylibmql.validate_semantic_ir(json)`. The returned object provides `version()`, `json()`, and `json_pretty()`.

The CLI validates and canonicalizes an IR document with:

```console
mql --validate-ir program.json
```

The normative RFC, schemas, and fixtures are under [`docs/`](docs/rfcs/0001-semantic-ir.md). This API validates the semantic model; it does not yet perform degree audits or multi-term planning.

## Components
- Command Line Interface
- Rust Library
- Python Library (FFI bindings)

### Command Line Usage

```
Usage: mql [OPTIONS] <INPUT>

Arguments:
  <INPUT>
          Path to the input file

Options:
  -o, --output <OUTPUT>
          Path to the output file [Default: output to `stdout`]

      --validate-ir
          Validate and canonicalize an MQL 0.2 semantic IR JSON document

  -D, --var <NAME=EXPR>
          Inject a legacy parser external value

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

`pylibmql.parse(...)` retains the legacy 0.1 language. New consumers should exchange and validate the versioned semantic IR with `pylibmql.validate_semantic_ir(...)`.

## MQL Syntax

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
