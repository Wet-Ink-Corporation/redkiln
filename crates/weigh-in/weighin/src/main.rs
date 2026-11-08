//! `weighin`: the single writer of the Weigh-In decision ledger.
//!
//! Output is written for a model to read at the lowest token cost: short lines, no decoration,
//! `--json` where a structure is needed.

#![forbid(unsafe_code)]

mod annotate;
mod ci;
mod cli_envelope;
mod commands;
mod context;
mod env;
mod git;
mod harness;
mod hooks;
mod journal;
mod legacy_import;
mod ops;
mod pane;
mod recall;
mod shell_bin;
mod state_file;
mod store;
mod text;

use anyhow::{Context as _, Result, bail};
use clap::{Args, Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use weighin_core::event::Surface;

use crate::env::{BoardMode, ConfigSource, Env, RepoClass, Vars};
use crate::ops::{DecideOp, DeferOp, Op};
use crate::text::Report;

// `help` and `about` attributes, not doc comments, carry text that names environment variables
// or tools: clap prints them verbatim, so they cannot take rustdoc's backticks.
#[derive(Parser)]
#[command(name = "weighin", version, about = "Weigh-In decision ledger")]
struct Cli {
    #[arg(long, global = true, help = harness::claude_code::SESSION_HELP)]
    session: Option<String>,
    /// Work as if run from this directory.
    #[arg(long, global = true)]
    dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Use this store file (wins over $REDKILN_STORE, $WEIGHIN_STORE and every config's `store`)"
    )]
    store: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Hook entry point: reads the hook JSON on stdin.
    Hook {
        /// The event name `hooks.json` passes (B-16). Unused: the event is read from stdin.
        #[arg(hide = true)]
        _event: Option<String>,
    },
    #[command(flatten)]
    Ledger(LedgerCmd),
    /// Every ledger event as JSON lines; with a subcommand, the journal refs (SPEC JR-07).
    Journal {
        #[command(subcommand)]
        cmd: Option<JournalCmd>,
    },
}

/// The journal-ref commands. Each writes the CLI envelope with `--json` and exits with SPEC
/// CLI-05's codes (`commands::journal`), unlike the other commands' 0 and 1.
#[derive(Subcommand)]
enum JournalCmd {
    /// Export the store's events to their journal refs (or inbox branches) now.
    Checkpoint(JsonArgs),
    /// Import journal refs, broken refs and unrelayed inboxes into the store.
    Import(JsonArgs),
    /// Move one inbox branch of origin into its journal ref (the relay workflow runs this).
    Relay(RelayArgs),
    /// git's pre-push hook: the checkpoint, unless only journal refs are pushed; never fails the
    /// push.
    PrePush(PrePushArgs),
}

#[derive(Args)]
struct RelayArgs {
    /// The inbox branch, `redkiln/inbox/<name>`.
    branch: String,
    #[arg(long)]
    json: bool,
}

/// git's pre-push arguments, as git gives them, so the hook script is one `exec`.
#[derive(Args)]
struct PrePushArgs {
    /// The remote git pushes to. Unused: journal refs go to origin.
    #[arg(value_name = "REMOTE")]
    _remote: String,
    /// The remote's URL. Unused.
    #[arg(value_name = "URL")]
    _url: Option<String>,
    #[arg(long)]
    json: bool,
}

/// Every command but `hook`: these report their errors and exit 1 on failure (a closed stdout
/// aside, which exits 0; see `report`).
#[derive(Subcommand)]
enum LedgerCmd {
    /// Print the marker convention and open counts.
    Prime,
    /// Create .kb/_ledger/config.json.
    Init(InitArgs),
    /// List open items (this session by default when a session is known).
    Open(OpenArgs),
    /// Show one item in full.
    Show(ItemArgs),
    /// Script-built evidence for one item (about 600 tokens at most).
    Context(ItemArgs),
    /// Raise a question by hand, or every `weigh-in` block on stdin with --markers.
    Raise(RaiseArgs),
    /// Apply a JSON array of operations from stdin (or --file) in one call.
    Batch(BatchArgs),
    /// Record a decision.
    Decide(DecideArgs),
    /// Defer with a named fact, a trigger, a resolve-by and a default.
    Defer(DeferArgs),
    /// Dismiss: `not-useful`, `recoverable`, `moot`, `already-decided:<ref>`, `duplicate-of:<id>`, `not-my-call:<who>`.
    Dismiss(DismissArgs),
    /// Reopen a decided, defaulted or dismissed item.
    Reopen(ReopenArgs),
    /// Defaults taken without you, and unattended items not yet triaged.
    Digest(JsonArgs),
    /// Action rate, not-useful rate, overrides, time to decide.
    Stats(JsonArgs),
    /// The commit trailer for a decided item.
    Trailer(TrailerArgs),
    /// Decisions still standing that are anchored at a file, nearest the line first.
    Decisions(DecisionsArgs),
    /// Raise every open question and new TODO in a document or source file (backfill).
    Scan(ScanArgs),
    /// Exchange with the Artifact board.
    Board {
        #[command(subcommand)]
        cmd: BoardCmd,
    },
    /// `weighin journal` with no subcommand (parsed as `Cmd::Journal`): every ledger event as
    /// JSON lines, as 1.x committed them in .kb/_ledger/journal/.
    #[command(skip)]
    Journal,
    /// Branch report for CI: open items, decisions since --base, append-only journals.
    CiCheck(CiCheckArgs),
}

