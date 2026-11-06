//! The store is redkiln-core's (#17 part 5a): a 1.x ledger at `.git/weighin/ledger.sqlite3` is
//! named, never opened or changed (decisions D1 and D6, ST-15, ST-16); a store path that names
//! one is refused with the import hint; a read never creates the store (B-01); a repository git
//! refuses stops a command and not a hook (ST-01); and a written event's worktree id is the
//! same in the store and in its journal line (ID-06).

use crate::scratch::{Repo, TestResult};
use crate::support::{MESSAGE, Run, store_of, weighin_env};
use redkiln_core::ids::{Host, WorktreeId};
use redkiln_core::store::{Filter, Store, Wait};
use serde_json::{Value, json};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

/// The 1.x ledgers Core keeps as fixtures, one per journal mode and layout 1.x wrote.
const FIXTURES: [&str; 3] = [
    "weighin-0.1.x.sqlite3",
    "weighin-0.2.2.sqlite3",
    "weighin-1.2.0.sqlite3",
];

/// The item each fixture leaves open (the other is dismissed).
const OPEN_IN_FIXTURE: &str = "wi-930e15";

/// The hint a store path naming an old ledger is refused with.
const IMPORT_HINT: &str = "imports a 1.x ledger rather than opening it";

/// The bytes of Core's fixture `name`.
fn fixture(name: &str) -> TestResult<Vec<u8>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../core/redkiln-core/tests/fixtures")
        .join(name);
    Ok(std::fs::read(path)?)
}

/// A repository whose only ledger is fixture `name`, at 1.x's default place; that path.
fn with_old_ledger(name: &str) -> TestResult<(Repo, PathBuf)> {
    let repo = Repo::new(false)?;
    let path = repo.root.join(".git/weighin/ledger.sqlite3");
    std::fs::create_dir_all(repo.root.join(".git/weighin"))?;
    std::fs::write(&path, fixture(name)?)?;
    Ok((repo, path))
}

/// The sorted names in `file`'s directory, and a digest of its bytes, to show neither changed.
fn snapshot(file: &Path) -> TestResult<(Vec<String>, redkiln_core::ids::Digest)> {
    let dir = file.parent().ok_or("a ledger is in a directory")?;
    Ok((
        names_in(dir)?,
        redkiln_core::ids::Digest::sha256(&std::fs::read(file)?),
    ))
}

/// The sorted names in `dir`.
fn names_in(dir: &Path) -> TestResult<Vec<String>> {
    let mut names = std::fs::read_dir(dir)?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<std::io::Result<Vec<_>>>()?;
    names.sort();
    Ok(names)
}

