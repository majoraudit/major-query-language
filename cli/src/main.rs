mod cli;

use anyhow::{Context, Result, bail};
use clap::Parser as ClapParser;

use std::collections::HashMap;
use std::io::Read;
use std::time::Instant;
use std::{fs::File, io::Write};

const EXTENSION: &str = "mql";

fn main() -> Result<()> {
    let start = Instant::now();

    let cli = cli::Args::parse();

    if let (true, _) | (_, None) = (cli.version(), cli.input()) {
        println!(
            "mql cli v{} libmql v{}",
            env!("CARGO_PKG_VERSION"),
            libmql::VERSION
        );
        return Ok(());
    }

    let input = cli.input();
    let output = cli.output();

    let expected_extension = if cli.validate_ir() { "json" } else { EXTENSION };
    if input.and_then(|x| x.extension()).and_then(|s| s.to_str()) != Some(expected_extension) {
        bail!("expected a .{expected_extension} input file")
    }

    if let Some(output_path) = output {
        let Some("json") = output_path.extension().and_then(|s| s.to_str()) else {
            bail!("the Major Query Language can only output to a .json file")
        };
    }

    let mut input_file =
        File::open(input.expect("input should not be none")).context("could not open file")?;

    let mut buf = String::new();

    input_file
        .read_to_string(&mut buf)
        .context("file was not UTF-8")?;

    if cli.validate_ir() {
        let program = libmql::semantic_ir::SemanticProgram::from_json(&buf)?;
        let rendered = serde_json::to_string_pretty(&program)?;
        if let Some(output_path) = cli.output() {
            let mut output_file = File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(output_path)
                .context("could not get handle to output file")?;
            output_file
                .write_all(rendered.as_bytes())
                .context("could not write output file")?;
        } else {
            println!("{rendered}");
        }
        return Ok(());
    }

    let mut externals = HashMap::new();
    for raw in cli.vars() {
        let (name, expr) = raw.split_once('=').with_context(|| {
            format!("--var argument must be of the form NAME=EXPR, got: {raw:?}")
        })?;
        let value = libmql::parse_extern_value(expr)
            .with_context(|| format!("failed to parse value for {name}: {expr:?}"))?;
        externals.insert(format!("${name}"), value);
    }

    let result = libmql::parse_with_externals(&buf, externals)?;

    if let Some(output_path) = cli.output() {
        let mut output_file = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(output_path)
            .context("could not get handle to output file")?;

        output_file
            .write(result.to_string_pretty()?.as_bytes())
            .context("could not write to output file")?;

        let elapsed_time = start.elapsed();

        println!(
            "output to {:?} in {}ms",
            output_path,
            elapsed_time.as_millis()
        )
    } else {
        println!("{}", result.to_string_pretty()?);
    }

    Ok(())
}
