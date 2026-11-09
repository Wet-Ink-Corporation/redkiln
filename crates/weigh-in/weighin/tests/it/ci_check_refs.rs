//! `weighin ci-check` reads the journal refs a clone holds (#48): `refs/redkiln/journal/*`, the
//! inbox copies under `refs/redkiln/inbox/*` and the broken refs under `refs/redkiln/broken/*`,
//! with no fetch and no store, and checks each journal ref's whole history (JR-05).
//!
//! Each test commits a base on `main`, works on `feat`, and exports session journals with
//! `journal checkpoint` and no remote, which moves only the local ref. Bad refs are built with
//! git's plumbing.

use crate::scratch::{Repo, TestResult};
use crate::support::{bin, git};
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};
use weighin_contract::ci_check::v2::Data;

/// `git <args>` in `dir` with `stdin`, which must succeed; its stdout, trimmed.
fn git_out(dir: &Path, args: &[&str], stdin: Option<&str>) -> TestResult<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let (Some(text), Some(pipe)) = (stdin, child.stdin.as_mut()) {
        pipe.write_all(text.as_bytes())?;
    }
    drop(child.stdin.take());
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

/// A commit in `repo` whose tree holds exactly `files`, on `parents`.
fn commit_of(repo: &Repo, files: &[(&str, &str)], parents: &[&str]) -> TestResult<String> {
    let index = tempfile::tempdir()?;
    let index = index.path().join("index");
    let index_arg = index.to_string_lossy().into_owned();
    for (path, content) in files {
        let blob = git_out(&repo.root, &["hash-object", "-w", "--stdin"], Some(content))?;
        let info = format!("100644,{blob},{path}");
        let status = Command::new("git")
            .arg("-C")
            .arg(&repo.root)
            .args(["update-index", "--add", "--cacheinfo", &info])
            .env("GIT_INDEX_FILE", &index_arg)
            .status()?;
        assert!(status.success(), "update-index {info}");
    }
    let tree = if files.is_empty() {
        git_out(&repo.root, &["mktree"], Some(""))?
    } else {
        let out = Command::new("git")
            .arg("-C")
            .arg(&repo.root)
            .arg("write-tree")
            .env("GIT_INDEX_FILE", &index_arg)
            .output()?;
        String::from_utf8(out.stdout)?.trim().to_owned()
    };
    let mut args = vec!["commit-tree", tree.as_str(), "-m", "j"];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    git_out(&repo.root, &args, None)
}

/// A chain of commits of `name`'s journal file, one per content, at `refname`; their shas.
fn chain_at(repo: &Repo, refname: &str, name: &str, contents: &[&str]) -> TestResult<Vec<String>> {
    let file = format!("{name}.jsonl");
    let mut commits: Vec<String> = Vec::new();
    for content in contents {
        let parents: Vec<&str> = commits.last().map(String::as_str).into_iter().collect();
        commits.push(commit_of(repo, &[(&file, content)], &parents)?);
    }
    let tip = commits.last().ok_or("a commit")?;
    git(&repo.root, &["update-ref", refname, tip]);
    Ok(commits)
}

/// An initialised repository whose `init` is committed on `main`, checked out on `feat`.
fn on_a_branch() -> TestResult<Repo> {
    let repo = Repo::new(true)?;
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    Ok(repo)
}

/// `q` raised in `session` and exported to its journal ref; the item's id.
fn raised_in_a_ref(repo: &Repo, session: &str, q: &str) -> TestResult<String> {
    let id = repo.raise(session, q)?;
    checkpoint(repo, session)?;
    Ok(id)
}

fn checkpoint(repo: &Repo, session: &str) -> TestResult {
    repo.ok(session, &["journal", "checkpoint", "--json"], None)?;
    Ok(())
}

/// The journal file at `refname`'s tip, of journal `name`.
fn ref_file(repo: &Repo, refname: &str, name: &str) -> TestResult<String> {
    Ok(git_out(
        &repo.root,
        &["show", &format!("{refname}:{name}.jsonl")],
        None,
    )? + "\n")
}

