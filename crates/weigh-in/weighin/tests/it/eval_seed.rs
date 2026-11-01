//! Marker capture precision against the eval seed (`eval/`, #22).
//!
//! Each seed line is text an agent wrote, labelled `capture` (a decision the ledger should hold)
//! or `skip` (noise, or no decision). The text goes through the real capture path in a scratch
//! repository: a `Stop` hook for `channel: marker`, a `PostToolUse` Write of a markdown file for
//! `channel: scan`. An item counts as captured when the ledger holds at least one item afterwards.
//!
//! * precision = captured and labelled `capture` / captured
//! * recall = captured and labelled `capture` / labelled `capture`
//! * status agreement = captured `capture` items whose first ledger item has `expect.status`
//!
//! The slice 1 exit criterion is precision of at least 80% (`docs/v1-plan.md`, Phase 3), measured
//! on the real seed once it exists. `cargo test -p weighin --test it eval_seed -- --nocapture` prints
//! the scores.

use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// One line of a seed file under `eval/`. Provenance fields (`session`, `source`, `repo`, `origin`, `by`, `on`) are
/// kept in the file for the reader and not used here.
#[derive(Debug, Deserialize)]
struct SeedItem {
    id: String,
    #[serde(default)]
    channel: Channel,
    text: String,
    label: Label,
    #[serde(default)]
    expect: Option<Expect>,
    why: String,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Channel {
    #[default]
    Marker,
    Scan,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Label {
    Capture,
    Skip,
}

#[derive(Debug, Deserialize)]
struct Expect {
    /// The ledger status of the first item raised: `raised` (open) or `defaulted`.
    status: String,
}

fn git(dir: &Path, args: &[&str]) -> Res<()> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into())
    }
}

fn weighin(dir: &Path, args: &[&str], stdin: &str) -> Res<String> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_weighin"))
        .current_dir(dir)
        .args(["--session", "eval"])
        .args(args)
        .env_remove("WEIGHIN_STORE")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_CODE_ENTRYPOINT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut pipe) = child.stdin.take() {
        pipe.write_all(stdin.as_bytes())?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        Ok(String::from_utf8(out.stdout)?)
    } else {
        Err(format!("weighin {args:?}: {}", String::from_utf8_lossy(&out.stderr)).into())
    }
}

/// A fresh repository with a ledger config, so every item is scored on its own.
fn repo() -> Res<(tempfile::TempDir, PathBuf)> {
    let tmp = tempfile::tempdir()?;
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(repo.join("docs"))?;
    std::fs::write(repo.join("README.md"), "# Scratch\n")?;
    git(&repo, &["init", "-q", "-b", "main"])?;
    git(&repo, &["add", "."])?;
    git(&repo, &["commit", "-q", "-m", "init"])?;
    weighin(&repo, &["init", "--class", "own", "--owner", "eval"], "")?;
    Ok((tmp, repo))
}

/// Run one seed item through capture and return the statuses of the items it raised.
fn capture(item: &SeedItem) -> Res<Vec<String>> {
    let (_tmp, repo) = repo()?;
    let payload = match item.channel {
        Channel::Marker => json!({
            "hook_event_name": "Stop",
            "session_id": "eval",
            "cwd": repo,
            "last_assistant_message": item.text,
        }),
        Channel::Scan => {
            let file = repo.join("docs/notes.md");
            std::fs::write(&file, &item.text)?;
            json!({
                "hook_event_name": "PostToolUse",
                "session_id": "eval",
                "cwd": repo,
                "tool_name": "Write",
                "tool_input": {"file_path": file, "content": item.text},
            })
        }
    };
    weighin(&repo, &["hook"], &payload.to_string())?;
    let listed: Vec<Value> = serde_json::from_str(&weighin(
        &repo,
        &["open", "--all", "--top", "1000", "--json"],
        "",
    )?)?;
    Ok(listed
        .iter()
        .map(|i| {
            i.get("status")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_owned()
        })
        .collect())
}

#[derive(Debug, Default)]
struct Score {
    true_pos: usize,
    false_pos: usize,
    false_neg: usize,
    true_neg: usize,
    status_agree: usize,
    status_checked: usize,
    /// One line per item scored wrong, for the failure message.
    misses: Vec<String>,
}

impl Score {
    fn precision(&self) -> Res<f64> {
        ratio(self.true_pos, self.true_pos + self.false_pos)
    }

    fn recall(&self) -> Res<f64> {
        ratio(self.true_pos, self.true_pos + self.false_neg)
    }

    fn summary(&self) -> Res<String> {
        Ok(format!(
            "precision {:.0}% ({} of {} captured) · recall {:.0}% ({} of {} labelled capture) · status {} of {} · true negatives {}",
            100.0 * self.precision()?,
            self.true_pos,
            self.true_pos + self.false_pos,
            100.0 * self.recall()?,
            self.true_pos,
            self.true_pos + self.false_neg,
            self.status_agree,
            self.status_checked,
            self.true_neg,
        ))
    }
}

fn ratio(n: usize, d: usize) -> Res<f64> {
    let (n, d) = (u32::try_from(n)?, u32::try_from(d)?);
    Ok(if d == 0 {
        0.0
    } else {
        f64::from(n) / f64::from(d)
    })
}

