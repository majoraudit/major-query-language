use libmql::{
    compiler::{CompileError, compile},
    institution::Institution,
    semantic_ir::{Bound, Constraint, ContributionKind, Obligation, Selector, SemanticProgram},
};
use serde_json::Value;

const FIXTURES: &str = include_str!("../../docs/fixtures/semantic-ir/reference-fixtures.json");
const SOURCE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../docs/fixtures/semantic-ir/source"
);

fn fixture_document() -> Value {
    serde_json::from_str(FIXTURES).unwrap()
}

fn institution() -> Institution {
    serde_json::from_value(fixture_document()["institution"].clone()).unwrap()
}

const HEADER: &str = r#"program test_program {
  catalog 2026-2027
  institution "https://yale.example/mql/institution" @ "2026-2027"
  credit_scale 2
}
"#;

fn compile_body(body: &str) -> Result<SemanticProgram, CompileError> {
    compile(&format!("{HEADER}{body}"), "test.mql", Some(&institution()))
}

fn codes(body: &str) -> Vec<&'static str> {
    match compile_body(body) {
        Ok(_) => panic!("expected compilation to fail"),
        Err(error) => error.diagnostics.iter().map(|d| d.code).collect(),
    }
}

/// Spans in the reference fixtures are illustrative, so compare everything else.
fn strip_spans(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("span");
            map.remove("provenance");
            map.values_mut().for_each(strip_spans);
        }
        Value::Array(items) => items.iter_mut().for_each(strip_spans),
        _ => {}
    }
}

#[test]
fn every_reference_fixture_compiles_from_source_to_its_expected_ir() {
    let document = fixture_document();
    let institution = institution();
    for fixture in document["fixtures"].as_array().unwrap() {
        let mut expected = fixture["program"].clone();
        let path = expected["sources"][0]["path"].as_str().unwrap().to_owned();
        let file_name = path.rsplit('/').next().unwrap();
        let source = std::fs::read_to_string(format!("{SOURCE_DIR}/{file_name}"))
            .unwrap_or_else(|error| panic!("missing source for {path}: {error}"));

        let program =
            compile(&source, &path, Some(&institution)).unwrap_or_else(|error| panic!("{error}"));
        let mut actual = serde_json::to_value(&program).unwrap();
        strip_spans(&mut actual);
        strip_spans(&mut expected);
        assert_eq!(actual, expected, "{path} did not compile to its fixture");
    }
}

#[test]
fn spans_point_at_declarations() {
    let program =
        compile_body("requirement core_req \"Core\" {\n  slot from [CPSC 2010];\n}\n").unwrap();
    let span = &program.requirements[0].meta.span;
    assert_eq!(span.start.line, 6);
    assert_eq!(span.start.column, 1);
    assert_eq!(span.end.line, 8);
    assert_eq!(program.requirements[0].meta.provenance, vec![span.clone()]);
    assert_eq!(program.sources[0].path, "test.mql");
}

#[test]
fn syntax_errors_report_their_position() {
    let error = compile_body("requirement core_req {\n  courses exactly from [CPSC 2010];\n}\n")
        .unwrap_err();
    let diagnostic = &error.diagnostics[0];
    assert_eq!(diagnostic.code, "MQL0300_SYNTAX");
    let span = diagnostic.span.as_ref().unwrap();
    assert_eq!((span.start.line, span.start.column), (7, 19));
    assert!(error.to_string().contains("test.mql:7:19: MQL0300_SYNTAX"));
}

#[test]
fn keywords_require_a_word_boundary() {
    // `intro` starts with the keyword `in`, `orders` with `or`.
    let program = compile_body(
        "requirement intro { slot from [PSYC 1100]; }\nrequirement orders { slot from [PSYC 1200]; }\n",
    )
    .unwrap();
    assert_eq!(program.requirements[1].meta.id, "orders");
    assert!(
        codes("requirement x { courses atleast 1 from [PSYC 1100]; }").contains(&"MQL0300_SYNTAX")
    );
}

#[test]
fn bounds_cover_exactly_at_most_and_between() {
    let program = compile_body(
        "requirement a { courses at most 2 from [CPSC 2010]; }\n\
         requirement b { courses between 1 and 3 from [CPSC 2020]; }\n\
         requirement c { credits exactly 1.5 from [CPSC 2030] each 0.5 credits; }\n",
    )
    .unwrap();
    let bounds: Vec<_> = program
        .requirements
        .iter()
        .map(|r| (r.lower.clone(), r.upper.clone()))
        .collect();
    assert!(matches!(
        bounds[0],
        (Bound::Fixed { value: 0 }, Some(Bound::Fixed { value: 2 }))
    ));
    assert!(matches!(
        bounds[1],
        (Bound::Fixed { value: 1 }, Some(Bound::Fixed { value: 3 }))
    ));
    assert!(matches!(
        bounds[2],
        (Bound::Fixed { value: 3 }, Some(Bound::Fixed { value: 3 }))
    ));
    assert_eq!(
        program.requirements[2].contribution.fixed_credit_units,
        Some(1)
    );
}

#[test]
fn nested_requirements_become_children_and_inherit_obligation() {
    let program = compile_body(
        "optional requirement track \"Pick one\" {\n\
           children at least 1;\n\
           requirement path_a { slot from [CPSC 2010]; }\n\
           advisory requirement path_b { slot from [CPSC 2020]; }\n\
         }\n",
    )
    .unwrap();
    let track = &program.requirements[0];
    assert_eq!(
        track.contribution.kind,
        ContributionKind::NestedSatisfaction
    );
    assert!(
        matches!(&track.selector, Selector::ChildRequirements { child_ids } if child_ids == &["path_a", "path_b"])
    );
    assert_eq!(track.children[0].obligation, Obligation::Optional);
    assert_eq!(track.children[1].obligation, Obligation::Advisory);
}