#[derive(Args)]
struct CiCheckArgs {
    /// The revision the branch is compared with (a merge base, `origin/main`, a sha).
    #[arg(long)]
    base: String,
    /// The report as one JSON document (schema `weighin.ci-check/1`).
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct InitArgs {
    /// `own` or `client` (client repos default to a local board), written to
    /// .redkiln/suite.toml when there is none; refused when it sets another class. With no
    /// `--class`, a new suite.toml takes config.json's 1.x `repo_class`; with no class anywhere,
    /// `init` refuses and writes nothing.
    #[arg(long, value_enum)]
    class: Option<RepoClass>,
    /// `artifact`, `local` or `off`. Unchanged when not given, unless `--class` is.
    #[arg(long, value_enum)]
    board: Option<BoardMode>,
    #[arg(long)]
    board_url: Option<String>,
    #[arg(long)]
    owner: Option<String>,
    /// A 1.x ledger's place, kept in config.json's `store`: the store goes beside it (relative to
    /// the folder, or absolute). A new setup sets `store` in .redkiln/suite.toml instead.
    #[arg(long = "ledger")]
    ledger: Option<String>,
}

#[derive(Args)]
struct OpenArgs {
    #[command(flatten)]
    scope: OpenScope,
    #[arg(long, default_value_t = 12)]
    top: usize,
    /// Append a context pack for each listed item that has a path anchor.
    #[arg(long)]
    with_context: bool,
    #[arg(long)]
    json: bool,
}

/// Which items `open` lists, as its flags spell it; [`OpenScope::scope`] reads them as one
/// [`OpenFrom`].
#[derive(Args)]
struct OpenScope {
    /// Every open item in the repository, not only this session's.
    #[arg(long)]
    repo: bool,
    /// Every item, open or not (wins over `--repo`).
    #[arg(long)]
    all: bool,
}

/// Which items `open` lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenFrom {
    /// The open items of this session, or of the repository when no session is known.
    Session,
    /// The open items of the repository.
    Repo,
    /// Every item, open or not.
    All,
}

impl OpenScope {
    /// The one scope the flags name: `--all` wins over `--repo`, and neither is this session.
    fn scope(&self) -> OpenFrom {
        if self.all {
            OpenFrom::All
        } else if self.repo {
            OpenFrom::Repo
        } else {
            OpenFrom::Session
        }
    }
}

#[derive(Args)]
struct ItemArgs {
    id: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct RaiseArgs {
    #[arg(long)]
    q: Option<String>,
    #[arg(long)]
    kind: Option<String>,
    #[arg(long)]
    door: Option<String>,
    #[arg(long)]
    authority: Option<String>,
    #[arg(long)]
    did: Option<String>,
    #[arg(long)]
    flip: Option<String>,
    #[arg(long)]
    at: Option<String>,
    /// Read `weigh-in` blocks from stdin (the in-sitting sweep).
    #[arg(long)]
    markers: bool,
    #[arg(long, default_value = "manual")]
    channel: String,
}

#[derive(Args)]
struct BatchArgs {
    #[arg(long)]
    file: Option<PathBuf>,
}

#[derive(Args)]
struct DecideArgs {
    id: String,
    option: String,
    #[arg(long)]
    note: Option<String>,
    #[arg(long, default_value = "cli")]
    via: String,
}

#[derive(Args)]
struct DeferArgs {
    id: String,
    #[arg(long)]
    flip: String,
    #[arg(long)]
    trigger: String,
    #[arg(long = "by")]
    resolve_by: String,
    #[arg(long)]
    default: String,
    #[arg(long)]
    reason: Option<String>,
}

#[derive(Args)]
struct DismissArgs {
    id: String,
    why: String,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Args)]
