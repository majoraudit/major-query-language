use libmql::{
    compiler::compile, institution::Institution, parse as libmql_parse,
    parse_extern_value as libmql_parse_extern_value,
    parse_with_externals as libmql_parse_with_externals, semantic_ir::SemanticProgram, ParseResult,
};
use pyo3::{exceptions::PyValueError, prelude::*};
use std::collections::HashMap;

#[pyclass]
pub struct MQL {
    result: ParseResult,
}

#[pyclass]
pub struct SemanticIR {
    program: SemanticProgram,
}

#[pymethods]
impl SemanticIR {
    fn version(&self) -> &str {
        &self.program.schema_version
    }

    fn json(&self) -> PyResult<String> {
        serde_json::to_string(&self.program)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))
    }

    fn json_pretty(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.program)
            .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))
    }
}

#[pymethods]
impl MQL {
    fn version(&self) -> &'static str {
        self.result.parsed_mql_file().version()
    }

    fn json(&self) -> PyResult<String> {
        self.result
            .to_string()
            .map_err(|e| PyErr::new::<PyValueError, _>(format!("{e:?}")))
    }

    fn json_pretty(&self) -> PyResult<String> {
        self.result
            .to_string_pretty()
            .map_err(|e| PyErr::new::<PyValueError, _>(format!("{e:?}")))
    }

    fn __str__(&self) -> String {
        format!("{:?}", self.result)
    }
}

#[pyfunction]
fn parse(mql: String) -> PyResult<MQL> {
    let result = libmql_parse(&mql).map_err(|e| PyErr::new::<PyValueError, _>(format!("{e:?}")))?;
    Ok(MQL { result })
}

#[pyfunction]
fn parse_with_externals(mql: String, externals: HashMap<String, String>) -> PyResult<MQL> {
    // Convert the HashMap<String, String> to HashMap<String, VarValue>
    let mut extern_vars = HashMap::new();
    for (key, value_str) in externals {
        let var_value = libmql_parse_extern_value(&value_str).map_err(|e| {
            PyErr::new::<PyValueError, _>(format!(
                "Error parsing extern value for {}: {:?}",
                key, e
            ))
        })?;
        extern_vars.insert(key, var_value);
    }

    let result = libmql_parse_with_externals(&mql, extern_vars)
        .map_err(|e| PyErr::new::<PyValueError, _>(format!("{e:?}")))?;
    Ok(MQL { result })
}

#[pyfunction]
fn parse_extern_value(expr: String) -> PyResult<String> {
    let var_value = libmql_parse_extern_value(&expr)
        .map_err(|e| PyErr::new::<PyValueError, _>(format!("{e:?}")))?;
    Ok(format!("{:?}", var_value))
}

#[pyfunction]
fn validate_semantic_ir(json: String) -> PyResult<SemanticIR> {
    SemanticProgram::from_json(&json)
        .map(|program| SemanticIR { program })
        .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))
}

#[pyfunction]
#[pyo3(signature = (source, path = "main.mql", institution = None))]
fn compile_program(
    source: String,
    path: &str,
    institution: Option<String>,
) -> PyResult<SemanticIR> {
    let institution = institution
        .map(|json| {
            Institution::from_json(&json).map_err(|e| {
                PyErr::new::<PyValueError, _>(format!("invalid institution schema: {e}"))
            })
        })
        .transpose()?;
    compile(&source, path, institution.as_ref())
        .map(|program| SemanticIR { program })
        .map_err(|e| PyErr::new::<PyValueError, _>(e.to_string()))
}

#[pymodule]
fn pylibmql(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    m.add_function(wrap_pyfunction!(parse_with_externals, m)?)?;
    m.add_function(wrap_pyfunction!(parse_extern_value, m)?)?;
    m.add_function(wrap_pyfunction!(validate_semantic_ir, m)?)?;
    m.add_function(wrap_pyfunction!(compile_program, m)?)?;
    m.add_class::<MQL>()?;
    m.add_class::<SemanticIR>()?;
    Ok(())
}
