//! Decision ageing (D-0022, #173): a standing two-way decision with a flip condition comes due a
//! look in the digest `caps.review_age_days` after its newest decide or review, and `ack`
//! records the review. A decision is made old by back-dating the ledger's stored events, as if
//! the binary had written them then, so no test waits on the wall clock.

use crate::support::{setup, weighin};
use happenstance_sqlite::rusqlite::Connection;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const QUESTION: &str = "Name the helper apply or run?";
const FLIP: &str = "a second caller needs a verb";
const LONG_AGO: &str = "2020-01-01T09:00:00Z";
const OLD_DATE: &str = "last_reviewed: \"2020-01-01\"";

/// Run a `batch` that must succeed; return its receipts.
fn batch(repo: &Path, ops: &Value) -> TestResult<String> {
    let (ok, out) = weighin(repo, "s1", &["batch"], Some(&ops.to_string()));
    if ok {
        Ok(out)
    } else {
        Err(format!("batch refused: {out}").into())
    }
}

/// Raise `q` behind `door`, ground it on a card whose flip is `flip`, decide A; return its id.
fn decide(repo: &Path, q: &str, door: &str, flip: &str) -> TestResult<String> {
    let card = json!({
        "why": "Callers read it.",
        "timing": "Decide now.",
        "options": [{"id": "A", "title": "apply"}, {"id": "B", "title": "run"}],
        "recommendation": {"option": "A", "flip": flip}
    });
    let out = batch(
        repo,
        &json!([
            {"op": "raise", "q": q, "door": door},
            {"op": "ground", "id": "@1", "card": card},
            {"op": "decide", "id": "@1", "option": "A"}
        ]),
    )?;
    let id = out
        .split_whitespace()
        .find(|w| w.starts_with("wi-"))
        .ok_or_else(|| format!("the raise names its id: {out}"))?;
    Ok(id
        .trim_end_matches(|c: char| !c.is_ascii_hexdigit())
        .to_owned())
}

/// `ack` `id` through `batch`, the write path the skill's digest uses; return the receipt.
fn ack(repo: &Path, id: &str) -> TestResult<String> {
    batch(repo, &json!([{"op": "ack", "id": id}]))
}

/// Rewrite the timestamp of every event in the ledger to `ts`, under the ledger's own journal
/// mode.
fn backdate(repo: &Path, ts: &str) -> TestResult {
    let conn = Connection::open(repo.join(".git/weighin/ledger.sqlite3"))?;
    conn.pragma_update(None, "journal_mode", "TRUNCATE")?;
    let changed = conn.execute(
        "UPDATE event SET data = CAST(json_set(CAST(data AS TEXT), '$.ts', ?1) AS BLOB)",
        [ts],
    )?;
    if changed == 0 {
        return Err("there were no events to back-date".into());
    }
    Ok(())
}

fn digest_json(repo: &Path) -> TestResult<Value> {
    let (ok, out) = weighin(repo, "s1", &["digest", "--json"], None);
    if !ok {
        return Err(format!("digest --json failed: {out}").into());
    }
    Ok(serde_json::from_str(out.trim())?)
}

/// The one decision atom's path and text.
fn atom_text(repo: &Path) -> TestResult<(PathBuf, String)> {
    let atom = std::fs::read_dir(repo.join(".kb/_intake/decisions"))?
        .next()
        .ok_or("the decision wrote an atom")??
        .path();
    let text = std::fs::read_to_string(&atom)?;
    Ok((atom, text))
}

