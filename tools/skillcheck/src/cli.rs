//! The command tree and the entry point that turns arguments into an exit code.
//!
//! `skillcheck lint skills` and `skillcheck eval routing|uplift` take Bench's former arguments
//! and give its exit codes and stderr lines, with `skillcheck` as the prefix.

use std::ffi::OsString;
use std::io::Write;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Args, CommandFactory, Parser, Subcommand};
use serde::Serialize;

use crate::command::CommandName;
use crate::envelope::{BINARY, Envelope, Finding};
use crate::error::CliError;
use crate::error_chain::reason;
use crate::eval::{self, EvalArgs};
use crate::exit_codes::render::{command_help, overview_help};
use crate::exit_codes::{self, Code};
use crate::lint::finding::{LintFinding, Severity};
use crate::lint::{self, SkillsArgs};
use crate::pipe::Pipe;
use crate::profile::profile;

/// The flags clap answers by printing instead of running a command.
const HELP_AND_VERSION: [&str; 4] = ["-h", "--help", "-V", "--version"];

/// `skillcheck`: the parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = BINARY,
    version,
    about = "skillcheck: skill QA for Redkiln plugins (design rules, routing and uplift evals)",
    disable_help_subcommand = true,
    after_help = overview_help()
)]
pub struct Cli {
    /// The command to run; none prints this help.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// The top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint skills against the skill design rules
    #[command(after_help = command_help(CommandName::Lint))]
    Lint(LintArgs),
    /// Run evaluation suites, such as skill routing
    #[command(after_help = command_help(CommandName::Eval))]
    Eval(Box<EvalArgs>),
}

/// Arguments of `skillcheck lint`.
#[derive(Debug, Args)]
pub struct LintArgs {
    /// What to lint.
    #[command(subcommand)]
    pub target: LintTarget,
}

/// The things `skillcheck lint` checks.
#[derive(Debug, Subcommand)]
pub enum LintTarget {
    /// Check skill directories against the 21 skill design rules
    #[command(after_help = command_help(CommandName::Lint))]
    Skills(SkillsArgs),
}

/// Runs `skillcheck` with `args` (including the program name), writing results to `out` and
/// errors to `err`, and returns the exit code. `out` is flushed before returning.
///
/// On failure `err` gets exactly one `skillcheck: error: <reason>` line, the reason holding the
/// error's cause chain, and the code is the error's documented code, never 0. When `out`'s
/// reader closes it (a broken pipe), the rest of the output is dropped and the command exits
/// with the code it earned, saying nothing on `err` (CLI-07).
pub fn run<I, T, O, E>(args: I, out: O, mut err: E) -> Code
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    O: Write,
    E: Write,
{
    let mut out = Pipe::new(out);
    let result = execute(args, &mut out, &mut err)
        .and_then(|code| out.flush().map(|()| code).map_err(CliError::Output));
    let error = match result {
        Ok(code) => return code,
        Err(error) => error,
    };
    let line = reason(&error).replace(['\r', '\n'], " ");
    match writeln!(err, "{BINARY}: error: {line}") {
        Ok(()) => error.code(),
        // stderr is closed too: the exit code is the only report left.
        Err(_unreportable) => exit_codes::OUTPUT,
    }
}

/// Parses `args` and runs the command.
fn execute<I, T, O, E>(args: I, out: &mut Pipe<O>, err: &mut E) -> Result<Code, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    O: Write,
    E: Write,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    match Cli::try_parse_from(&args) {
        Err(error) => clap_outcome(&error, args.last(), out),
        Ok(Cli { command: None }) => {
            let help = Cli::command().render_help();
            write!(out, "{help}").map_err(CliError::Output)?;
            Ok(exit_codes::OK)
        }
        Ok(Cli {
            command: Some(Command::Lint(args)),
        }) => match &args.target {
            LintTarget::Skills(skills) => {
                let profile = profile(&skills.plugin)?;
                let outcome = lint::lint(skills, &profile)?;
                report(&outcome, out)
            }
        },
        Ok(Cli {
            command: Some(Command::Eval(args)),
        }) => eval::run(&args, |var| std::env::var_os(var), out, err),
    }
}

/// The envelope's `data` for `lint skills`: counts by severity and every finding with its
/// rule.
#[derive(Debug, Serialize)]
struct Data<'a> {
    checked: usize,
    cached: usize,
    errors: usize,
    warnings: usize,
    reviews: usize,
    details: Vec<Detail<'a>>,
}

#[derive(Debug, Serialize)]
struct Detail<'a> {
    key: String,
    rule: u8,
    #[serde(flatten)]
    finding: &'a LintFinding,
}

