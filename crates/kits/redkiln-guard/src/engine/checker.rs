//! One check of a command: every simple command it runs, in order, with what it has set so
//! far (the bytes lexed, the directory `cd` moved to, the variables assigned). What a command
//! may not do is the rule pack's ([`Pack`]); the checker only refuses what it cannot read.

use super::cwd::Cwd;
use super::feed::{Feed, own_feed};
use super::git_settings;
use super::seam::{Call, Denial, Pack};
use super::variables::{self, Variables};
use super::wrappers::{self, Program, Run};
use super::{braces, readings};
use crate::shell::{self, ParseError, SimpleCommand, Word};

/// How deep `bash -c`, `eval`, `env -S` and `find -exec` may nest before a command is refused.
pub const MAX_DEPTH: usize = 8;

/// The most MiB lexed for one command, nested `bash -c` and `eval` strings included (H-13).
pub const MAX_LEXED_MIB: usize = 4;

/// [`MAX_LEXED_MIB`] in bytes.
const MAX_LEXED_BYTES: usize = MAX_LEXED_MIB * 1024 * 1024;

/// The most commands arguments are read as in one check (H-13): each read walks the rest of
/// the arguments and may read some of them again, so the total is capped, which also bounds
/// how deeply such reads nest.
const MAX_ARGUMENT_COMMANDS: usize = 32;

/// Checks the Bash command `command`, as the guard receives it, against `pack`.
///
/// The answer is the first denial, the pack's or the engine's own (under [`Pack::UNREADABLE`]: a text the lexer cannot
/// read, nesting past [`MAX_DEPTH`], more than [`MAX_LEXED_MIB`] lexed, a brace expansion past
/// its cap), or `None` when nothing denies it. The engine never allows: `None` is undecided.
pub fn check<P: Pack>(command: &str, pack: &mut P) -> Option<Denial<P::Rule>> {
    let top_level = Scope {
        feed: &Feed::Empty,
        depth: 0,
        appended: false,
        moved: None,
    };
    Checker::new(pack).text(command, top_level)
}

/// Where a command is checked: what its standard input gets (when it does not redirect it),
/// how deeply nested it is, and whether operands are appended at run time.
#[derive(Debug, Clone, Copy)]
struct Scope<'a> {
    feed: &'a Feed<'a>,
    depth: usize,
    appended: bool,
    /// How a wrapper of this command (or of one it nests in) changed the working directory
    /// first: `env --chdir`, `sudo --chdir`, `find -execdir` ([`Call::moved`]).
    moved: Option<&'static str>,
}

impl Scope<'_> {
    /// One level deeper, reading the same input.
    fn nested(self) -> Self {
        Self {
            depth: self.depth.saturating_add(1),
            appended: false,
            ..self
        }
    }
}

/// Where the walk stands, as a pack's [`Call`] sees it: where relative paths start, and how
/// deep the check is inside [`Checker::arguments`]. Kept apart from the pack so a call can
/// borrow it while the pack is asked.
#[derive(Debug)]
struct Position {
    cwd: Cwd,
    argument_depth: usize,
}

impl Position {
    /// The call of `program <args>` in `scope`, for the pack.
    fn call<'a>(&'a self, program: Program<'a>, args: &'a [Word], scope: Scope<'_>) -> Call<'a> {
        Call {
            program,
            args,
            appended: scope.appended,
            moved: scope.moved,
            in_arguments: self.argument_depth > 0,
            cwd: &self.cwd,
        }
    }
}

/// The state of one check: the pack, the bytes lexed so far, where the walk stands and the
/// variables set.
#[derive(Debug)]
struct Checker<'p, P> {
    pack: &'p mut P,
    lexed: usize,
    /// How many times arguments have been read as a command ([`Checker::arguments`]).
    argument_commands: usize,
    position: Position,
    variables: Variables,
}

impl<'p, P: Pack> Checker<'p, P> {
    fn new(pack: &'p mut P) -> Self {
        Self {
            pack,
            lexed: 0,
            argument_commands: 0,
            position: Position {
                cwd: Cwd::Start,
                argument_depth: 0,
            },
            variables: Variables::default(),
        }
    }