/// The `weighin: warning:` lines of `stderr`.
fn warnings(stderr: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stderr)
        .lines()
        .filter(|l| l.starts_with("weighin: warning:"))
        .map(str::to_owned)
        .collect()
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_raise_beside_an_old_ledger_lands_in_the_store_and_leaves_the_ledger_as_it_was() -> TestResult {
    for name in FIXTURES {
        let (repo, old) = with_old_ledger(name)?;
        let before = snapshot(&old)?;

        let out = repo.run("s1", &["raise", "--q", "Keep retention per series?"], None)?;

        assert!(
            out.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(store_of(&repo.root).exists(), "{name}: the store was made");
        assert_eq!(
            snapshot(&old)?,
            before,
            "{name}: the old ledger is unchanged"
        );
        let warned = warnings(&out.stderr);
        assert_eq!(warned.len(), 1, "{name}: {warned:?}");
        assert!(
            warned.iter().all(|w| w.contains("1.x ledger is at")
                && w.contains("ledger.sqlite3")
                && w.contains("imported")),
            "{name}: {warned:?}"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn session_start_names_the_old_ledger_and_open_lists_none_of_its_items() -> TestResult {
    for name in FIXTURES {
        let (repo, old) = with_old_ledger(name)?;
        let before = snapshot(&old)?;
        let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo.root, "source": "startup"});

        let started = repo.hook(&start)?;
        let open = repo.run("s1", &["open"], None)?;

        assert!(started.status.success(), "{name}: the hook exits 0");
        let started: Value = serde_json::from_slice(&started.stdout)?;
        let message = started["systemMessage"]
            .as_str()
            .ok_or("SessionStart has a message for the user")?;
        assert!(
            message.contains("1.x ledger is at") && message.contains("ledger.sqlite3"),
            "{name}: {message}"
        );
        let listed = String::from_utf8(open.stdout)?;
        assert!(open.status.success(), "{name}: {listed}");
        assert!(
            listed.contains("0 open in repo") && !listed.contains(OPEN_IN_FIXTURE),
            "{name}: {listed}"
        );
        assert_eq!(
            snapshot(&old)?,
            before,
            "{name}: the old ledger is unchanged"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_store_path_naming_an_old_ledger_is_refused_with_the_import_hint() -> TestResult {
    for name in FIXTURES {
        let (repo, old) = with_old_ledger(name)?;
        let before = snapshot(&old)?;
        let named = old.to_string_lossy().into_owned();
        let raise = ["raise", "--q", "Keep retention per series?"];
        let by_flag = [&["--store", named.as_str()][..], &raise[..]].concat();
        let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo.root, "source": "startup"})
            .to_string();

        for (args, vars) in [
            (&by_flag[..], &[][..]),
            (&raise[..], &[("REDKILN_STORE", named.as_str())][..]),
            (&["open"][..], &[("REDKILN_STORE", named.as_str())][..]),
        ] {
            let (ok, out) = weighin_env(
                &repo.root,
                &Run {
                    session: "s1",
                    args,
                    stdin: None,
                    vars,
                },
            );
            assert!(!ok && out.contains(IMPORT_HINT), "{name} {args:?}: {out}");
        }
        assert_eq!(
            snapshot(&old)?,
            before,
            "{name}: the old ledger is unchanged"
        );
        let (ok, out) = weighin_env(
            &repo.root,
            &Run {
                session: "s1",
                args: &["hook"],
                stdin: Some(&start),
                vars: &[("REDKILN_STORE", named.as_str())],
            },
        );
        assert!(ok, "{name}: a hook fails open: {out}");
        // The hook logs the refusal in weighin's state dir beside the store it was given (D4),
        // its one channel back to the user; the ledger and its SQLite files stay as they were.
        let (mut beside, digest) = snapshot(&old)?;
        beside.retain(|n| n != "weighin");
        assert_eq!(
            (beside, digest),
            before,
            "{name}: the old ledger is unchanged"
        );
        assert!(
            out.contains("hook error") && old.with_file_name("weighin/hook-errors.log").exists(),
            "{name}: the refusal is reported: {out}"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_read_beside_only_an_old_ledger_creates_no_store() -> TestResult {
    for name in FIXTURES {
        let (repo, _old) = with_old_ledger(name)?;

        for args in [&["open"][..], &["digest"][..], &["journal"][..]] {
            let out = repo.run("s1", args, None)?;
            assert!(
                out.status.success(),
                "{name} {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        assert!(
            !repo.root.join(".git/redkiln").exists(),
            "{name}: a read created the store's directory"
        );
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_written_events_worktree_is_its_toplevels_id_in_the_store_and_the_journal() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s1", "Keep retention per series?")?;
    let toplevel = std::process::Command::new("git")
        .arg("-C")
        .arg(&repo.root)
        .args(["rev-parse", "--show-toplevel"])
        .output()?;
    let toplevel = String::from_utf8(toplevel.stdout)?;
    let expected = WorktreeId::of_toplevel(Path::new(toplevel.trim_end()), Host::current())?;

    let store = Store::open(&store_of(&repo.root), Wait::Command)?;
    let read = store.read(&Filter::all(), 0, NonZeroUsize::MIN.saturating_add(9))?;
    // Closed before the binary runs again, as Windows wants of an open file.
    drop(store);
    let stored = read
        .iter()
        .map(|event| {
            let envelope: Value = serde_json::from_slice(&event.data)?;
            Ok(envelope["worktree"].as_str().map(str::to_owned))
        })
        .collect::<TestResult<Vec<_>>>()?;
    let written = String::from_utf8(repo.journal("s1")?)?;
    let journal = written
        .lines()
        .map(|l| {
            Ok(serde_json::from_str::<Value>(l)?["worktree"]
                .as_str()
                .map(str::to_owned))
        })
        .collect::<TestResult<Vec<_>>>()?;

    let id = [Some(expected.as_str())];
    assert_eq!(
        stored.iter().map(Option::as_deref).collect::<Vec<_>>(),
        id,
        "the store's worktree"
    );
    assert_eq!(
        journal.iter().map(Option::as_deref).collect::<Vec<_>>(),
        id,
        "the journal's worktree"
    );
    // The line rebuilt from the store is the line written, so a reader of both counts it once.
    assert_eq!(repo.printed_for("s1")?, written);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_repository_git_refuses_stops_a_command_and_not_a_hook_and_nothing_is_written() -> TestResult {
    let repo = Repo::new(false)?;
    let broken = repo.sibling("broken")?;
    std::fs::write(broken.join(".git"), "gitdir: /nowhere/at/all\n")?;
    let stop = json!({"hook_event_name": "Stop", "session_id": "s1", "cwd": broken, "last_assistant_message": MESSAGE})
        .to_string();

    let command = repo.run_in(&broken, &["--session", "s1", "raise", "--q", "Why?"])?;
    let (hooked, out) = weighin_env(
        &broken,
        &Run {
            session: "s1",
            args: &["hook"],
            stdin: Some(&stop),
            vars: &[],
        },
    );

    let refused = String::from_utf8_lossy(&command.stderr);
    assert!(
        !command.status.success() && refused.contains("git"),
        "{refused}"
    );
    assert!(hooked, "a hook fails open: {out}");
    assert_eq!(
        names_in(&broken)?,
        [".git"],
        "nothing was written beside the broken .git"
    );
    Ok(())
}