struct ReopenArgs {
    id: String,
    #[arg(long)]
    note: Option<String>,
}

#[derive(Args)]
struct JsonArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct TrailerArgs {
    id: String,
}

#[derive(Args)]
struct DecisionsArgs {
    /// The file, relative to the working directory or absolute.
    #[arg(long)]
    path: PathBuf,
    /// Rank the anchors nearest this line first (else the newest decision first).
    #[arg(long)]
    line: Option<usize>,
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct ScanArgs {
    paths: Vec<PathBuf>,
}

#[derive(Subcommand)]
enum BoardCmd {
    #[command(about = "Write the board state document (push it with ArtifactData set, file_path)")]
    Export(ExportArgs),
    /// Apply answers read back from the board (a JSON file, an array, or a directory of documents).
    Apply(ApplyArgs),
}

#[derive(Args)]
struct ExportArgs {
    #[arg(long, conflicts_with = "stdout")]
    out: Option<PathBuf>,
    /// Print the state on stdout and write no file, under a `local` or `artifact` board (the pane).
    #[arg(long)]
    stdout: bool,
}

#[derive(Args)]
struct ApplyArgs {
    /// A JSON file, an array, or a directory of documents; `-` or nothing reads stdin.
    path: Option<PathBuf>,
    /// Where the answers were given: the Artifact board, or the local board pane.
    #[arg(long, value_enum, default_value_t = AnswerSurface::Board)]
    surface: AnswerSurface,
}

/// Where `board apply`'s answers were given; their decisions carry it as `via` (D-0017).
#[derive(Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum AnswerSurface {
    /// The claude.ai Artifact board.
    Board,
    /// The local board pane: it never publishes, and leaves raw items read-only.
    Pane,
}

impl AnswerSurface {
    fn surface(self) -> Surface {
        match self {
            Self::Board => Surface::Board,
            Self::Pane => Surface::Pane,
        }
    }

    /// Where, as a receipt's reason says it: `on the board`, `in the pane`.
    fn place(self) -> &'static str {
        match self {
            Self::Board => "on the board",
            Self::Pane => "in the pane",
        }
    }
}

fn main() -> ExitCode {
    let Cli {
        session,
        dir,
        store,
        cmd,
    } = Cli::parse();
    let globals = Globals {
        session,
        dir,
        store,
    };
    let cmd = match cmd {
        Cmd::Hook { .. } => return run_hook_guarded(),
        Cmd::Journal { cmd: Some(sub) } => return commands::journal::run(globals, sub),
        Cmd::Journal { cmd: None } => LedgerCmd::Journal,
        Cmd::Ledger(cmd) => cmd,
    };
    let stdout = io::stdout();
    let mut out = Stdout(stdout.lock());
    let (result, warnings) = run(globals, cmd, &mut out);
    let code = match result.and_then(|()| out.flush().map_err(anyhow::Error::from)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => report(&e),
    };
    warnings.iter().fold(code, |code, w| warn(w, code))
}

/// The options every command takes.
struct Globals {
    session: Option<String>,
    dir: Option<PathBuf>,
    store: Option<PathBuf>,
}

/// `weighin: <error chain>` on stderr and exit 1; a closed stdout (`weighin open | head`) is
/// not a failure, so it exits 0 without a word.
fn report(e: &anyhow::Error) -> ExitCode {
    if is_closed_stdout(e) {
        return ExitCode::SUCCESS;
    }
    match writeln!(io::stderr().lock(), "weighin: {e:#}") {
        // With stderr gone as well, the exit code is the only report left.
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}

/// The command's standard output. A broken pipe on it is tagged [`StdoutClosed`], so `report`
/// tells a reader that stopped reading from any other broken pipe (stderr, a child process).
struct Stdout<W>(W);

impl<W: Write> Write for Stdout<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf).map_err(StdoutClosed::tag)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush().map_err(StdoutClosed::tag)
    }
}

/// A broken pipe on standard output: whoever read it has stopped reading.
#[derive(Debug)]
struct StdoutClosed(io::Error);

