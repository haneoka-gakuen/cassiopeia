use haneoka_cassiopeia_cli::summarize;
use haneoka_cassiopeia_core::{CassiopeiaChart, RoundingProfile};
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const USAGE: &str = "Usage:\n  cassiopeia-cli conformance <chart.json|->";

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for CliError {}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cassiopeia-cli: {error}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let Some(input) = parse_args(std::env::args().skip(1))? else {
        println!("{USAGE}");
        return Ok(());
    };
    let source = read_source(&input)?;
    let chart: CassiopeiaChart = serde_json::from_str(&source)?;
    let summary = summarize(&chart, RoundingProfile::ExactRational)?;

    let stdout = io::stdout();
    let mut output = io::BufWriter::new(stdout.lock());
    serde_json::to_writer_pretty(&mut output, &summary)?;
    writeln!(output)?;
    Ok(())
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Option<String>, CliError> {
    match args.next().as_deref() {
        None | Some("-h" | "--help") => return Ok(None),
        Some("conformance") => {}
        Some(command) => return Err(CliError(format!("unknown command {command:?}\n{USAGE}"))),
    }

    let mut input = None;
    for argument in args {
        if argument.starts_with('-') && argument != "-" {
            return Err(CliError(format!("unknown option {argument:?}")));
        } else if input.replace(argument).is_some() {
            return Err(CliError("only one chart input may be provided".into()));
        }
    }

    let input = input.ok_or_else(|| CliError(format!("missing chart input\n{USAGE}")))?;
    Ok(Some(input))
}

fn read_source(input: &str) -> io::Result<String> {
    if input == "-" {
        let mut source = String::new();
        io::stdin().read_to_string(&mut source)?;
        Ok(source)
    } else {
        fs::read_to_string(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_stdin_as_the_chart_source() {
        let parsed = parse_args(["conformance", "-"].into_iter().map(String::from))
            .unwrap()
            .unwrap();
        assert_eq!(parsed, "-");
    }

    #[test]
    fn rejects_multiple_chart_inputs() {
        let error = parse_args(
            ["conformance", "one.json", "two.json"]
                .into_iter()
                .map(String::from),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "only one chart input may be provided");
    }
}