    /// A denial for a command the checker cannot read.
    fn unreadable(reason: String) -> Denial<P::Rule> {
        Denial::new(P::UNREADABLE, reason)
    }

    /// Checks shell text.
    fn text(&mut self, text: &str, scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        self.lexed_by(text, scope, shell::parse)
    }

    /// Checks text read as `parse` lexes it.
    fn lexed_by(
        &mut self,
        text: &str,
        scope: Scope<'_>,
        parse: fn(&str) -> Result<Vec<SimpleCommand>, ParseError>,
    ) -> Option<Denial<P::Rule>> {
        if scope.depth > MAX_DEPTH {
            return Some(Self::unreadable(format!(
                "the command nests bash -c, eval or find -exec deeper than {MAX_DEPTH}"
            )));
        }
        self.lexed = self.lexed.saturating_add(text.len());
        if self.lexed > MAX_LEXED_BYTES {
            return Some(Self::unreadable(format!(
                "the command re-reads more than {MAX_LEXED_MIB} MiB through bash -c or eval"
            )));
        }
        let commands = match parse(text) {
            Ok(commands) => commands,
            Err(error) => {
                return Some(Self::unreadable(format!(
                    "the command cannot be read: {error}"
                )));
            }
        };
        commands.iter().find_map(|command| {
            self.variables.record(command);
            let own = own_feed(command, &commands);
            let feed = own.as_ref().unwrap_or(scope.feed);
            self.command(command, Scope { feed, ..scope })
        })
    }

    fn command(&mut self, command: &SimpleCommand, scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        let writes = self.variables.substitute(command.writes());
        let words = self.variables.substitute(command.words());
        let outer = match self.pack.enter_command(command, &words) {
            Ok(outer) => outer,
            Err(denial) => return Some(denial),
        };
        let denial = writes
            .iter()
            .flat_map(readings::of)
            .find_map(|target| self.pack.redirect(&target, &self.position.cwd))
            .or_else(|| self.words(&words, scope));
        self.pack.leave_command(&words, outer);
        denial
    }

    /// Checks one simple command's words brace-expanded, as the shell expands them before any
    /// program, wrapper or shell reads its options (`bash {-c,'rm -rf /'}` runs
    /// `bash -c 'rm -rf /'`). Brace expansion reads quotes as bash does ([`braces`]), so the
    /// expanded words are the ones that run and one reading is enough. A program word that
    /// holds a group once the words as written are unwrapped (`{rm,-rf,/}`,
    /// `sudo {sh,-c,'rm -rf /'}`) is denied as unreadable, as it was before the expanded
    /// reading.
    fn words(&mut self, words: &[Word], scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        if let Some(denial) = self.pack.words(words) {
            return Some(denial);
        }
        if !words.iter().any(braces::has_group) {
            return self.unwrapped(words, scope);
        }
        if let Run::Command {
            program: Program::Unknown(word),
            ..
        } = wrappers::unwrap(words)
            && braces::has_group(word)
        {
            return Some(Self::unreadable(format!(
                "the program {} is built by brace expansion; write the command out",
                word.text
            )));
        }
        let expanded = match braces::expand(words) {
            Ok(expanded) => expanded,
            Err(error) => return Some(Self::unreadable(error.to_string())),
        };
        self.unwrapped(&expanded, scope)
    }

    /// Checks one reading of a simple command's words: its wrappers removed, what it runs.
    /// An outer wrapper's change of directory is kept ([`wrappers::chdir`]).
    fn unwrapped(&mut self, words: &[Word], scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        let scope = Scope {
            moved: scope.moved.or_else(|| wrappers::chdir(words)),
            ..scope
        };
        self.run(wrappers::unwrap(words), scope)
    }