#[test]
fn every_constraint_and_effect_form_compiles() {
    let program = compile_body(
        r#"choice lab "Lab track": declared one of "wet", "dry";
fact dus "DUS substitution": approval dus_substitution { for: "core" };
requirement core { courses exactly 2 from [CPSC 2010, CPSC 2020]; }
requirement extra { credits at least 2 from [CPSC 3010]; }
requirement alt { slot from [CPSC 2030]; }
constraint a in direct core: all where course.level >= 2000;
constraint b in subtree core: limit 1 credits where course.format = "lab";
constraint c in requirements [core, extra]: at most 3 canonical courses where course.subject in ["CPSC", "MATH"];
constraint d in branch lab = "wet": order course.level <= 2000 before course.level >= 3000;
constraint e when lab != "dry" in program: group course.group at most 2 per group;
constraint f in program: total at least 3 canonical courses where not course.group is null;
effect g when fact dus: substitute core with alt;
effect h when lab = "wet": set lower of extra to 1.5;
effect i when fact dus and lab = "dry": make extra optional;
effect j when fact dus: permit core where course.subject = "MATH";
effect k when not fact dus: deactivate alt;
"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(program.constraints.len(), 6);
    assert_eq!(program.effects.len(), 5);
    assert_eq!(
        program.effects[1].value,
        Some(3),
        "credit bound uses credit_scale"
    );
    assert!(matches!(
        &program.constraints[1],
        Constraint::Limit { maximum: 2, .. }
    ));
    assert_eq!(program.facts[0].parameters["for"], "core");
}

#[test]
fn predicates_require_an_institution_schema() {
    let source = format!("{HEADER}requirement r {{ slot where course.level >= 2000; }}");
    let error = compile(&source, "test.mql", None).unwrap_err();
    assert_eq!(error.diagnostics[0].code, "MQL0305_UNKNOWN_FIELD");

    let plain = format!("{HEADER}requirement r {{ slot from [CPSC 2010]; }}");
    compile(&plain, "test.mql", None).expect("course lists need no institution schema");
}

#[test]
fn institution_schema_checks_fields_operators_and_values() {
    assert!(
        codes("requirement r { slot where course.colour = \"red\"; }")
            .contains(&"MQL0305_UNKNOWN_FIELD")
    );
    assert!(
        codes("requirement r { slot where course.level > 2000; }")
            .contains(&"MQL0306_OPERATOR_NOT_ALLOWED")
    );
    assert!(
        codes("requirement r { slot where course.format = \"studio\"; }")
            .contains(&"MQL0311_UNKNOWN_VALUE")
    );
    assert!(
        codes("requirement r { slot where attempt.grade >= \"E\"; }")
            .contains(&"MQL0311_UNKNOWN_VALUE")
    );
}

#[test]
fn institution_schema_checks_fact_types() {
    assert!(
        codes("fact x: no_such_fact;\nrequirement r { slot from [CPSC 2010]; }")
            .contains(&"MQL0308_UNKNOWN_FACT_TYPE")
    );
    assert!(
        codes("fact x: dus_substitution;\nrequirement r { slot from [CPSC 2010]; }")
            .contains(&"MQL0308_UNKNOWN_FACT_TYPE")
    );
    assert!(codes("fact x: approval dus_substitution { bogus: 1 };\nrequirement r { slot from [CPSC 2010]; }").contains(&"MQL0308_UNKNOWN_FACT_TYPE"));
}

#[test]
fn institution_must_match_the_program_header() {
    let mut other = institution();
    other.version = "2025-2026".to_owned();
    let source = format!("{HEADER}requirement r {{ slot from [CPSC 2010]; }}");
    let error = compile(&source, "test.mql", Some(&other)).unwrap_err();
    assert_eq!(error.diagnostics[0].code, "MQL0307_INSTITUTION_MISMATCH");
}

#[test]
fn structural_mistakes_have_stable_codes() {
    assert!(codes("requirement r { }").contains(&"MQL0301_COUNT_STATEMENT"));
    assert!(
        codes("requirement r { slot from [A 100]; slot from [CPSC 2010]; }")
            .contains(&"MQL0300_SYNTAX")
    );
    assert!(
        codes("requirement r { slot from [CPSC 2010]; slot from [CPSC 2020]; }")
            .contains(&"MQL0301_COUNT_STATEMENT")
    );
    assert!(
        codes("requirement r { courses from [CPSC 2010]; }").contains(&"MQL0302_MISSING_BOUND")
    );
    assert!(
        codes("requirement r { credits at least 1.25 from [CPSC 2010]; }")
            .contains(&"MQL0303_AMOUNT")
    );
    assert!(
        codes("requirement r { courses at least 1.5 from [CPSC 2010]; }")
            .contains(&"MQL0303_AMOUNT")
    );
    assert!(codes("requirement r { courses at least 1; }").contains(&"MQL0304_MISSING_SELECTOR"));
    assert!(
        codes("requirement r { slot from [CPSC 2010]; }\nconstraint c: total at least 1 attempts;")
            .contains(&"MQL0309_MISSING_SCOPE")
    );
}

#[test]
fn semantic_validation_errors_carry_source_spans() {
    let error = compile_body("requirement r [core] {\n  slot from [CPSC 2010];\n  constraint c in direct missing: all where course.level >= 1000;\n}\n")
        .unwrap_err();
    let diagnostic = error
        .diagnostics
        .iter()
        .find(|d| d.code == "MQL0201_UNRESOLVED_ID")
        .expect("unresolved scope target is reported");
    assert_eq!(diagnostic.semantic_id.as_deref(), Some("c"));
    assert_eq!(diagnostic.span.as_ref().unwrap().start.line, 8);
}