impl StdoutClosed {
    fn tag(e: io::Error) -> io::Error {
        if e.kind() == io::ErrorKind::BrokenPipe {
            io::Error::new(io::ErrorKind::BrokenPipe, Self(e))
        } else {
            e
        }
    }
}

impl std::fmt::Display for StdoutClosed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "writing standard output: {}", self.0)
    }
}

impl std::error::Error for StdoutClosed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

fn is_closed_stdout(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<io::Error>()
            .and_then(io::Error::get_ref)
            .is_some_and(<dyn std::error::Error + Send + Sync>::is::<StdoutClosed>)
    })
}

/// A hook never fails a session, not even on a bug: a panic prints nothing and exits 0.
#[expect(
    clippy::disallowed_methods,
    reason = "process-boundary fault barrier: a hook never fails a session, even on a panic (AGENTS.md)"
)]
fn run_hook_guarded() -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(|| {
        harness::claude_code::entry(&mut io::stdin().lock(), &mut io::stdout().lock());
    });
    match outcome {
        // `entry` logs its own failures; a panic went to the silent panic hook above.
        // Either way the session goes on.
        Ok(()) | Err(_) => ExitCode::SUCCESS,
    }
}

/// Run `cmd`: its result, and the warnings to print after it whatever the result: what locating
/// the store found (a 2.x fallback), then what the one-time 1.x import did, which every command
/// but `ci-check` runs first (`legacy_import`, decision H5). An appending command exports
/// nothing: its events leave at the next Stop, `journal pre-push` or `journal checkpoint` (JR-08).
fn run(g: Globals, cmd: LedgerCmd, out: &mut impl Write) -> (Result<()>, Vec<String>) {
    let mut env = match environment(g) {
        Ok(env) => env,
        Err(e) => return (Err(e), Vec::new()),
    };
    // `ci-check` never opens the ledger (B-28).
    let legacy = match cmd {
        LedgerCmd::CiCheck(_) => Vec::new(),
        _ => legacy_warnings(&env),
    };
    let result = dispatch(&mut env, cmd, out);
    let mut warnings: Vec<String> = env.notices.iter().map(ToString::to_string).collect();
    warnings.extend(legacy);
    (result, warnings)
}

/// The 1.x import's notes as a command's warnings, in a Claude Code session less a source that
/// stays due, which its `SessionStart` tells (wi-fd5345); a failure is one warning, and the
/// command goes on (it stops itself if it cannot open the store).
fn legacy_warnings(env: &Env) -> Vec<String> {
    match legacy_import::import_due(env, legacy_import::Mode::Command, "cli") {
        Ok(legacy_import::Legacy::NothingDue) => Vec::new(),
        Ok(legacy_import::Legacy::Ran(report)) => {
            legacy_import::command_notes(&report, harness::claude_code::detect())
        }
        Err(e) => vec![format!("the weighin 1.x import did not run: {e:#}")],
    }
}

/// The directory a command works in: `--dir` made absolute, as Core locates from (a relative
/// one is the process's directory's), else the process's own.
///
/// # Errors
/// Neither path can be found.
fn working_dir(dir: Option<PathBuf>) -> Result<PathBuf> {
    match dir {
        Some(d) => std::path::absolute(&d)
            .with_context(|| format!("finding the absolute path of --dir {}", d.display())),
        None => std::env::current_dir().context("finding the working directory"),
    }
}

/// The environment the options name, refused when the store cannot be located (ST-01), the
/// session id is one the store refuses (D10), or the folder's config is broken.
fn environment(g: Globals) -> Result<Env> {
    let cwd = working_dir(g.dir)?;
    let session = g.session.map(|s| (s, "--session"));
    let env = Env::discover(&cwd, session, g.store, Vars::from_process());
    env.store_path()?;
    if let Some(e) = &env.session_error {
        bail!("{e}");
    }
    // Only hooks run on the defaults when the config is broken: a command would drop its
    // settings, the ledger's place among them, without a word.
    if let ConfigSource::Broken(e) = &env.config_source {
        bail!(
            "loading this folder's config: {} (fix the file by hand: `weighin init` will not \
             overwrite it)",
            Report(e)
        );
    }
    Ok(env)
}

/// `weighin: warning: <warning>` on stderr, and the exit code the command already earned.
fn warn(warning: &str, code: ExitCode) -> ExitCode {
    match writeln!(io::stderr().lock(), "weighin: warning: {warning}") {
        // With stderr gone, as in `report`, the exit code stands.
        Ok(()) | Err(_) => code,
    }
}

