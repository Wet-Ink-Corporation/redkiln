//! `Pack::script_file`: the engine tells a pack about every script file a shell (or `source`)
//! runs, in each reading it follows, and never about a script it reads itself.

use redkiln_guard::engine::{self, Call, Cwd, Denial, Pack, ScriptFile};
use redkiln_guard::shell::{SimpleCommand, Word};

/// One `script_file` call as the pack saw it: the runner, the script word and its argument
/// words as text, and whether arguments are appended at run time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Seen {
    runner: String,
    script: String,
    args: Vec<String>,
    appended: bool,
}

/// A pack that denies nothing and records every script file the engine reports.
#[derive(Debug, Default)]
struct Recorder {
    seen: Vec<Seen>,
}

impl Pack for Recorder {
    type Rule = ();
    type Mark = ();
    const UNREADABLE: () = ();

    fn enter_command(
        &mut self,
        _command: &SimpleCommand,
        _words: &[Word],
    ) -> Result<(), Denial<()>> {
        Ok(())
    }

    fn leave_command(&mut self, _words: &[Word], (): ()) {}

    fn redirect(&mut self, _target: &Word, _cwd: &Cwd) -> Option<Denial<()>> {
        None
    }

    fn script_file(&mut self, file: &ScriptFile<'_>) -> Option<Denial<()>> {
        self.seen.push(Seen {
            runner: file.runner.to_owned(),
            script: file.script.text().to_owned(),
            args: file
                .args
                .iter()
                .map(|word| word.text().to_owned())
                .collect(),
            appended: file.appended,
        });
        None
    }

    fn command(&mut self, _call: &Call<'_>) -> Option<Denial<()>> {
        None
    }

    fn mark(&self) {}
}

/// The script files the engine reports for `command`, and whether it denied it.
fn seen(command: &str) -> (Vec<Seen>, bool) {
    let mut pack = Recorder::default();
    let denied = engine::check(command, &mut pack).is_some();
    (pack.seen, denied)
}

fn file(runner: &str, script: &str, args: &[&str], appended: bool) -> Seen {
    Seen {
        runner: runner.to_owned(),
        script: script.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        appended,
    }
}

#[test]
fn a_script_file_is_reported_with_its_arguments_behind_every_wrapper() {
    let rows = [
        ("sh x/run a b", file("sh", "x/run", &["a", "b"], false)),
        ("sudo sh x/run a", file("sh", "x/run", &["a"], false)),
        (
            "sudo -u root bash -e x/run a",
            file("bash", "x/run", &["a"], false),
        ),
        ("env -i sh x/run a", file("sh", "x/run", &["a"], false)),
        ("exec sh x/run a", file("sh", "x/run", &["a"], false)),
        ("nice -n 5 sh x/run", file("sh", "x/run", &[], false)),
        ("timeout 5 sh x/run a", file("sh", "x/run", &["a"], false)),
        ("bash -c 'sh x/run a'", file("sh", "x/run", &["a"], false)),
        ("CI=1 dash -- x/run a", file("dash", "x/run", &["a"], false)),
        ("su root x/run a", file("su", "x/run", &["a"], false)),
        ("sh {x/run,a} b", file("sh", "x/run", &["a", "b"], false)),
        ("cat list | xargs sh x/run", file("sh", "x/run", &[], true)),
        (
            "find . -exec sh x/run {} +",
            file("sh", "x/run", &["{}"], true),
        ),
    ];
    for (command, expected) in rows {
        let (seen, denied) = seen(command);
        assert!(!denied, "{command:?} is denied");
        assert!(seen.contains(&expected), "{command:?} reported {seen:?}");
    }
}

#[test]
fn source_and_dot_report_the_file_they_run_with_its_arguments() {
    for command in ["source x/run a", ". x/run a"] {
        let (seen, _) = seen(command);
        assert_eq!(
            seen,
            vec![file("source", "x/run", &["a"], false)],
            "{command:?}"
        );
    }
}

#[test]
fn no_script_file_is_reported_for_a_script_the_engine_reads_itself() {
    for command in [
        "sh -c 'echo hi' x/run",
        "sh -s x/run",
        "sh < x/run",
        "sh /dev/stdin a",
        "echo hi",
        "bash --version x/run",
    ] {
        let (seen, _) = seen(command);
        assert_eq!(seen, Vec::new(), "{command:?}");
    }
}
