//! Inline interpreter code (#117): `python -c CODE` and `-m MODULE`, `node -e`/`--eval`/`-p`/
//! `--print`, `perl -e`/`-E` and `ruby -e`. The guard cannot read that code as commands, so
//! when any argument, the code included, names a config folder or a protected file (as a
//! path, or as a word of a path inside the code: [`ProtectedPaths::named_in`]), the call is
//! denied. Code that builds a name at run time (`'.cl' + 'aude'`) and a script file
//! (`python3 script.py`) are limits: the guard is a tripwire (guide TS-1). So is code fed on
//! stdin (`python3 - <<'EOF'`, `python3 < x.py`, `perl <<EOF`): only `-c`/`-e`-style arguments
//! are read, and a heredoc's body is data (#135). `sh -c` and `bash -c` are not here: the
//! engine reads their code as commands, and a shell's static stdin script (`sh -s <<'EOF'`) too.
//!
//! [`ProtectedPaths::named_in`]: redkiln_guard::protect::ProtectedPaths::named_in

use super::{CONFIG, creates_protected, deny};
use crate::hook::guard::Denial;
use redkiln_guard::engine::Cwd;
use redkiln_guard::engine::wrappers::{is_option, skip_one};
use redkiln_guard::shell::Word;

/// How an interpreter's command line gives it inline code.
#[derive(Debug)]
struct Interpreter {
    /// Short options whose value is the code (or, for `python -m`, the program to run).
    inline: &'static [char],
    /// Long options whose value is the code.
    long_inline: &'static [&'static str],
    /// Other short options that take a value: the rest of the cluster, else the next word.
    values: &'static [char],
    /// Short options that take the rest of the cluster as their value, and never the next word.
    /// One that takes only digits after it (`perl -l`, `-0`) is not one: the digits name no
    /// option, so the cluster reads on past them (`perl -lne`, `-0777e`).
    attached: &'static [char],
    /// Long options that take the next word as a value when written without `=`. Any other
    /// long option without `=` is read as taking a next word that is no option too.
    long_values: &'static [&'static str],
}

const PYTHON: Interpreter = Interpreter {
    inline: &['c', 'm'],
    long_inline: &[],
    values: &['W', 'X'],
    attached: &[],
    long_values: &["--check-hash-based-pycs"],
};

const NODE: Interpreter = Interpreter {
    inline: &['e', 'p'],
    long_inline: &["--eval", "--print"],
    values: &['r', 'C'],
    attached: &[],
    long_values: &[
        "--require",
        "--import",
        "--loader",
        "--experimental-loader",
        "--conditions",
        "--input-type",
        "--title",
        "--env-file",
    ],
};

const PERL: Interpreter = Interpreter {
    inline: &['e', 'E'],
    long_inline: &[],
    // `-I DIR` takes the next word when nothing follows it in the cluster.
    values: &['I'],
    attached: &['i', 'M', 'm', 'x', 'C', 'd', 'D', 'V'],
    long_values: &[],
};

const RUBY: Interpreter = Interpreter {
    inline: &['e'],
    long_inline: &[],
    values: &['C', 'E', 'F', 'I', 'r'],
    attached: &['i', 'x', 'W', 'T', 'K'],
    long_values: &[],
};

/// The interpreter `program` names, if it is one this entry reads: `python`, `python3`,
/// `python3.12`, `node`, `nodejs`, `perl`, `perl5.36`, `ruby`.
fn interpreter(program: &str) -> Option<&'static Interpreter> {
    let versioned = |name: &str| {
        program
            .strip_prefix(name)
            .is_some_and(|version| version.chars().all(|c| c.is_ascii_digit() || c == '.'))
    };
    if versioned("python") {
        Some(&PYTHON)
    } else if versioned("perl") {
        Some(&PERL)
    } else {
        match program {
            "node" | "nodejs" => Some(&NODE),
            "ruby" => Some(&RUBY),
            _ => None,
        }
    }
}

/// What reading one short-option cluster found.
enum Read {
    /// An inline-code option.
    Inline,
    /// No inline code; whether the next word is the last option's value.
    Options { takes_next: bool },
}

impl Interpreter {
    /// Whether `args` give inline code: an inline option among the options before the first
    /// operand (a script file, whose own arguments follow) or `--`.
    fn runs_inline(&self, args: &[Word]) -> bool {
        let mut rest = args;
        while let [first, tail @ ..] = rest {
            rest = tail;
            let text = first.text();
            if text == "--" || !is_option(first) {
                return false;
            }
            if text.starts_with("--") {
                let name = text.split_once('=').map_or(text, |(name, _)| name);
                if self.long_inline.contains(&name) {
                    return true;
                }
                if text.contains('=') {
                    continue;
                }
                // A listed option takes the next word. An unlisted one may too, so a next
                // word that is no option is read as its value rather than a script file,
                // and the scan reads on (fail closed: `node --disable-warning X -e ...`).
                let unlisted_value = tail.first().is_some_and(|next| !is_option(next));
                if self.long_values.contains(&name) || unlisted_value {
                    rest = skip_one(rest);
                }
                continue;
            }
            match self.cluster(text) {
                Read::Inline => return true,
                Read::Options { takes_next: true } => rest = skip_one(rest),
                Read::Options { takes_next: false } => {}
            }
        }
        false
    }

    /// Reads the short-option cluster `cluster`, `-` included.
    fn cluster(&self, cluster: &str) -> Read {
        for (at, flag) in cluster.char_indices().skip(1) {
            if self.inline.contains(&flag) {
                return Read::Inline;
            }
            if self.attached.contains(&flag) {
                return Read::Options { takes_next: false };
            }
            if self.values.contains(&flag) {
                let last = at.saturating_add(flag.len_utf8()) == cluster.len();
                return Read::Options { takes_next: last };
            }
        }
        Read::Options { takes_next: false }
    }
}

/// `program` running inline code (see the module docs) with an argument that names protected
/// config.
pub(super) fn check(program: &str, args: &[Word], cwd: &Cwd) -> Option<Denial> {
    if !interpreter(program)?.runs_inline(args) {
        return None;
    }
    args.iter().find_map(|arg| {
        let label = CONFIG
            .named_in(arg.text())
            .or_else(|| creates_protected(arg.text(), arg.shape(), cwd))?;
        Some(deny(
            &format!("{program} runs inline code, which may write what it names"),
            label,
        ))
    })
}
