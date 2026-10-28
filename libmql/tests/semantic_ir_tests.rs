use libmql::semantic_ir::{SemanticIrError, SemanticProgram};
use serde_json::{Value, json};

const FIXTURES: &str = include_str!("../../docs/fixtures/semantic-ir/reference-fixtures.json");

fn fixture_document() -> Value {
    serde_json::from_str(FIXTURES).expect("RFC fixture document must be JSON")
}

fn program(index: usize) -> Value {
    fixture_document()["fixtures"][index]["program"].clone()
}

fn validation_codes(value: &Value) -> Vec<&'static str> {
    match SemanticProgram::from_json(&value.to_string()) {
        Err(SemanticIrError::Validation(errors)) => {
            errors.into_iter().map(|error| error.code).collect()
        }
        Err(error) => panic!("expected semantic validation errors, got {error}"),
        Ok(_) => panic!("expected semantic validation to fail"),
    }
}

#[test]
fn all_eight_rfc_programs_deserialize_and_validate() {
    let document = fixture_document();
    let fixtures = document["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 8);

    for fixture in fixtures {
        let fixture_id = fixture["fixture_id"].as_str().unwrap();
        SemanticProgram::from_json(&fixture["program"].to_string())
            .unwrap_or_else(|error| panic!("fixture {fixture_id} failed: {error}"));
    }
}

#[test]
fn semantic_ir_round_trips_without_losing_meaning() {
    let original = SemanticProgram::from_json(&program(0).to_string()).unwrap();
    let encoded = serde_json::to_string(&original).unwrap();
    let decoded = SemanticProgram::from_json(&encoded).unwrap();

    assert_eq!(decoded.schema_version, "0.2.0");
    assert_eq!(decoded.program_id, "cs_systems");
    assert_eq!(decoded.requirements.len(), 3);
    assert_eq!(decoded.choices.len(), 1);
}

#[test]
fn rejects_wrong_schema_version() {
    let mut value = program(0);
    value["schema_version"] = json!("0.1.12");
    assert!(validation_codes(&value).contains(&"MQL0200_SCHEMA_VERSION"));
}

#[test]
fn rejects_duplicate_semantic_and_node_ids() {
    let mut value = program(0);
    value["requirements"][1]["meta"]["id"] = value["requirements"][0]["meta"]["id"].clone();
    value["requirements"][1]["meta"]["node_id"] =
        value["requirements"][0]["meta"]["node_id"].clone();
    let codes = validation_codes(&value);
    assert!(
        codes
            .iter()
            .filter(|code| **code == "MQL0207_DUPLICATE_ID")
            .count()
            >= 2
    );
}

#[test]
fn rejects_unresolved_constraint_reference() {
    let mut value = program(2);
    value["requirements"][0]["constraint_ids"][0] = json!("missing_constraint");
    assert!(validation_codes(&value).contains(&"MQL0201_UNRESOLVED_ID"));
}

#[test]
fn rejects_advisory_requirement_scope() {
    let mut value = program(2);
    value["requirements"][0]["obligation"] = json!("ADVISORY");
    assert!(validation_codes(&value).contains(&"MQL0204_ADVISORY_SCOPE"));
}

#[test]
fn rejects_invalid_scope_shape() {
    let mut value = program(2);
    value["constraints"][0]["scope"] = json!({ "kind": "DIRECT" });
    assert!(validation_codes(&value).contains(&"MQL0221_INVALID_SCOPE"));
}

#[test]
fn rejects_incomplete_choice_bound_table() {
    let mut value = program(0);
    value["requirements"][2]["lower"]["cases"]
        .as_object_mut()
        .unwrap()
        .remove("combined");
    assert!(validation_codes(&value).contains(&"MQL0218_INCOMPLETE_CHOICE_TABLE"));
}

#[test]
fn rejects_lower_bound_above_upper_bound() {
    let mut value = program(0);
    value["requirements"][0]["lower"]["value"] = json!(3);
    assert!(validation_codes(&value).contains(&"MQL0215_INVALID_BOUNDS"));
}

#[test]
fn rejects_conflicting_simultaneous_effects() {
    let mut value = program(5);
    let span = value["effects"][0]["meta"]["span"].clone();
    let provenance = value["effects"][0]["meta"]["provenance"].clone();
    value["effects"] = json!([
        {
            "meta": { "id": "lower_one", "node_id": "psychology_waiver::lower_one", "description": "Set lower one", "span": span, "provenance": provenance },
            "when": { "kind": "FACT_PRESENT", "fact_id": "ap5" },
            "kind": "REPLACE_BOUND", "target_id": "intro", "bound": "LOWER", "value": 1
        },
        {
            "meta": { "id": "lower_two", "node_id": "psychology_waiver::lower_two", "description": "Set lower two", "span": value["facts"][0]["meta"]["span"].clone(), "provenance": value["facts"][0]["meta"]["provenance"].clone() },
            "when": { "kind": "FACT_PRESENT", "fact_id": "ap5" },
            "kind": "REPLACE_BOUND", "target_id": "intro", "bound": "LOWER", "value": 2
        }
    ]);
    assert!(validation_codes(&value).contains(&"MQL0203_CONFLICTING_EFFECTS"));
}

#[test]
fn json_type_errors_are_distinct_from_semantic_errors() {
    let mut value = program(0);
    value["credit_scale"] = json!(1.5);
    assert!(matches!(
        SemanticProgram::from_json(&value.to_string()),
        Err(SemanticIrError::Json(_))
    ));
}

#[test]
fn rejects_incompatible_contribution_unit() {
    let mut value = program(0);
    value["requirements"][0]["contribution"]["unit"] = json!("CREDIT_UNITS");
    assert!(validation_codes(&value).contains(&"MQL0224_INCOMPATIBLE_UNIT"));
}

#[test]
fn rejects_generated_node_id_mismatch() {
    let mut value = program(0);
    value["requirements"][0]["meta"]["node_id"] = json!("arbitrary");
    assert!(validation_codes(&value).contains(&"MQL0226_NODE_ID_MISMATCH"));
}

#[test]
fn rejects_ordering_operator_for_string_field() {
    let mut value = program(2);
    value["constraints"][0]["predicate"]["field"]["type"] = json!("STRING");
    assert!(validation_codes(&value).contains(&"MQL0225_INVALID_OPERATOR"));
}

#[test]
fn rejects_choice_alternative_with_wrong_type() {
    let mut value = program(0);
    value["choices"][0]["alternatives"][1] = json!(2);
    assert!(validation_codes(&value).contains(&"MQL0223_CHOICE_TYPE"));
}

#[test]
fn rejects_empty_program_collections_and_strings() {
    let mut value = program(0);
    value["requirements"] = json!([]);
    value["sources"][0]["path"] = json!("");
    value["institution_schema"]["version"] = json!("");
    let codes = validation_codes(&value);
    assert!(
        codes
            .iter()
            .filter(|code| **code == "MQL0228_EMPTY_VALUE")
            .count()
            >= 3
    );
}

#[test]
fn rejects_malformed_catalog_year() {
    let mut value = program(0);
    value["catalog_year"] = json!("2024");
    assert!(validation_codes(&value).contains(&"MQL0229_CATALOG_YEAR"));
}

#[test]
fn rejects_malformed_roles_and_field_names() {
    let mut value = program(2);
    value["requirements"][0]["roles"] = json!(["Bad Role"]);
    value["constraints"][0]["predicate"]["field"]["field"] = json!("Bad Field");
    let codes = validation_codes(&value);
    assert!(
        codes
            .iter()
            .filter(|code| **code == "MQL0206_INVALID_ID")
            .count()
            >= 2
    );
}

#[test]
fn rejects_duplicate_list_entries() {
    let mut value = program(0);
    value["requirements"][0]["roles"] = json!(["core", "core"]);
    let alternative = value["choices"][0]["alternatives"][0].clone();
    value["choices"][0]["alternatives"]
        .as_array_mut()
        .unwrap()
        .push(alternative);
    let codes = validation_codes(&value);
    assert!(
        codes
            .iter()
            .filter(|code| **code == "MQL0230_DUPLICATE_ENTRY")
            .count()
            >= 2
    );
}

#[test]
fn rejects_unexpected_migration_source_version() {
    let mut value = program(0);
    value["migration"] = json!({ "from_version": "0.1.0", "operations": [] });
    assert!(validation_codes(&value).contains(&"MQL0200_SCHEMA_VERSION"));
}

#[test]
fn upper_bound_is_required_but_nullable() {
    let mut value = program(0);
    value["requirements"][0]["upper"] = Value::Null;
    SemanticProgram::from_json(&value.to_string()).expect("null upper bound is unbounded");

    value["requirements"][0]
        .as_object_mut()
        .unwrap()
        .remove("upper");
    assert!(matches!(
        SemanticProgram::from_json(&value.to_string()),
        Err(SemanticIrError::Json(_))
    ));
}

/// Every document the JSON Schema rejects must also be rejected in Rust.
#[test]
fn rust_rejects_every_schema_negative_case() {
    let document = fixture_document();
    let negative: Value = serde_json::from_str(include_str!(
        "../../docs/fixtures/semantic-ir/negative-cases.json"
    ))
    .unwrap();

    for case in negative["schema_cases"].as_array().unwrap() {
        let case_id = case["case_id"].as_str().unwrap();
        let base = case["base"].as_str().or(negative["base"].as_str()).unwrap();
        let pointer = base.split_once('#').unwrap().1;
        let mut value = document.pointer(pointer).unwrap().clone();

        let operation = &case["operation"];
        let path = operation["path"].as_str().unwrap();
        let (parent, key) = path.rsplit_once('/').unwrap();
        let parent = value.pointer_mut(parent).unwrap();
        match operation["op"].as_str().unwrap() {
            "remove" => {
                parent.as_object_mut().unwrap().remove(key);
            }
            "add" | "replace" => parent[key] = operation["value"].clone(),
            op => panic!("unsupported fixture operation {op}"),
        }

        assert!(
            SemanticProgram::from_json(&value.to_string()).is_err(),
            "{case_id} passed Rust validation but fails the schema"
        );
    }
}

/// Canonical output must be the same JSON the schema validated, with absent
/// optional fields omitted rather than written as `null`.
#[test]
fn canonical_serialization_reproduces_every_fixture_exactly() {
    let document = fixture_document();
    for fixture in document["fixtures"].as_array().unwrap() {
        let program = SemanticProgram::from_json(&fixture["program"].to_string()).unwrap();
        assert_eq!(
            serde_json::to_value(&program).unwrap(),
            fixture["program"],
            "{} changed when re-serialized",
            fixture["fixture_id"]
        );
    }
}