fn score(items: &[SeedItem]) -> Res<Score> {
    let mut s = Score::default();
    for item in items {
        let statuses = capture(item)?;
        let got = !statuses.is_empty();
        let want = item.label == Label::Capture;
        let id = &item.id;
        match (got, want) {
            (true, true) => s.true_pos += 1,
            (true, false) => {
                s.false_pos += 1;
                s.misses
                    .push(format!("{id}: captured, labelled skip ({})", item.why));
            }
            (false, true) => {
                s.false_neg += 1;
                s.misses.push(format!(
                    "{id}: not captured, labelled capture ({})",
                    item.why
                ));
            }
            (false, false) => s.true_neg += 1,
        }
        if got
            && want
            && let Some(expect) = &item.expect
        {
            s.status_checked += 1;
            if statuses.first() == Some(&expect.status) {
                s.status_agree += 1;
            } else {
                s.misses.push(format!(
                    "{id}: status {statuses:?}, expected {}",
                    expect.status
                ));
            }
        }
    }
    Ok(s)
}

fn parse(lines: &str) -> Res<Vec<SeedItem>> {
    lines
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Ok(serde_json::from_str(l)?))
        .collect()
}

fn line(id: &str, label: &str, status: Option<&str>, text: &str) -> String {
    let mut v = json!({"id": id, "label": label, "why": "fixture", "text": text});
    if let (Some(s), Some(obj)) = (status, v.as_object_mut()) {
        obj.insert("expect".into(), json!({"status": s}));
    }
    v.to_string()
}

fn block(door: &str, authority: &str, q: &str, did: &str) -> String {
    format!(
        "Done.\n\n```weigh-in\nkind: call\ndoor: {door}\nauthority: {authority}\nq: {q}\ndid: {did}\n```\n"
    )
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn scorer_counts_each_outcome() -> Res<()> {
    let one_way = block(
        "one-way",
        "authority",
        "Should deleted tenants' backups be kept for 30 or 90 days?",
        "kept 30 days",
    );
    let default = block(
        "two-way",
        "judgment",
        "Name the retry helper backoff or retry_with_delay?",
        "named it backoff",
    );
    let noise = block(
        "two-way",
        "judgment",
        "Use a for loop or an iterator here?",
        "used an iterator",
    );
    let scan = json!({"id": "t-6", "channel": "scan", "label": "capture", "why": "fixture",
        "text": "# Plan\n\n## Open questions\n- Should the ledger be sharded by tenant?\n"});
    let seed = [
        line("t-1", "capture", Some("raised"), &one_way),
        line("t-2", "capture", Some("defaulted"), &default),
        line("t-3", "skip", None, &noise),
        line(
            "t-4",
            "capture",
            None,
            "I assumed the API is internal-only, so I skipped versioning it.",
        ),
        line("t-5", "skip", None, "Tests pass on all three platforms."),
        scan.to_string(),
    ]
    .join("\n");

    let s = score(&parse(&seed)?)?;

    assert_eq!(
        (
            s.true_pos,
            s.false_pos,
            s.false_neg,
            s.true_neg,
            s.status_agree,
            s.status_checked
        ),
        (3, 1, 1, 1, 2, 2),
        "{s:?}"
    );
    assert!((s.precision()? - 0.75).abs() < 1e-9, "{}", s.summary()?);
    assert!((s.recall()? - 0.75).abs() < 1e-9, "{}", s.summary()?);
    Ok(())
}

/// The real seed (#22): end-of-turn messages from sessions that ran with the marker convention
/// (0.5.0 and later), labelled by hand. This is where the slice 1 exit criterion applies:
/// capture precision of at least 80%. It must hold at least 20 items, so the bar is not met
/// by a seed trimmed down to the items capture gets right.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn real_seed_meets_the_precision_bar() -> Res<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/seed.jsonl");
    let items = parse(&std::fs::read_to_string(&path)?)?;
    assert!(
        items.len() >= 20,
        "eval/seed.jsonl holds {} items; the seed is at least 20",
        items.len()
    );

    let s = score(&items)?;

    writeln!(std::io::stderr(), "real seed: {}", s.summary()?)?;
    assert!(
        s.true_pos + s.false_pos > 0 && s.precision()? >= 0.8,
        "precision below 80%: {}\n{}",
        s.summary()?,
        s.misses.join("\n")
    );
    Ok(())
}

/// The provisional seed (#22): the real messages of five earlier sessions, none of which carried a
/// real block, plus blocks written onto some of them. Its precision is set by how many noise
/// blocks were written, so it is pinned here rather than held to the bar: a change to what capture
/// raises, or to which items default, moves these counts. The 80% bar applies to the real seed
/// (`eval/seed.jsonl`), drafted from blocks agents emit during Phase 3's daily use.
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn scripted_seed_scores_as_recorded() -> Res<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../eval/seed-scripted.jsonl");
    let items = parse(&std::fs::read_to_string(&path)?)?;

    let s = score(&items)?;

    writeln!(std::io::stderr(), "scripted seed: {}", s.summary()?)?;
    assert_eq!(
        (
            s.true_pos,
            s.false_pos,
            s.false_neg,
            s.true_neg,
            s.status_agree,
            s.status_checked
        ),
        (11, 5, 11, 20, 11, 11),
        "{}\n{}",
        s.summary()?,
        s.misses.join("\n")
    );
    Ok(())
}