fn review_line(atom: &str) -> TestResult<&str> {
    Ok(atom
        .lines()
        .find(|l| l.starts_with("last_reviewed: "))
        .ok_or("the atom has a review date")?)
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn an_old_two_way_decision_is_due_a_look_until_it_is_reviewed() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decide(&repo, QUESTION, "two-way", FLIP)?;
    decide(&repo, "Keep the region in eu-west?", "one-way", FLIP)?;
    decide(&repo, "Call the flag --dry or --plan?", "two-way", "")?;
    backdate(&repo, LONG_AGO)?;

    let (ok, text) = weighin(&repo, "s1", &["digest"], None);
    assert!(ok, "{text}");
    assert!(text.contains("Decisions due a look (1)"), "{text}");
    let line = text
        .lines()
        .find(|l| l.starts_with(&id))
        .ok_or_else(|| format!("{id} is listed: {text}"))?;
    assert!(
        line.contains("→ A") && line.contains(FLIP) && line.contains(QUESTION),
        "{line}"
    );
    let doc = digest_json(&repo)?;
    let due = &doc["due_for_review"];
    assert_eq!(due.as_array().map(Vec::len), Some(1), "{due}");
    assert_eq!(due[0]["id"], json!(id));
    assert_eq!(due[0]["flip_condition"], json!(FLIP));
    assert!(
        due[0]["age_days"].as_u64().is_some_and(|d| d > 2000),
        "{due}"
    );
    let (_, primed) = weighin(&repo, "s1", &["prime"], None);
    assert!(
        primed.contains("default(s) to review · 1 decision(s) due a look."),
        "{primed}"
    );

    let out = ack(&repo, &id)?;

    assert!(
        out.contains(&format!("reviewed decision on {id}: still holds · atom ")),
        "{out}"
    );
    let (ok, text) = weighin(&repo, "s1", &["digest"], None);
    assert!(ok && !text.contains("due a look"), "{text}");
    assert_eq!(digest_json(&repo)?.get("due_for_review"), None);
    let (_, primed) = weighin(&repo, "s1", &["prime"], None);
    assert!(!primed.contains("due a look"), "{primed}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_review_moves_the_atom_last_reviewed_date_and_nothing_else() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decide(&repo, QUESTION, "two-way", FLIP)?;
    backdate(&repo, LONG_AGO)?;
    let (atom, text) = atom_text(&repo)?;
    let aged = text.replace(review_line(&text)?, OLD_DATE);
    std::fs::write(&atom, &aged)?;

    ack(&repo, &id)?;

    let (_, after) = atom_text(&repo)?;
    let moved = review_line(&after)?;
    assert_ne!(moved, OLD_DATE, "the date moved");
    assert_eq!(after.replace(moved, OLD_DATE), aged, "only the date moved");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_review_age_of_zero_lists_nothing() -> TestResult {
    let (_tmp, repo) = setup();
    let config = repo.join(".kb/_ledger/config.json");
    std::fs::create_dir_all(config.parent().ok_or("the config has a folder")?)?;
    std::fs::write(&config, r#"{"caps":{"review_age_days":0}}"#)?;
    decide(&repo, QUESTION, "two-way", FLIP)?;
    backdate(&repo, LONG_AGO)?;

    let (ok, text) = weighin(&repo, "s1", &["digest"], None);

    assert!(ok && !text.contains("due a look"), "{text}");
    assert_eq!(digest_json(&repo)?.get("due_for_review"), None);
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn stats_count_reviews_and_the_reopens_that_follow_them() -> TestResult {
    let (_tmp, repo) = setup();
    let id = decide(&repo, QUESTION, "two-way", FLIP)?;
    let other = decide(&repo, "Call the flag --dry or --plan?", "two-way", FLIP)?;
    backdate(&repo, LONG_AGO)?;
    for item in [&id, &other] {
        ack(&repo, item)?;
    }
    batch(
        &repo,
        &json!([{"op": "reopen", "id": id, "reason": "the second caller arrived"}]),
    )?;

    let (ok, stats) = weighin(&repo, "s1", &["stats", "--json"], None);
    let (_, text) = weighin(&repo, "s1", &["stats"], None);

    assert!(ok, "{stats}");
    let h: Value = serde_json::from_str(stats.trim())?;
    assert_eq!(
        (&h["decisions_reviewed"], &h["reopened_after_review"]),
        (&json!(2), &json!(1)),
        "{stats}"
    );
    assert!(text.contains("reviewed decisions 2 (reopened 1)"), "{text}");
    Ok(())
}
