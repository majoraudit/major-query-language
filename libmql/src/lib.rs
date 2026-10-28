mod closest_string;
mod yale_departments;

mod parser;
pub mod semantic_ir;

#[path = "parser.rs"]
#[cfg_attr(test, visibility::make(pub))]
pub mod test_parser {
    pub use super::parser::*;
}

use anyhow::{Context, Result};

use parser::{MQLParser, Rule};

use pest::Parser;

use crate::parser::MQLQueryFile;

pub use parser::VarValue;

/// Version of the `libmql` package and its public SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Normative version of documents emitted by the retained legacy parser.
///
/// Package 0.2 validates the new semantic IR, but it does not silently relabel
/// the old ambiguous syntax as language version 0.2.
pub const LEGACY_LANGUAGE_VERSION: &str = "0.1.12";

#[derive(Debug, Clone)]
pub struct ParseResult {
    parsed_mql_file: MQLQueryFile,
}

impl ParseResult {
    pub fn to_string_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.parsed_mql_file)
    }

    pub fn to_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&self.parsed_mql_file)
    }

    pub fn parsed_mql_file(&self) -> &MQLQueryFile {
        &self.parsed_mql_file
    }
}

/// Parse an MQL expression string into a [`VarValue`] suitable for use as an extern variable.
///
/// Accepts the same syntax as variable assignment values, e.g.:
/// - `"MATH 2250"` → `VarValue::Class`
/// - `"[MATH 2250, MATH 2260]"` → `VarValue::SelectorList`
/// - `"\"some string\""` → `VarValue::String`
/// - `"3"` → `VarValue::Quantity`
pub fn parse_extern_value(expr: &str) -> Result<VarValue> {
    use std::collections::HashMap;
    let mut pairs = MQLParser::parse(Rule::var_value, expr)
        .map_err(|e| e.renamed_rules(parser::renamed_rules_impl))?;
    MQLParser::parse_var_value(
        pairs.next().context("var_value should have a child")?,
        &HashMap::new(),
    )
}

fn parse_impl(
    text: &str,
    externals: std::collections::HashMap<String, VarValue>,
) -> Result<ParseResult> {
    let mut pairs = MQLParser::parse(Rule::file, text)
        .map_err(|e| e.renamed_rules(parser::renamed_rules_impl))?;

    let parsed_mql_file = MQLParser::parse_file(
        pairs.next().context("File should have a child rule")?,
        externals,
    )?;

    Ok(ParseResult { parsed_mql_file })
}

/// Parse an MQL file with no extern variable injections.
pub fn parse(text: &dyn AsRef<str>) -> Result<ParseResult> {
    parse_impl(text.as_ref(), std::collections::HashMap::new())
}

/// Parse an MQL file, providing values for any `extern` variables declared at the top of the file.
///
/// `externals` keys must include the `$` sigil (e.g. `"$myVar"`).
/// Use [`parse_extern_value`] to construct values from MQL expression strings.
pub fn parse_with_externals(
    text: &dyn AsRef<str>,
    externals: std::collections::HashMap<String, VarValue>,
) -> Result<ParseResult> {
    parse_impl(text.as_ref(), externals)
}
