use std::path::{Path, PathBuf};

use clap::Parser;

const CLI_LONG_ABOUT: &str = r#"

 /$$      /$$  /$$$$$$  /$$      
| $$$    /$$$ /$$__  $$| $$      
| $$$$  /$$$$| $$  \ $$| $$      
| $$ $$/$$ $$| $$  | $$| $$      
| $$  $$$| $$| $$  | $$| $$      
| $$\  $ | $$| $$/$$ $$| $$      
| $$ \/  | $$|  $$$$$$/| $$$$$$$$
|__/     |__/ \____ $$$|________/
                   \__/          
                                 
Compile MQL 0.2 programs to semantic IR (--compile), validate semantic IR
JSON (--validate-ir), or parse legacy 0.1 files (the default). Degree auditing
and planning are not performed by this CLI.

* You can find the .mql specification at 
  https://github.com/mrodz/major-requirement-parser/tree/main
* Check out MajorAudit at
  https://github.com/majoraudit"#;

#[derive(Parser, Debug)]
#[command(about, long_about = CLI_LONG_ABOUT)]
pub(crate) struct Args {
    /// Path to the input file
    input: Option<PathBuf>,
    /// Path to the output file [Default: output to `stdout`]
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Inject a value for an extern variable declared in the .mql file.
    /// Format: NAME=EXPR where EXPR is any valid MQL value expression.
    /// Example: --var 'myClass=MATH 2250' --var 'courses=[MATH 2250, MATH 2260]'
    #[arg(long = "var", short = 'D', value_name = "NAME=EXPR")]
    vars: Vec<String>,

    /// Validate and canonicalize an MQL 0.2 semantic IR JSON document.
    #[arg(long, conflicts_with = "vars")]
    validate_ir: bool,

    /// Compile an MQL 0.2 source file to validated semantic IR JSON.
    #[arg(long, conflicts_with_all = ["vars", "validate_ir"])]
    compile: bool,

    /// Institution schema JSON that types predicate fields and checks facts.
    #[arg(long, value_name = "FILE", requires = "compile")]
    institution: Option<PathBuf>,

    #[arg(long = "version", short = 'V', action = clap::ArgAction::SetTrue)]
    version: bool,
}

impl Args {
    pub fn input(&self) -> Option<&Path> {
        self.input.as_deref()
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn vars(&self) -> &[String] {
        &self.vars
    }

    pub fn version(&self) -> bool {
        self.version
    }

    pub fn validate_ir(&self) -> bool {
        self.validate_ir
    }

    pub fn compile(&self) -> bool {
        self.compile
    }

    pub fn institution(&self) -> Option<&Path> {
        self.institution.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::Args;
    use clap::Parser;

    #[test]
    fn accepts_semantic_ir_validation_mode() {
        let args = Args::try_parse_from(["mql", "--validate-ir", "program.json"]).unwrap();
        assert!(args.validate_ir());
        assert_eq!(args.input().unwrap().to_str(), Some("program.json"));
    }

    #[test]
    fn accepts_compile_mode_with_institution() {
        let args = Args::try_parse_from([
            "mql",
            "--compile",
            "program.mql",
            "--institution",
            "yale.json",
        ])
        .unwrap();
        assert!(args.compile());
        assert_eq!(args.institution().unwrap().to_str(), Some("yale.json"));
    }

    #[test]
    fn institution_requires_compile_mode() {
        assert!(
            Args::try_parse_from(["mql", "program.mql", "--institution", "yale.json"]).is_err()
        );
        assert!(
            Args::try_parse_from(["mql", "--compile", "--validate-ir", "program.mql"]).is_err()
        );
    }

    #[test]
    fn semantic_ir_mode_rejects_legacy_external_variables() {
        assert!(
            Args::try_parse_from(["mql", "--validate-ir", "program.json", "--var", "COUNT=2",])
                .is_err()
        );
    }
}
