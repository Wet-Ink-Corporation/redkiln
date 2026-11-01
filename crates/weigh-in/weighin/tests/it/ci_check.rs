//! `weighin ci-check --base <ref>`: the branch report CI reads (B-28), from git and the checkout
//! alone. Each test commits a base on `main`, works on a branch `feat`, and commits again.

use crate::scratch::{Repo, TestResult};
use crate::support::git;
use serde_json::{Value, json};

const JOURNAL: &str = ".kb/_ledger/journal/s.jsonl";

/// An initialised repository whose `init` is committed on `main`, checked out on `feat`.
fn on_a_branch() -> TestResult<Repo> {
    let repo = Repo::new(true)?;
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    Ok(repo)
}

/// `ci-check --base main --json`, which must exit 0; the report.
fn report(repo: &Repo) -> TestResult<Value> {
    let out = repo.ok("ci", &["ci-check", "--base", "main", "--json"], None)?;
    Ok(serde_json::from_str(&out)?)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_untriaged_item_raised_on_the_branch_is_reported_open_with_nothing_recorded() -> TestResult {
    let repo = on_a_branch()?;
    let id = repo.raise("s", "Keep three retries?")?;
    let head = repo.commit("raise")?;

    let r = report(&repo)?;

    let expected = json!({
        "schema": "weighin.ci-check/1",
        "base": repo.rev("main")?,
        "head": head,
        "open_items": [{
            "id": id, "status": "raised", "kind": "question", "door": null,
            "authority": null, "blocks": "unknown", "resolve_by": null,
            "question": "Keep three retries?"
        }],
        "decisions": [],
        "journal": {"ok": true, "violations": []}
    });
    assert_eq!(r, expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_triaged_item_reports_the_triage_and_its_deadline_not_the_hints() -> TestResult {
    let repo = on_a_branch()?;
    let batch = json!([
        {"op": "raise", "q": "Drop the v1 endpoint?", "door": "two-way", "authority": "authority"},
        {"op": "triage", "id": "@1", "door": "one-way", "authority": "judgment", "blast": "external",
         "route": "present", "resolve_by": "before-merge", "reason": "clients call it"}
    ]);
    repo.ok("s", &["batch"], Some(&batch.to_string()))?;
    repo.commit("triage")?;

    let r = report(&repo)?;

    let item = &r["open_items"][0];
    let got = [
        &item["door"],
        &item["authority"],
        &item["blocks"],
        &item["resolve_by"],
    ];
    assert_eq!(
        got,
        [
            &json!("one-way"),
            &json!("judgment"),
            &json!("merge"),
            &json!("before-merge")
        ]
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_trailer_alone_closes_the_item_and_names_its_commit() -> TestResult {
    let repo = on_a_branch()?;
    let id = repo.raise("s", "Keep three retries?")?;
    repo.commit("raise")?;
    let decided = repo.commit(&format!("Settle the retries\n\nWeigh-In: {id} decided A\n"))?;

    let r = report(&repo)?;

    let expected = json!([{"id": id, "option": "A", "commit": decided, "sources": ["trailer"]}]);
    assert_eq!((&r["open_items"], &r["decisions"]), (&json!([]), &expected));
    Ok(())
}

/// A repository whose `main` holds a journal in which `q` was raised and dismissed, and
/// reopened when `reopen_at_base` is true, checked out on `feat`; the item's id.
fn dismissed_at_base(q: &str, reopen_at_base: bool) -> TestResult<(Repo, String)> {
    let repo = Repo::new(true)?;
    let id = repo.raise("s", q)?;
    repo.ok("s", &["dismiss", &id, "moot"], None)?;
    if reopen_at_base {
        repo.ok("s", &["reopen", &id], None)?;
    }
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    Ok((repo, id))
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_reopen_from_before_the_branch_does_not_outrank_its_trailer() -> TestResult {
    let (repo, id) = dismissed_at_base("Keep three retries?", true)?;
    repo.commit(&format!("Settle the retries\n\nWeigh-In: {id} decided A\n"))?;

    let r = report(&repo)?;

    assert_eq!(r["open_items"], json!([]));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_reopen_committed_after_the_trailer_keeps_the_item_open() -> TestResult {
    let (repo, id) = dismissed_at_base("Keep three retries?", false)?;
    repo.commit(&format!("Settle the retries\n\nWeigh-In: {id} decided A\n"))?;
    repo.ok("s", &["reopen", &id], None)?;
    repo.commit("reopen")?;

    let r = report(&repo)?;

    let open: Vec<&Value> = r["open_items"]
        .as_array()
        .ok_or("a list")?
        .iter()
        .map(|i| &i["id"])
        .collect();
    assert_eq!(open, [&json!(id)]);
    Ok(())
}

/// Write an accepted decision atom for `id` as `.kb/_intake/decisions/<name>`.
fn write_atom(repo: &Repo, id: &str, name: &str) -> TestResult {
    let dir = repo.root.join(".kb/_intake/decisions");
    std::fs::create_dir_all(&dir)?;
    let atom = format!(
        "---\nid: \"kb-decision-{id}\"\nkind: decision\nstatus: accepted\nweighin_item: \"{id}\"\n---\n\n# Three retries\n"
    );
    std::fs::write(dir.join(name), atom)?;
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_atom_alone_closes_the_item_and_names_the_commit_that_added_it() -> TestResult {
    let repo = on_a_branch()?;
    let id = repo.raise("s", "Keep three retries?")?;
    repo.commit("raise")?;
    write_atom(&repo, &id, &format!("{id}-three-retries.md"))?;
    let added = repo.commit("record")?;

    let r = report(&repo)?;

    let expected = json!([{"id": id, "option": null, "commit": added, "sources": ["atom"]}]);
    assert_eq!((&r["open_items"], &r["decisions"]), (&json!([]), &expected));
    Ok(())
}

#[test]
#[cfg(unix)]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_atom_whose_name_git_would_quote_still_names_its_commit() -> TestResult {
    let repo = on_a_branch()?;
    let id = repo.raise("s", "Keep three retries?")?;
    repo.commit("raise")?;
    write_atom(&repo, &id, &format!("{id}-réessais \"trois\".md"))?;
    let added = repo.commit("record")?;

    let r = report(&repo)?;

    let expected = json!([{"id": id, "option": null, "commit": added, "sources": ["atom"]}]);
    assert_eq!(r["decisions"], expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_decision_taken_in_the_session_is_one_entry_from_all_three_sources() -> TestResult {
    let repo = on_a_branch()?;
    let id = repo.raise("s", "Keep three retries?")?;
    repo.ok("s", &["decide", &id, "A"], None)?;
    let trailer = repo.ok("s", &["trailer", &id], None)?;
    let decided = repo.commit(&format!("Settle the retries\n\n{trailer}"))?;

    let r = report(&repo)?;

    let expected = json!([{
        "id": id, "option": "A", "commit": decided,
        "sources": ["journal", "atom", "trailer"]
    }]);
    assert_eq!((&r["open_items"], &r["decisions"]), (&json!([]), &expected));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_journal_the_branch_only_appends_to_is_ok() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s", "Keep three retries?")?;
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    repo.raise("s", "Keep the cache warm?")?;
    repo.commit("more")?;

    let r = report(&repo)?;

    assert_eq!(r["journal"], json!({"ok": true, "violations": []}));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_line_the_branch_deletes_is_a_violation_and_still_exits_zero() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s", "Keep three retries?")?;
    repo.raise("s", "Keep the cache warm?")?;
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    let text = String::from_utf8(repo.journal("s")?)?;
    let first = text.lines().next().ok_or("a line")?;
    std::fs::write(repo.journal_path("s"), format!("{first}\n"))?;
    repo.commit("drop a line")?;

    let r = report(&repo)?;

    let expected =
        json!({"ok": false, "violations": [{"path": JOURNAL, "line": 2, "reason": "missing"}]});
    assert_eq!(r["journal"], expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_cut_short_base_line_the_branch_rewrites_whole_is_ok() -> TestResult {
    let repo = Repo::new(true)?;
    repo.raise("s", "Keep three retries?")?;
    repo.raise("s", "Keep the cache warm?")?;
    let text = String::from_utf8(repo.journal("s")?)?;
    let mut lines = text.lines();
    let first = lines.next().ok_or("one line")?;
    let cut = lines
        .next()
        .ok_or("two lines")?
        .get(..30)
        .ok_or("a long line")?;
    std::fs::write(repo.journal_path("s"), format!("{first}\n{cut}"))?;
    repo.commit("base")?;
    git(&repo.root, &["checkout", "-q", "-b", "feat"]);
    repo.raise("s", "Shard the ledger?")?;
    repo.commit("more")?;

    let r = report(&repo)?;

    assert_eq!(r["journal"], json!({"ok": true, "violations": []}));
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn without_json_the_report_is_one_summary_line() -> TestResult {
    let repo = on_a_branch()?;
    repo.raise("s", "Keep three retries?")?;
    let head = repo.commit("raise")?;
    let base = repo.rev("main")?;

    let out = repo.ok("ci", &["ci-check", "--base", "main"], None)?;

    let short = |s: &str| s.get(..7).map(str::to_owned).unwrap_or_default();
    let expected = format!(
        "ci-check {}..{}: 1 open (0 block merge, 0 block release, 1 unknown) · 0 decision(s) · journal ok\n",
        short(&base),
        short(&head)
    );
    assert_eq!(out, expected);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_reader_that_stops_early_is_not_a_failure_in_either_form() -> TestResult {
    let repo = on_a_branch()?;
    // Larger than stdout's buffer, so the report's own writes meet the closed pipe.
    let raises: Vec<Value> = (0..60)
        .map(|n| json!({"op": "raise", "q": format!("Should cache tier {n} keep its entries warm across restarts?")}))
        .collect();
    repo.ok("s", &["batch"], Some(&Value::from(raises).to_string()))?;
    repo.commit("raise")?;

    let outs = [
        repo.run_with_stdout_closed(&["ci-check", "--base", "main", "--json"])?,
        repo.run_with_stdout_closed(&["ci-check", "--base", "main"])?,
    ];

    let got = outs.map(|o| {
        (
            o.status.code(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    });
    let quiet = (Some(0), String::new());
    assert_eq!(got, [quiet.clone(), quiet]);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_base_that_names_no_commit_exits_one_naming_it() -> TestResult {
    let repo = on_a_branch()?;

    let out = repo.run("ci", &["ci-check", "--base", "nope"], None)?;

    let stderr = String::from_utf8(out.stderr)?;
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.starts_with("weighin: resolving --base nope"),
        "{stderr}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn outside_git_ci_check_exits_one() -> TestResult {
    let repo = Repo::new(false)?;
    let plain = repo.sibling("plain")?;

    let out = repo.run_in(&plain, &["ci-check", "--base", "main"])?;

    let stderr = String::from_utf8(out.stderr)?;
    assert_eq!(out.status.code(), Some(1), "{stderr}");
    assert_eq!(stderr, "weighin: ci-check needs a git repository\n");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_fresh_clone_with_no_ledger_gets_its_report_and_no_store() -> TestResult {
    let repo = on_a_branch()?;
    repo.raise("s", "Keep three retries?")?;
    repo.commit("raise")?;
    let clone = repo.sibling("clone")?.join("c");
    let (from, to) = (repo.root.to_string_lossy(), clone.to_string_lossy());
    git(&repo.root, &["clone", "-q", &from, &to]);
    let store = clone.join("elsewhere/ledger.sqlite3");
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
    let r: Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(r["open_items"].as_array().map(Vec::len), Some(1), "{r}");
    let created = [store.exists(), clone.join(".git/weighin").exists()];
    assert_eq!(created, [false, false], "no store, no state dir");
    Ok(())
}
