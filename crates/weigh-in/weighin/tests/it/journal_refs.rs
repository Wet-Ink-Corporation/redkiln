//! `weighin journal checkpoint|import|relay|pre-push` (SPEC JR-07 to JR-13, CLI-01 to CLI-08)
//! over real repositories: a scratch repository with a bare `origin` beside it, and clones.

use crate::scratch::{Repo, TestResult};
use crate::support::{git, store_of};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const QUESTION: &str = "Keep retention per series?";

/// git's stdout in `dir`, trimmed; the run must succeed.
fn git_out(dir: &Path, args: &[&str]) -> TestResult<String> {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output()?;
    if !out.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// The refs of `dir` under `prefix`, sorted.
fn refs(dir: &Path, prefix: &str) -> TestResult<Vec<String>> {
    let listed = git_out(dir, &["for-each-ref", "--format=%(refname)", prefix])?;
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    names.sort();
    Ok(names)
}

/// The file `path` at `rev` in `dir`.
fn file_at(dir: &Path, rev: &str, path: &str) -> TestResult<String> {
    git_out(dir, &["show", &format!("{rev}:{path}")])
}

/// The envelope a `--json` run wrote: exactly one line.
fn envelope(out: &Output) -> TestResult<Value> {
    let text = std::str::from_utf8(&out.stdout)?;
    assert_eq!(text.matches('\n').count(), 1, "one line: {text}");
    Ok(serde_json::from_str(text)?)
}

fn code(out: &Output) -> Option<i32> {
    out.status.code()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// An `own` repository with a remote, `QUESTION` raised in `s1`; the repository, the remote and
/// the new item's id.
fn raised() -> TestResult<(Repo, PathBuf, String)> {
    let (repo, remote) = Repo::with_remote(true)?;
    let id = repo.raise("s1", QUESTION)?;
    Ok((repo, remote, id))
}

/// How many events `weighin journal` prints for `session`.
fn stored_for(repo: &Repo, session: &str) -> TestResult<usize> {
    Ok(repo.printed_for(session)?.lines().count())
}

/// Makes `remote` refuse any push outside `refs/heads/*`, as Claude Code cloud's git proxy does.
#[cfg(unix)]
fn refuse_custom_refs(remote: &Path) -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let hook = remote.join("hooks/pre-receive");
    let script = "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/*) ;;\n    *) echo \"push contains a ref outside refs/heads/*\" >&2; exit 1 ;;\n  esac\ndone\n";
    std::fs::write(&hook, script)?;
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn checkpoint_pushes_each_sessions_events_to_its_journal_ref() -> TestResult {
    let (repo, remote, id) = raised()?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    let env = envelope(&out)?;
    assert_eq!(env["data"]["direct"], json!(["s1"]), "{env}");
    let journal = file_at(&remote, "refs/redkiln/journal/s1", "s1.jsonl")?;
    assert_eq!(journal.lines().count(), stored_for(&repo, "s1")?);
    assert!(
        journal.lines().all(|l| l.contains("\"origin\"")),
        "{journal}"
    );
    assert!(journal.contains(&id), "{journal}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_checkpoint_envelope_matches_its_golden() -> TestResult {
    let (repo, _remote, _id) = raised()?;
    let lines = stored_for(&repo, "s1")?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(
        String::from_utf8(out.stdout)?,
        format!(
            "{{\"schema\":\"weighin.journal-checkpoint/1\",\"ok\":true,\
             \"command\":\"journal checkpoint\",\"data\":{{\"skipped\":null,\"appended\":{lines},\
             \"direct\":[\"s1\"],\"relayed\":[],\"local_only\":false,\"unexportable\":0,\
             \"relay_reason\":null}},\"findings\":[]}}\n"
        )
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_second_checkpoint_with_nothing_new_appends_nothing() -> TestResult {
    let (repo, _remote, _id) = raised()?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    let data = &envelope(&out)?["data"];
    assert_eq!(
        (&data["appended"], &data["direct"]),
        (&json!(0), &json!([]))
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_client_repository_checkpoint_is_skipped_and_writes_no_ref() -> TestResult {
    let (repo, remote) = Repo::with_remote(false)?;
    repo.ok("s1", &["init", "--class", "client"], None)?;
    repo.raise("s1", QUESTION)?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    assert_eq!(envelope(&out)?["data"]["skipped"], "client");
    assert_eq!(refs(&repo.root, "refs/redkiln")?, Vec::<String>::new());
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn outside_git_the_checkpoint_is_skipped() -> TestResult {
    let repo = Repo::new(false)?;
    let folder = repo.sibling("plain")?;

    let out = repo.run_in(&folder, &["journal", "checkpoint", "--json"])?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    assert_eq!(envelope(&out)?["data"]["skipped"], "not-git");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn journal_false_skips_the_checkpoint() -> TestResult {
    let (repo, remote, _id) = raised()?;
    let config = repo.root.join(".kb/_ledger/config.json");
    let mut json: Value = serde_json::from_str(&std::fs::read_to_string(&config)?)?;
    json["journal"] = json!(false);
    std::fs::write(&config, json.to_string())?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(envelope(&out)?["data"]["skipped"], "opted-out");
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn with_no_store_the_checkpoint_is_skipped_and_creates_none() -> TestResult {
    let (repo, _remote) = Repo::with_remote(true)?;
    assert!(!store_of(&repo.root).exists(), "init creates no store");

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert_eq!(envelope(&out)?["data"]["skipped"], "no-store");
    assert!(!store_of(&repo.root).exists());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn with_no_remote_only_the_local_ref_moves() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s1", QUESTION)?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    let data = &envelope(&out)?["data"];
    assert_eq!(
        (&data["local_only"], &data["direct"]),
        (&json!(true), &json!(["s1"]))
    );
    assert_eq!(
        refs(&repo.root, "refs/redkiln/journal")?,
        ["refs/redkiln/journal/s1"]
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn where_the_remote_refuses_custom_refs_the_checkpoint_pushes_an_inbox_branch() -> TestResult {
    let (repo, remote, _id) = raised()?;
    refuse_custom_refs(&remote)?;

    let out = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    let data = &envelope(&out)?["data"];
    assert_eq!(
        (&data["relayed"], &data["direct"]),
        (&json!(["s1"]), &json!([]))
    );
    assert_eq!(
        refs(&remote, "refs/heads/redkiln")?,
        ["refs/heads/redkiln/inbox/s1"]
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn relay_moves_an_inbox_into_its_ref_and_deletes_the_branch() -> TestResult {
    let (repo, remote, _id) = raised()?;
    refuse_custom_refs(&remote)?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;
    std::fs::remove_file(remote.join("hooks/pre-receive"))?;
    let lines = stored_for(&repo, "s1")?;

    let out = repo.run(
        "ci",
        &["journal", "relay", "redkiln/inbox/s1", "--json"],
        None,
    )?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    assert_eq!(
        envelope(&out)?,
        json!({"schema": "weighin.journal-relay/1", "ok": true, "command": "journal relay",
            "data": {"branch": "redkiln/inbox/s1", "outcome": "merged", "added": lines,
                "deleted": "yes"},
            "findings": []})
    );
    assert_eq!(refs(&remote, "refs/heads/redkiln")?, Vec::<String>::new());
    let journal = file_at(&remote, "refs/redkiln/journal/s1", "s1.jsonl")?;
    assert_eq!(journal.lines().count(), lines);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn relay_of_a_gone_branch_exits_0_with_outcome_gone() -> TestResult {
    let (repo, _remote) = Repo::with_remote(false)?;

    let out = repo.run(
        "ci",
        &["journal", "relay", "redkiln/inbox/x", "--json"],
        None,
    )?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    let data = &envelope(&out)?["data"];
    assert_eq!(
        data,
        &json!({"branch": "redkiln/inbox/x", "outcome": "gone", "added": null, "deleted": null})
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn relay_of_a_branch_that_is_no_inbox_exits_2_with_nothing_on_stdout() -> TestResult {
    let (repo, _remote) = Repo::with_remote(false)?;

    let out = repo.run("ci", &["journal", "relay", "main", "--json"], None)?;

    assert_eq!(code(&out), Some(2));
    assert_eq!(out.stdout, b"");
    assert!(
        stderr(&out).starts_with("weighin: error: "),
        "{}",
        stderr(&out)
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn relay_leaves_an_inbox_commit_that_changes_another_path_and_exits_1() -> TestResult {
    let (repo, remote) = Repo::with_remote(false)?;
    std::fs::write(repo.root.join("docs/plan.md"), "# Changed\n")?;
    repo.commit("not an inbox commit")?;
    git(
        &repo.root,
        &["push", "-q", "origin", "HEAD:refs/heads/redkiln/inbox/s1"],
    );

    let out = repo.run(
        "ci",
        &["journal", "relay", "redkiln/inbox/s1", "--json"],
        None,
    )?;

    assert_eq!(code(&out), Some(1), "{}", stderr(&out));
    let env = envelope(&out)?;
    assert_eq!(
        (&env["ok"], &env["data"]["outcome"]),
        (&json!(false), &json!("refused"))
    );
    assert_eq!(
        env["findings"][0]["key"], "relay/refused:redkiln/inbox/s1",
        "{env}"
    );
    assert_eq!(
        refs(&remote, "refs/heads/redkiln")?,
        ["refs/heads/redkiln/inbox/s1"]
    );
    Ok(())
}

/// A clone of `remote` beside `repo`, initialised as `own`.
fn own_clone(repo: &Repo, remote: &Path) -> TestResult<PathBuf> {
    let parent = repo.sibling("clones")?;
    git(&parent, &["clone", "-q", &remote.to_string_lossy(), "b"]);
    let clone = parent.join("b");
    let out = repo.run_in(&clone, &["init", "--class", "own"])?;
    assert!(out.status.success(), "{}", stderr(&out));
    Ok(clone)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn import_reads_another_clones_journal_ref_into_the_store() -> TestResult {
    let (repo, remote, id) = raised()?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;
    let lines = stored_for(&repo, "s1")?;
    let clone = own_clone(&repo, &remote)?;

    let out = repo.run_in(&clone, &["journal", "import", "--json"])?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    let env = envelope(&out)?;
    let sources = env["data"]["sources"].as_array().ok_or("sources")?;
    assert_eq!(sources.len(), 1, "{env}");
    assert_eq!(sources[0]["source"], "refs/redkiln/journal/s1");
    assert_eq!(sources[0]["imported"], json!(lines));
    let open = repo.run_in(&clone, &["open", "--all"])?;
    assert!(String::from_utf8(open.stdout)?.contains(&id));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_second_import_reads_nothing_new() -> TestResult {
    let (repo, remote, _id) = raised()?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;
    let clone = own_clone(&repo, &remote)?;
    repo.run_in(&clone, &["journal", "import"])?;

    let out = repo.run_in(&clone, &["journal", "import", "--json"])?;

    assert_eq!(
        envelope(&out)?["data"],
        json!({"skipped": null, "sources": [], "late": false})
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn import_in_a_client_repository_is_skipped() -> TestResult {
    let (repo, _remote) = Repo::with_remote(false)?;
    repo.ok("s1", &["init", "--class", "client"], None)?;

    let out = repo.run("s1", &["journal", "import", "--json"], None)?;

    assert_eq!(envelope(&out)?["data"]["skipped"], "client");
    assert!(!store_of(&repo.root).exists(), "no store is created");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn stats_counts_an_event_once_when_it_is_in_a_committed_journal_and_imported_from_a_ref()
-> TestResult {
    let (repo, remote, _id) = raised()?;
    repo.ok("s1", &["journal", "checkpoint"], None)?;
    let expected: Value = serde_json::from_str(repo.ok("s1", &["stats", "--json"], None)?.trim())?;
    let lines = stored_for(&repo, "s1")?;
    let committed = repo.journal("s1")?;
    let clone = own_clone(&repo, &remote)?;
    let dir = clone.join(".kb/_ledger/journal");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("s1.jsonl"), committed)?;

    let import = repo.run_in(&clone, &["journal", "import", "--json"])?;
    assert_eq!(code(&import), Some(0), "{}", stderr(&import));
    let imported = envelope(&import)?;
    assert_eq!(
        imported["data"]["sources"][0]["imported"],
        json!(lines),
        "{imported}"
    );
    let out = repo.run_in(&clone, &["stats", "--json"])?;

    let got: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(got, expected);
    Ok(())
}

/// git's pre-push input line for `local` pushed to `remote_ref`.
fn push_line(remote_ref: &str) -> String {
    let sha = "1111111111111111111111111111111111111111";
    let zero = "0000000000000000000000000000000000000000";
    format!("{remote_ref} {sha} {remote_ref} {zero}\n")
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn pre_push_of_only_journal_refs_does_nothing() -> TestResult {
    let (repo, remote, _id) = raised()?;
    let input = push_line("refs/redkiln/journal/s1") + &push_line("refs/heads/redkiln/inbox/s1");

    let out = repo.run(
        "s1",
        &["journal", "pre-push", "origin", "url", "--json"],
        Some(&input),
    )?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    let env = envelope(&out)?;
    assert_eq!(
        (&env["schema"], &env["data"]["skipped"]),
        (
            &json!("weighin.journal-pre-push/1"),
            &json!("journal-only-push")
        )
    );
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

/// A journal-only push skips before the environment is looked for: a broken config says nothing.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn pre_push_of_only_journal_refs_looks_for_no_environment() -> TestResult {
    let (repo, _remote, _id) = raised()?;
    std::fs::write(repo.root.join(".kb/_ledger/config.json"), "{ not json")?;

    let out = repo.run(
        "s1",
        &["journal", "pre-push", "origin", "url", "--json"],
        Some(&push_line("refs/redkiln/journal/s1")),
    )?;

    assert_eq!(code(&out), Some(0));
    assert_eq!(stderr(&out), "");
    assert_eq!(envelope(&out)?["data"]["skipped"], "journal-only-push");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn pre_push_of_a_branch_exports_and_exits_0() -> TestResult {
    let (repo, remote, _id) = raised()?;

    let out = repo.run(
        "s1",
        &["journal", "pre-push", "origin", "url"],
        Some(&push_line("refs/heads/main")),
    )?;

    assert_eq!(code(&out), Some(0), "{}", stderr(&out));
    assert_eq!(
        refs(&remote, "refs/redkiln/journal")?,
        ["refs/redkiln/journal/s1"]
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn pre_push_with_a_broken_config_warns_and_exits_0() -> TestResult {
    let (repo, remote, _id) = raised()?;
    std::fs::write(repo.root.join(".kb/_ledger/config.json"), "{ not json")?;

    let out = repo.run(
        "s1",
        &["journal", "pre-push", "origin", "url"],
        Some(&push_line("refs/heads/main")),
    )?;

    assert_eq!(code(&out), Some(0));
    assert!(
        stderr(&out).starts_with("weighin: warning: "),
        "{}",
        stderr(&out)
    );
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_internal_error_writes_the_envelope_and_exits_70() -> TestResult {
    let repo = Repo::new(true)?;
    let bogus = repo.sibling("bogus")?.join("events.sqlite3");
    std::fs::write(&bogus, "not a database")?;
    let store = bogus.to_string_lossy().into_owned();

    let out = repo.run(
        "s1",
        &["--store", &store, "journal", "checkpoint", "--json"],
        None,
    )?;

    assert_eq!(code(&out), Some(70));
    let env = envelope(&out)?;
    assert_eq!(
        (&env["ok"], &env["data"], &env["findings"][0]["key"]),
        (&json!(false), &json!({}), &json!("internal")),
        "{env}"
    );
    assert!(
        stderr(&out).starts_with("weighin: error: "),
        "{}",
        stderr(&out)
    );
    Ok(())
}

/// A warning stderr cannot take, written before the envelope, must not make the exit disagree
/// with the envelope's `ok` (CLI-02).
#[cfg(target_os = "linux")]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_warning_stderr_cannot_take_leaves_ok_and_the_exit_agreeing() -> TestResult {
    let (repo, _remote) = Repo::with_remote(false)?;
    std::fs::create_dir_all(repo.root.join(".kb/_ledger"))?;
    std::fs::write(
        repo.root.join(".kb/_ledger/config.json"),
        r#"{"repo_class":"own"}"#,
    )?;
    repo.raise("s1", QUESTION)?;
    let said = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;
    assert!(
        stderr(&said).starts_with("weighin: warning: "),
        "the precondition: a notice goes to stderr before the envelope: {}",
        stderr(&said)
    );

    for command in ["checkpoint", "import"] {
        let out = repo.run_with_stderr_full("s1", &["journal", command, "--json"])?;

        let env = envelope(&out)?;
        assert_eq!(env["ok"], json!(true), "{command}: {env}");
        assert_eq!(code(&out), Some(0), "{command}: ok and the exit agree");
    }
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_closed_stdout_exits_0_with_nothing_on_stderr() -> TestResult {
    let (repo, _remote, _id) = raised()?;

    let out = repo.run_with_stdout_closed(&["journal", "checkpoint", "--json"])?;

    assert_eq!(code(&out), Some(0));
    assert_eq!(stderr(&out), "");
    Ok(())
}

/// A closed stdout silences a success, never a failure (CLI-05, CLI-07).
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_internal_error_with_a_closed_stdout_still_exits_70() -> TestResult {
    let repo = Repo::new(true)?;
    let bogus = repo.sibling("bogus")?.join("events.sqlite3");
    std::fs::write(&bogus, "not a database")?;
    let store = bogus.to_string_lossy().into_owned();

    let out =
        repo.run_with_stdout_closed(&["--store", &store, "journal", "checkpoint", "--json"])?;

    assert_eq!(code(&out), Some(70), "{}", stderr(&out));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn pre_push_that_cannot_open_the_store_warns_exits_0_and_keeps_its_data_shape() -> TestResult {
    let repo = Repo::new(true)?;
    let bogus = repo.sibling("bogus")?.join("events.sqlite3");
    std::fs::write(&bogus, "not a database")?;
    let store = bogus.to_string_lossy().into_owned();

    let out = repo.run(
        "s1",
        &[
            "--store", &store, "journal", "pre-push", "origin", "url", "--json",
        ],
        Some(&push_line("refs/heads/main")),
    )?;

    assert_eq!(code(&out), Some(0));
    assert!(
        stderr(&out).starts_with("weighin: warning: "),
        "{}",
        stderr(&out)
    );
    let env = envelope(&out)?;
    assert_eq!(
        (&env["ok"], &env["data"], &env["findings"][0]["key"]),
        (
            &json!(true),
            &json!({"skipped": null, "appended": 0, "direct": [], "relayed": [],
                "local_only": false, "unexportable": 0, "relay_reason": null}),
            &json!("internal")
        ),
        "{env}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn checkpoint_without_json_prints_one_summary_line() -> TestResult {
    let (repo, _remote, _id) = raised()?;
    let lines = stored_for(&repo, "s1")?;

    let out = repo.ok("s1", &["journal", "checkpoint"], None)?;

    assert_eq!(out, format!("exported {lines} line(s): pushed s1\n"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_command_that_fails_before_writing_leaves_no_store_and_no_ref() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;

    let out = repo.run("s1", &["decide", "wi-000000", "A"], None)?;
    let checkpoint = repo.run("s1", &["journal", "checkpoint", "--json"], None)?;

    assert!(!out.status.success());
    assert!(!store_of(&repo.root).exists());
    assert_eq!(envelope(&checkpoint)?["data"]["skipped"], "no-store");
    assert_eq!(refs(&remote, "refs/redkiln")?, Vec::<String>::new());
    Ok(())
}