/// The SPEC's CLI envelope schema.
fn cli_envelope_schema() -> TestResult<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/schemas/cli-envelope-1.schema.json");
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

/// `ci-check --base main --json`, which must exit 0 with one envelope line passing
/// `cli-envelope-1`; the envelope.
fn envelope(repo: &Repo) -> TestResult<Value> {
    let out = repo.ok("ci", &["ci-check", "--base", "main", "--json"], None)?;
    assert_eq!(out.matches('\n').count(), 1, "one line: {out}");
    let envelope: Value = serde_json::from_str(&out)?;
    let validator = jsonschema::draft202012::new(&cli_envelope_schema()?)?;
    let errors: Vec<String> = validator
        .iter_errors(&envelope)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{out} breaks cli-envelope-1: {errors:?}");
    Ok(envelope)
}

/// The envelope's `data`.
fn data(repo: &Repo) -> TestResult<Value> {
    Ok(envelope(repo)?
        .get_mut("data")
        .ok_or("an envelope with data")?
        .take())
}

/// The ids of `data`'s open items; none when it lists none.
fn open_ids(data: &Value) -> Vec<String> {
    data.get("open_items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// `data`'s violations; none when it lists none.
fn violations(data: &Value) -> Vec<Value> {
    data.pointer("/journal/violations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn ref_violation(refname: &str, commit: &str, line: Option<u64>, reason: &str) -> Value {
    json!({"journal": refname, "kind": "ref", "commit": commit, "line": line, "reason": reason})
}

const S1: &str = "refs/redkiln/journal/s1";

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn the_report_is_the_v2_envelope_and_reads_through_the_contract() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;

    let envelope = envelope(&repo)?;

    assert_eq!(
        (&envelope["schema"], &envelope["command"], &envelope["ok"]),
        (
            &json!("weighin.ci-check/2"),
            &json!("ci-check"),
            &json!(true)
        )
    );
    assert!(envelope["proves"].is_string(), "{envelope}");
    assert!(envelope["does_not_prove"].is_string(), "{envelope}");
    let data = Data::from_envelope(&envelope.to_string())?;
    assert_eq!(data.read.refs.len(), 1);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_item_raised_in_a_session_exported_only_to_a_ref_is_open() -> TestResult {
    let repo = on_a_branch()?;
    let id = raised_in_a_ref(&repo, "s1", "Keep three retries?")?;

    let data = data(&repo)?;

    assert_eq!(open_ids(&data), [id]);
    assert_eq!(data["read"]["files"], json!([]), "no committed journal");
    assert_eq!(data["journal"], json!({"ok": true, "violations": []}));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn read_refs_names_each_ref_with_its_kind_tip_and_commits() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    raised_in_a_ref(&repo, "s1", "Keep the cache warm?")?;
    raised_in_a_ref(&repo, "s2", "Shard the ledger?")?;

    let data = data(&repo)?;

    let s2 = "refs/redkiln/journal/s2";
    assert_eq!(
        data["read"]["refs"],
        json!([
            {"ref": S1, "kind": "journal", "tip": repo.rev(S1)?, "commits": 2},
            {"ref": s2, "kind": "journal", "tip": repo.rev(s2)?, "commits": 1},
        ])
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn with_no_journal_refs_read_refs_is_empty_and_a_warning_says_so() -> TestResult {
    let repo = on_a_branch()?;

    let envelope = envelope(&repo)?;

    assert_eq!(envelope["data"]["read"]["refs"], json!([]));
    let keys: Vec<&Value> = envelope["findings"]
        .as_array()
        .ok_or("findings")?
        .iter()
        .map(|f| &f["key"])
        .collect();
    assert_eq!(keys, [&json!("ci-check/no-journal-refs")]);
    assert_eq!(envelope["findings"][0]["severity"], json!("warning"));
    Ok(())
}

/// An item raised and decided in session `s1`, exported to its ref, with the decision atom
/// `decide` wrote taken out of the checkout, so only the ref records the decision.
fn decided_only_in_a_ref(repo: &Repo) -> TestResult<String> {
    let id = repo.raise("s1", "Keep three retries?")?;
    repo.ok("s1", &["decide", &id, "A"], None)?;
    checkpoint(repo, "s1")?;
    git(&repo.root, &["clean", "-fdxq", "--", ".kb"]);
    Ok(id)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_item_decided_in_a_ref_is_not_open() -> TestResult {
    let repo = on_a_branch()?;
    decided_only_in_a_ref(&repo)?;

    let data = data(&repo)?;

    assert_eq!(open_ids(&data), Vec::<String>::new());
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_decision_recorded_only_in_a_ref_is_not_listed_in_decisions() -> TestResult {
    let repo = on_a_branch()?;
    decided_only_in_a_ref(&repo)?;

    let data = data(&repo)?;

    assert_eq!(data["decisions"], json!([]));
    assert_eq!(data["read"]["refs"][0]["ref"], json!(S1));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_event_in_both_a_committed_journal_and_a_ref_folds_once() -> TestResult {
    let repo = on_a_branch()?;
    let id = raised_in_a_ref(&repo, "s", "Keep three retries?")?;
    // The 1.x committed journal of the same session, as `ci_check.rs` writes it.
    let journal = repo.root.join(".kb/_ledger/journal/s.jsonl");
    std::fs::create_dir_all(journal.parent().ok_or("a directory")?)?;
    std::fs::write(&journal, repo.printed_for("s")?)?;
    repo.commit("journal")?;

    let data = data(&repo)?;

    assert_eq!(open_ids(&data), [id]);
    assert_eq!(data["open_items"][0]["status"], json!("raised"));
    assert_eq!(
        data["read"]["files"],
        json!([".kb/_ledger/journal/s.jsonl"])
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ref_whose_history_drops_a_line_is_rewritten_naming_the_ref_and_commit_and_still_exits_zero()
-> TestResult {
    let repo = on_a_branch()?;
    let first = raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    raised_in_a_ref(&repo, "s1", "Keep the cache warm?")?;
    let whole = ref_file(&repo, S1, "s1")?;
    let last = whole.lines().last().ok_or("a line")?;
    let tip = repo.rev(S1)?;
    let rewritten = commit_of(&repo, &[("s1.jsonl", &format!("{last}\n"))], &[&tip])?;
    git(&repo.root, &["update-ref", S1, &rewritten]);

    let data = data(&repo)?;

    assert_eq!(
        violations(&data),
        [ref_violation(S1, &rewritten, None, "rewritten")]
    );
    assert_eq!(data["journal"]["ok"], json!(false));
    assert!(
        open_ids(&data).contains(&first),
        "the dropped line's item is still read: {data}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_merge_in_a_ref_is_a_merge_violation() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    let whole = ref_file(&repo, S1, "s1")?;
    let tip = repo.rev(S1)?;
    let side = commit_of(&repo, &[("s1.jsonl", &whole)], &[])?;
    let merged = commit_of(&repo, &[("s1.jsonl", &whole)], &[&tip, &side])?;
    git(&repo.root, &["update-ref", S1, &merged]);

    let data = data(&repo)?;

    assert_eq!(
        violations(&data),
        [ref_violation(S1, &merged, None, "merge")]
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_ref_tree_with_another_file_is_a_tree_violation() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    let whole = ref_file(&repo, S1, "s1")?;
    let tip = repo.rev(S1)?;
    let extra = commit_of(&repo, &[("s1.jsonl", &whole), ("x.txt", "x\n")], &[&tip])?;
    git(&repo.root, &["update-ref", S1, &extra]);

    let data = data(&repo)?;

    assert_eq!(violations(&data), [ref_violation(S1, &extra, None, "tree")]);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_unended_ref_file_is_an_unended_violation() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    let whole = ref_file(&repo, S1, "s1")?;
    let tip = repo.rev(S1)?;
    let unended = commit_of(
        &repo,
        &[("s1.jsonl", &format!("{whole}{{\"type\":"))],
        &[&tip],
    )?;
    git(&repo.root, &["update-ref", S1, &unended]);

    let data = data(&repo)?;

    assert_eq!(
        violations(&data),
        [ref_violation(S1, &unended, None, "unended")]
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_malformed_ref_line_is_undecodable_at_its_line_and_commit() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    let whole = ref_file(&repo, S1, "s1")?;
    let tip = repo.rev(S1)?;
    let lines = u64::try_from(whole.lines().count())?;
    let bad = commit_of(
        &repo,
        &[("s1.jsonl", &format!("{whole}{{\"not\":\"a line\"}}\n"))],
        &[&tip],
    )?;
    git(&repo.root, &["update-ref", S1, &bad]);

    let data = data(&repo)?;

    assert_eq!(
        violations(&data),
        [ref_violation(S1, &bad, Some(lines + 1), "undecodable")]
    );
    Ok(())
}

/// A JR-02 line of `event_type` at `version` naming item `wi-930e15`.
fn foreign_line(event_type: &str, version: u32) -> String {
    format!(
        r#"{{"type":"{event_type}","version":{version},"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"x","version":"2.0.0","spec":"1.0.0"}},"origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:7","data":{{"item_id":"wi-930e15"}}}}"#
    ) + "\n"
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_weighin_line_this_build_cannot_read_is_counted_by_type_not_a_violation() -> TestResult {
    let repo = on_a_branch()?;
    let newer = foreign_line("weighin.item_raised", 2);
    chain_at(&repo, S1, "s1", &[&newer, &(newer.clone() + &newer)])?;

    let envelope = envelope(&repo)?;

    let data = &envelope["data"];
    assert_eq!(data["unreadable"], json!({"weighin.item_raised": 2}));
    assert_eq!(data["journal"], json!({"ok": true, "violations": []}));
    let warning = envelope["findings"]
        .as_array()
        .ok_or("findings")?
        .iter()
        .find(|f| f["key"] == json!("ci-check/unreadable:weighin.item_raised"))
        .ok_or("an unreadable finding")?;
    assert_eq!(warning["severity"], json!("warning"));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn another_plugins_line_in_a_ref_is_passed_over() -> TestResult {
    let repo = on_a_branch()?;
    chain_at(&repo, S1, "s1", &[&foreign_line("bench.run_finished", 1)])?;

    let data = data(&repo)?;

    assert_eq!(
        (
            &data["open_items"],
            &data["unreadable"],
            &data["journal"]["ok"]
        ),
        (&json!([]), &json!({}), &json!(true))
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_inbox_copys_items_are_open_and_a_replaced_inbox_is_no_violation() -> TestResult {
    let repo = on_a_branch()?;
    let first = raised_in_a_ref(&repo, "s2", "Keep three retries?")?;
    let second = raised_in_a_ref(&repo, "s3", "Keep the cache warm?")?;
    let file = ref_file(&repo, "refs/redkiln/journal/s2", "s2")?;
    let other = ref_file(&repo, "refs/redkiln/journal/s3", "s3")?;
    // Only the inbox copy is left to read.
    git(&repo.root, &["update-ref", "-d", "refs/redkiln/journal/s2"]);
    git(&repo.root, &["update-ref", "-d", "refs/redkiln/journal/s3"]);
    let path = ".redkiln/inbox/s2.jsonl";
    let old = commit_of(&repo, &[(path, &file)], &[])?;
    // Replaced: the new file does not start with the old one's.
    let inbox = commit_of(&repo, &[(path, &format!("{other}{file}"))], &[&old])?;
    git(&repo.root, &["update-ref", "refs/redkiln/inbox/s2", &inbox]);

    let data = data(&repo)?;

    let mut open = open_ids(&data);
    open.sort();
    let mut expected = vec![first, second];
    expected.sort();
    assert_eq!(open, expected);
    assert_eq!(data["journal"], json!({"ok": true, "violations": []}));
    assert_eq!(
        data["read"]["refs"],
        json!([{"ref": "refs/redkiln/inbox/s2", "kind": "inbox", "tip": inbox, "commits": 1}])
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_broken_refs_items_are_read_and_it_is_never_checked() -> TestResult {
    let repo = on_a_branch()?;
    let id = raised_in_a_ref(&repo, "s4", "Keep three retries?")?;
    let file = ref_file(&repo, "refs/redkiln/journal/s4", "s4")?;
    let tip = repo.rev("refs/redkiln/journal/s4")?;
    git(&repo.root, &["update-ref", "-d", "refs/redkiln/journal/s4"]);
    // A merge and a stray file: a breach a person already set aside.
    let side = commit_of(&repo, &[("s4.jsonl", &file)], &[])?;
    let broken_tip = commit_of(
        &repo,
        &[("s4.jsonl", &file), ("x.txt", "x\n")],
        &[&tip, &side],
    )?;
    let broken = format!("refs/redkiln/broken/s4/{tip}");
    git(&repo.root, &["update-ref", &broken, &broken_tip]);

    let data = data(&repo)?;

    assert_eq!(open_ids(&data), [id]);
    assert_eq!(data["journal"], json!({"ok": true, "violations": []}));
    assert_eq!(data["read"]["refs"][0]["kind"], json!("broken"));
    Ok(())
}

/// The refs under `refs/redkiln`, with what each names.
fn redkiln_refs(repo: &Repo) -> TestResult<String> {
    git_out(&repo.root, &["for-each-ref", "refs/redkiln"], None)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn ci_check_never_fetches_a_journal_ref_the_remote_has() -> TestResult {
    let (repo, remote) = Repo::with_remote(true)?;
    repo.raise("s1", "Keep three retries?")?;
    // The checkpoint pushes the ref to origin; then the clone's own copy goes.
    checkpoint(&repo, "s1")?;
    let on_remote = git_out(&remote, &["for-each-ref", "refs/redkiln/journal"], None)?;
    assert!(!on_remote.is_empty(), "the remote has the ref");
    git(&repo.root, &["update-ref", "-d", S1]);
    git(&repo.root, &["branch", "-q", "feat"]);
    let before = redkiln_refs(&repo)?;

    let data = data(&repo)?;

    assert_eq!(data["read"]["refs"], json!([]));
    assert_eq!(redkiln_refs(&repo)?, before, "nothing fetched");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn ci_check_beside_journal_refs_creates_no_store_and_moves_no_ref() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    let clone = repo.sibling("clone")?.join("c");
    let (from, to) = (repo.root.to_string_lossy(), clone.to_string_lossy());
    git(&repo.root, &["clone", "-q", &from, &to]);
    git(
        &clone,
        &[
            "fetch",
            "-q",
            "origin",
            "+refs/redkiln/journal/*:refs/redkiln/journal/*",
        ],
    );
    let refs_before = git_out(&clone, &["for-each-ref"], None)?;
    let store = repo.sibling("scratch")?.join("store.sqlite3");
    let store_arg = store.to_string_lossy();

    let out = repo.run_in(
        &clone,
        &[
            "--store",
            &store_arg,
            "ci-check",
            "--base",
            "origin/main",
            "--json",
        ],
    )?;

    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(envelope["data"]["read"]["refs"][0]["ref"], json!(S1));
    let created = [
        store.exists(),
        clone.join(".git/weighin").exists() || clone.join(".git/redkiln").exists(),
    ];
    assert_eq!(created, [false, false], "no store, no state dir");
    assert_eq!(git_out(&clone, &["for-each-ref"], None)?, refs_before);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn ci_check_needs_no_locatable_store() -> TestResult {
    let repo = on_a_branch()?;
    raised_in_a_ref(&repo, "s1", "Keep three retries?")?;
    // A store path from home with no home directory: Core cannot locate it (ST-01).
    let out = Command::new(bin())
        .current_dir(&repo.root)
        .args(["ci-check", "--base", "main", "--json"])
        .env("REDKILN_STORE", "~/events.sqlite3")
        .env_remove("WEIGHIN_STORE")
        .env_remove("HOME")
        .env_remove("USERPROFILE")
        .stdin(Stdio::null())
        .output()?;

    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let envelope: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(open_ids(&envelope["data"]).len(), 1, "{envelope}");
    Ok(())
}