/// Writes the `lint skills` envelope for `outcome` and returns its code: 1 when any finding is
/// an error, else 0. Warnings and review items are listed but never fail the run.
fn report<O: Write>(outcome: &lint::Outcome, out: &mut O) -> Result<Code, CliError> {
    let findings = &outcome.findings;
    let count = |severity| findings.iter().filter(|f| f.severity == severity).count();
    let errors = count(Severity::Error);
    let data = Data {
        checked: outcome.skills,
        cached: outcome.cached,
        errors,
        warnings: count(Severity::Warning),
        reviews: count(Severity::Review),
        details: findings
            .iter()
            .map(|finding| Detail {
                key: finding.key(),
                rule: finding.checker.rule(),
                finding,
            })
            .collect(),
    };
    let code = if errors > 0 {
        exit_codes::LINT_FINDINGS
    } else {
        exit_codes::LINT_CLEAN
    };
    let envelope_findings: Vec<Finding> = findings.iter().map(LintFinding::to_envelope).collect();
    Envelope::new(CommandName::Lint, Some("skills"), code, data)
        .with_findings(&envelope_findings)
        .emit(out)
        .map_err(CliError::Output)
}

/// `--help` and `--version` go to stdout and exit 0 when they are the last argument; anything
/// after them, and anything else clap rejects, is a usage error, so it never exits 0.
fn clap_outcome<O: Write>(
    error: &clap::Error,
    last: Option<&OsString>,
    out: &mut O,
) -> Result<Code, CliError> {
    let flag_is_last = last.is_some_and(|arg| HELP_AND_VERSION.iter().any(|flag| arg == flag));
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion if !flag_is_last => {
            Err(CliError::Usage {
                reason: "unexpected argument after --help or --version".to_owned(),
            })
        }
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            write!(out, "{}", error.render()).map_err(CliError::Output)?;
            Ok(exit_codes::OK)
        }
        _ => Err(CliError::Usage {
            reason: usage_reason(error),
        }),
    }
}

/// One line: `unknown argument: <arg>` for an unknown command or flag, `missing required
/// argument: <arg>, ...` naming every missing one, else clap's first line.
fn usage_reason(error: &clap::Error) -> String {
    if error.kind() == ErrorKind::MissingRequiredArgument
        && let Some(ContextValue::Strings(missing)) = error.get(ContextKind::InvalidArg)
        && !missing.is_empty()
    {
        return format!("missing required argument: {}", missing.join(", "));
    }
    let unknown = match error.kind() {
        ErrorKind::InvalidSubcommand => error.get(ContextKind::InvalidSubcommand),
        ErrorKind::UnknownArgument => error.get(ContextKind::InvalidArg),
        _ => None,
    };
    if let Some(ContextValue::String(arg)) = unknown {
        return format!("unknown argument: {arg}");
    }
    let rendered = error.render().to_string();
    let first = rendered.lines().next().unwrap_or("invalid arguments");
    first.strip_prefix("error: ").unwrap_or(first).to_owned()
}

#[cfg(test)]
mod tests {
    use super::run;

    fn ran(args: &[&str]) -> (u8, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(
            std::iter::once("skillcheck").chain(args.iter().copied()),
            &mut out,
            &mut err,
        );
        (
            code.value(),
            String::from_utf8(out).expect("UTF-8"),
            String::from_utf8(err).expect("UTF-8"),
        )
    }

    #[test]
    fn an_unknown_command_is_one_usage_error_line_and_nothing_on_stdout() {
        let (code, out, err) = ran(&["frobnicate"]);

        assert_eq!(code, 2, "usage error");
        assert_eq!(out, "", "nothing on stdout");
        assert_eq!(err, "skillcheck: error: unknown argument: frobnicate\n");
    }

    #[test]
    fn no_command_prints_the_help_and_exits_0() {
        let (code, out, err) = ran(&[]);

        assert_eq!((code, err.as_str()), (0, ""), "help is a success");
        assert!(out.contains("lint") && out.contains("eval"), "{out}");
    }

    #[test]
    fn a_missing_skill_directory_is_a_usage_error_with_its_cause_chain() {
        let (code, out, err) = ran(&["lint", "skills", "no/such/dir"]);

        assert_eq!((code, out.as_str()), (2, ""), "usage error");
        assert_eq!(err, "skillcheck: error: lint: no such path: no/such/dir\n");
    }

    #[test]
    fn a_plugin_with_no_profile_is_a_usage_error_before_any_path_is_read() {
        let (code, out, err) = ran(&["lint", "skills", "no/such/dir", "--plugin", "weighin"]);

        assert_eq!((code, out.as_str()), (2, ""), "usage error");
        assert_eq!(
            err,
            "skillcheck: error: lint: no skill profile for plugin \"weighin\"\n"
        );
    }
}