    /// Checks what a simple command runs once its wrappers are removed.
    fn run(&mut self, run: Run<'_>, scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        match run {
            Run::Nothing => None,
            Run::Either(runs) => runs.into_iter().find_map(|run| self.run(run, scope)),
            Run::Script(text) => self.text(&text, scope.nested()),
            Run::Split(text) => self.lexed_by(&text, scope.nested(), shell::parse_without_extglob),
            Run::StdinScript(shell) => self.stdin_script(shell, scope),
            Run::Unseen(shell) => Some(Self::unreadable(format!(
                "{shell} runs a script from a process substitution, known only at run time; \
                 pass the script as text the guard can read, as in bash -c '...'"
            ))),
            Run::UnsureOption { wrapper, word } => Some(Self::unreadable(format!(
                "{wrapper} reads its options where {} stands, a pattern that may expand to \
                 one; write the option out",
                word.text
            ))),
            Run::Exec(actions) => actions.into_iter().find_map(|action| {
                let appended = Scope {
                    appended: true,
                    ..scope.nested()
                };
                self.words(action, appended)
            }),
            Run::Command {
                program,
                args,
                appended,
            } => {
                let scope = Scope {
                    appended: scope.appended || appended,
                    ..scope
                };
                match scope.feed {
                    Feed::Static(input) if appended => {
                        // `xargs` appends what a static `echo` pipes into it; the new argument
                        // list owns its words. Quotes are dropped, as xargs drops them.
                        let input = input.replace(['\'', '"', '\\'], "");
                        let line: Vec<Word> = args
                            .iter()
                            .cloned()
                            .chain(input.split_whitespace().map(variables::literal))
                            .collect();
                        self.program(program, &line, scope)
                    }
                    _ => self.program(program, args, scope),
                }
            }
        }
    }

    /// Checks the script `shell` reads from its standard input.
    fn stdin_script(&mut self, shell: &str, scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        match scope.feed {
            Feed::Empty | Feed::File => None,
            Feed::Static(script) => {
                let empty = Feed::Empty;
                self.text(
                    script,
                    Scope {
                        feed: &empty,
                        ..scope.nested()
                    },
                )
            }
            Feed::Unknown => Some(Self::unreadable(format!(
                "{shell} reads its script from standard input, which is known only at run \
                 time; pass the script as text the guard can read, as in bash -c '...'"
            ))),
        }
    }

    /// Asks the pack about `program`, to the arguments as written and, when a word holds an
    /// expansion, again in their emptied reading ([`readings::arguments`]). `cd` is followed
    /// once: the first reading already sends it anywhere, and following the second too would
    /// move it from anywhere to one directory (`cd "$d"/x*` to `/x*`), so relative writes after
    /// it would no longer be judged as possibly protected. The second reading reads the same
    /// calls again, so the argument commands it counts are dropped (each is counted once; it
    /// still stops at the budget while it runs, and reads no more than the first), and the pack
    /// drops what it counted twice ([`Pack::reread`]).
    fn program(
        &mut self,
        program: Program<'_>,
        args: &[Word],
        scope: Scope<'_>,
    ) -> Option<Denial<P::Rule>> {
        let first = self.pack.mark();
        let denial = self.reading(program, args, scope);
        if denial.is_some() || matches!(program, Program::Named("cd" | "pushd")) {
            return denial;
        }
        let emptied = readings::arguments(args)?;
        let second = self.pack.mark();
        let commands = self.argument_commands;
        let denial = self.reading(program, &emptied, scope);
        self.pack.reread(first, second);
        self.argument_commands = commands;
        denial
    }

    /// Asks the pack about `program` with one reading of its arguments, brace-expanded: a
    /// no-op for the words [`Checker::words`] expanded, kept so that no reading a pack judges
    /// holds a group. git's settings that run a command ([`git_settings`]) are read first, and
    /// `cd`, `pushd` and `popd` are followed; a program the pack does not read itself and that
    /// may run its arguments has them read as a command ([`Checker::arguments`]).
    fn reading(
        &mut self,
        program: Program<'_>,
        args: &[Word],
        scope: Scope<'_>,
    ) -> Option<Denial<P::Rule>> {
        let expanded = match braces::expand(args) {
            Ok(expanded) => expanded,
            Err(error) => return Some(Self::unreadable(error.to_string())),
        };
        let args = expanded.as_ref();
        let call = self.position.call(program, args, scope);
        if let Some(denial) = self.pack.read(&call) {
            return Some(denial);
        }
        match program {
            Program::Named("git") => {
                let scripts = git_settings::settings(args).scripts;
                scripts
                    .into_iter()
                    .find_map(|script| self.text(script, scope.nested()))
                    .or_else(|| {
                        let call = self.position.call(program, args, scope);
                        self.pack.command(&call)
                    })
            }
            Program::Named("cd") => self.change_directory("cd", Some(args)),
            Program::Named("pushd") => self.change_directory("pushd", Some(args)),
            Program::Named("popd") => self.change_directory("popd", None),
            Program::Named(name) => self.pack.command(&call).or_else(|| {
                (!self.pack.reads_arguments(name) && wrappers::may_run_arguments(name))
                    .then(|| self.arguments(args, scope))
                    .flatten()
            }),
            Program::Unknown(word) => self.unknown(word, args, scope),
        }
    }

    /// Follows `cd` or `pushd <args>`, or `popd` (`None`: back to a directory the checker
    /// never saw, so relative paths may start anywhere), and tells the pack.
    fn change_directory(
        &mut self,
        how: &'static str,
        args: Option<&[Word]>,
    ) -> Option<Denial<P::Rule>> {
        match args {
            Some(args) => self.position.cwd.change(args),
            None => self.position.cwd = Cwd::Unknown,
        }
        self.pack.changed_directory(how);
        None
    }

    /// Whether `word` names a program whose arguments are read as a command: one the checker
    /// checks, a shell or a wrapper, or one the pack watches ([`Pack::watches`]).
    fn names_checked(&self, word: &Word) -> bool {
        wrappers::names_checked(word)
            || wrappers::names_program_where(word, |name| self.pack.watches(name))
    }

    /// The arguments of a program the checker does not know, read as a command from each word
    /// that names a program it checks or unwraps: an unlisted wrapper (`caffeinate`,
    /// `runuser -u root --`, `chroot /`) runs them. At most [`MAX_ARGUMENT_COMMANDS`] are read
    /// in one check; past that it denies. The arguments come brace-expanded
    /// ([`Checker::words`]), and a name brace expansion built (`caffeinate {rm,-rf,/}`) reads
    /// as written, so `rm -rf /` is judged and `brew install {git,jq}` is allowed as
    /// `brew install git jq` is.
    fn arguments(&mut self, args: &[Word], scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        for (at, word) in args.iter().enumerate() {
            if !self.names_checked(word) {
                continue;
            }
            self.argument_commands = self.argument_commands.saturating_add(1);
            if self.argument_commands > MAX_ARGUMENT_COMMANDS {
                return Some(Self::unreadable(format!(
                    "arguments name more than {MAX_ARGUMENT_COMMANDS} commands the guard \
                     checks; run them one by one"
                )));
            }
            let Some(command) = args.get(at..) else {
                continue;
            };
            let depth = self.position.argument_depth;
            self.position.argument_depth = depth.saturating_add(1);
            let denial = self.words(command, scope);
            self.position.argument_depth = depth;
            if denial.is_some() {
                return denial;
            }
        }
        None
    }

    /// A program word that is not a literal name (never one that holds a brace group, which
    /// [`Checker::words`] denies): read as the words its literal parts split into
    /// (`rm${IFS}-rf`), its arguments judged by the pack ([`Pack::command`]) and read as a
    /// command ([`Checker::arguments`]), and then denied when it is a command's output or a
    /// variable this command set to a value known only at run time.
    fn unknown(&mut self, word: &Word, args: &[Word], scope: Scope<'_>) -> Option<Denial<P::Rule>> {
        let fields = variables::fields(word);
        if !fields.is_empty() {
            // The split words and the arguments form a new command line, which owns them.
            let line: Vec<Word> = fields.into_iter().chain(args.iter().cloned()).collect();
            if let Some(denial) = self.words(&line, scope) {
                return Some(denial);
            }
        }
        let call = self.position.call(Program::Unknown(word), args, scope);
        if let Some(denial) = self.pack.command(&call) {
            return Some(denial);
        }
        if let Some(denial) = self.arguments(args, scope) {
            return Some(denial);
        }
        if let Some(source) = variables::runtime_source(word) {
            return Some(Self::unreadable(format!(
                "the program {} is {source}, known only at run time; write the command out",
                word.text
            )));
        }
        let name = self.variables.set_here(word)?;
        Some(Self::unreadable(format!(
            "the program comes from ${name}, which this command sets to a value known only at \
             run time; write the command out"
        )))
    }
}
