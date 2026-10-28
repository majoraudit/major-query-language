//! Institution schema registry (`docs/schemas/institution-schema-0.2.schema.json`).
//!
//! The MQL 0.2 compiler resolves predicate fields, enum members, grades, and
//! fact types against this data instead of parser constants (RFC 0001 §16).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::semantic_ir::{FieldType, RecordTarget};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Institution {
    pub schema_id: String,
    pub version: String,
    pub normalization: Normalization,
    pub fields: Vec<InstitutionField>,
    pub enums: BTreeMap<String, Vec<String>>,
    pub grade_order: Vec<String>,
    pub joins: Vec<Join>,
    pub facts: Vec<FactType>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub department_aliases: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_values: Vec<String>,
}

impl Institution {
    pub fn from_json(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }

    pub fn field(&self, target: RecordTarget, name: &str) -> Option<&InstitutionField> {
        self.fields
            .iter()
            .find(|field| field.target == target && field.name == name)
    }

    pub fn fact(&self, name: &str) -> Option<&FactType> {
        self.facts.iter().find(|fact| fact.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Normalization {
    pub strings: StringNormalization,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StringNormalization {
    Exact,
    Nfc,
    NfcCasefold,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstitutionField {
    pub target: RecordTarget,
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: FieldType,
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_name: Option<String>,
    pub operators: Vec<FieldOperator>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FieldOperator {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    In,
    Contains,
    Overlaps,
    IsNull,
    IsUnknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Join {
    pub from_target: String,
    pub from_field: String,
    pub to_target: String,
    pub to_field: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactType {
    pub name: String,
    pub approval: bool,
    pub parameters: BTreeMap<String, String>,
}