/// Run one command. Only `init` changes the environment: it rewrites the config.
fn dispatch(env: &mut Env, cmd: LedgerCmd, out: &mut impl Write) -> Result<()> {
    match cmd {
        LedgerCmd::Prime => Ok(write!(out, "{}", hooks::prime(env, "manual")?)?),
        LedgerCmd::Init(a) => commands::init(env, a, out),
        LedgerCmd::Open(a) => commands::open(env, &a, out),
        LedgerCmd::Show(a) => commands::show(env, &a, out),
        LedgerCmd::Context(a) => commands::context_pack(env, &a, out),
        LedgerCmd::Raise(a) => commands::raise(env, a, out),
        LedgerCmd::Batch(a) => commands::batch(env, a, out),
        LedgerCmd::Decide(a) => commands::apply_one(env, &a.into_op(), out),
        LedgerCmd::Defer(a) => commands::apply_one(env, &a.into_op(), out),
        LedgerCmd::Dismiss(a) => commands::apply_one(env, &a.into_op(), out),
        LedgerCmd::Reopen(a) => commands::apply_one(env, &a.into_op(), out),
        LedgerCmd::Digest(a) => commands::digest(env, &a, out),
        LedgerCmd::Stats(a) => commands::stats(env, &a, out),
        LedgerCmd::Trailer(a) => commands::trailer(env, &a, out),
        LedgerCmd::Decisions(a) => recall::list(
            env,
            &recall::Query {
                path: &a.path,
                line: a.line,
                json: a.json,
            },
            out,
        ),
        LedgerCmd::Scan(a) => commands::scan(env, a, out),
        LedgerCmd::Board {
            cmd: BoardCmd::Export(a),
        } => commands::board_export(env, a, out),
        LedgerCmd::Board {
            cmd: BoardCmd::Apply(a),
        } => commands::board_apply(env, &a, out),
        LedgerCmd::Journal => journal::write(&ops::stored_events(env)?, out),
        LedgerCmd::CiCheck(a) => ci::run(env, &a.base, a.json, out),
    }
}

impl DecideArgs {
    fn into_op(self) -> Op {
        Op::Decide(DecideOp {
            id: self.id,
            option: self.option,
            note: self.note,
            provenance: None,
            via: Some(self.via),
            seconds: None,
        })
    }
}

impl DeferArgs {
    fn into_op(self) -> Op {
        Op::Defer(DeferOp {
            id: self.id,
            flip: self.flip,
            trigger: self.trigger,
            resolve_by: self.resolve_by,
            default: self.default,
            owner: None,
            unqualified: false,
            reason: self
                .reason
                .unwrap_or_else(|| "deferred with a named fact".into()),
        })
    }
}

impl DismissArgs {
    fn into_op(self) -> Op {
        Op::Dismiss {
            id: self.id,
            why: self.why,
            note: self.note,
            reason: "dismissed from the cli".into(),
        }
    }
}

impl ReopenArgs {
    fn into_op(self) -> Op {
        Op::Reopen {
            id: self.id,
            note: self.note,
            reason: "reopened from the cli".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Stdout, is_closed_stdout};
    use anyhow::Context as _;
    use std::io::{self, Write as _};

    /// A writer whose reader has gone.
    struct Gone;

    impl io::Write for Gone {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }

    #[test]
    fn a_closed_stdout_anywhere_in_the_chain_is_recognised() {
        let written: anyhow::Result<()> = writeln!(Stdout(Gone), "an item").map_err(Into::into);
        let wrapped = written.context("writing the list").unwrap_err();
        assert!(is_closed_stdout(&wrapped));
        let flushed: anyhow::Result<()> = Stdout(Gone).flush().map_err(Into::into);
        assert!(is_closed_stdout(&flushed.unwrap_err()));
    }

    #[test]
    fn a_broken_pipe_elsewhere_is_a_failure() {
        let stderr: anyhow::Result<()> = writeln!(Gone, "note").map_err(Into::into);
        let wrapped = stderr.context("writing the note").unwrap_err();
        assert!(!is_closed_stdout(&wrapped));
        let other = anyhow::Error::from(io::Error::from(io::ErrorKind::NotFound));
        assert!(!is_closed_stdout(&other));
    }
}
