//! Past decisions anchored to a file (#116): the `decisions` command's query, and the context a
//! `PostToolUse` hook adds when an edit touches such a file.

use anyhow::Result;
use serde::Serialize;
use std::borrow::Cow;
use std::io::Write;
use std::path::Path;
use weighin_core::projection::Decision;
use weighin_core::{Item, Ledger, Status};

use crate::context::parse_anchor;
use crate::env::Env;
use crate::ops::read_ledger;
use crate::text::clip;

/// The most decisions one edit adds to the model's context, one line each (#116).
const PER_EDIT: usize = 3;

/// What `weighin decisions` asks for.
pub(crate) struct Query<'a> {
    /// The file, relative to the working directory or absolute.
    pub(crate) path: &'a Path,
    /// The line to rank the nearest anchors first by, when given.
    pub(crate) line: Option<usize>,
    /// A JSON array instead of one line per decision.
    pub(crate) json: bool,
}

/// `weighin decisions --path <path> [--line <n>] [--json]`: the decisions anchored at the
/// query's path, one line each, or as a JSON array.
///
/// # Errors
/// Fails when the ledger cannot be read or the output cannot be written.
pub(crate) fn list(env: &Env, q: &Query<'_>, out: &mut impl Write) -> Result<()> {
    let abs = if q.path.is_absolute() {
        q.path.to_path_buf()
    } else {
        env.cwd.join(q.path)
    };
    let rel = env.relative(&abs.to_string_lossy());
    let ledger = read_ledger(env)?;
    let found = anchored(&ledger, &rel, q.line);
    if q.json {
        let listed: Vec<Listed<'_>> = found.iter().map(Listed::of).collect();
        writeln!(out, "{}", serde_json::to_string(&listed)?)?;
        return Ok(());
    }
    if found.is_empty() {
        writeln!(out, "no decisions anchored at {rel}")?;
    }
    for a in &found {
        writeln!(out, "{}", a.line())?;
    }
    Ok(())
}

/// An edit the `PostToolUse` hook saw to a file in this repository: a `Write`, `Edit` or
/// `MultiEdit`.
pub(crate) struct Edit<'p> {
    /// The file, repo-relative.
    rel: String,
    /// The text an `Edit` or `MultiEdit` wrote, to find the line it touched; empty for a
    /// `Write`, which touches the whole file.
    edited: Vec<&'p str>,
}

impl<'p> Edit<'p> {
    /// The edit of `file` that wrote `edited`, or `None` for a file outside this repository,
    /// which has no repo-relative anchor to match.
    pub(crate) fn of(env: &Env, file: &str, edited: Vec<&'p str>) -> Option<Self> {
        let rel = env.relative(file);
        (!Path::new(&rel).is_absolute() && !rel.starts_with("../")).then_some(Self { rel, edited })
    }
}

/// What the `PostToolUse` hook adds after `edit`: the hook output carrying at most [`PER_EDIT`]
/// decisions in `ledger` anchored at the edited file as `additionalContext`, or `None` when there
/// are none. Reads the file only when a decision is anchored at it; never writes.
pub(crate) fn edit_context(env: &Env, ledger: &Ledger, edit: &Edit<'_>) -> Option<String> {
    let mut found = anchored_at(ledger, &edit.rel);
    if found.is_empty() {
        return None;
    }
    // A file that cannot be read now gives no line; the decisions still stand.
    let line = std::fs::read_to_string(env.root().join(&edit.rel))
        .ok()
        .and_then(|on_disk| edited_line(&on_disk, &edit.edited));
    rank(&mut found, line);
    let lines: Vec<String> = found.iter().take(PER_EDIT).map(Anchored::line).collect();
    let output = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": lines.join("\n"),
        },
    });
    Some(output.to_string())
}

/// One decision as `decisions --json` prints it.
#[derive(Serialize)]
struct Listed<'a> {
    id: &'a str,
    /// The option chosen, or the answer itself for an item with no card.
    option: &'a str,
    /// The chosen option's title on the card; `null` without one.
    title: Option<&'a str>,
    question: &'a str,
    /// The anchor: `path:line`, or the path alone.
    at: &'a str,
    line: Option<usize>,
    /// Where it was decided: `chat`, `board`, `pane` or `cli`.
    via: &'a str,
    decided_at: &'a str,
}

impl<'a> Listed<'a> {
    fn of(a: &'a Anchored<'_>) -> Self {
        Self {
            id: a.item.id.as_str(),
            option: &a.decision.option,
            title: a.title(),
            question: &a.item.question,
            at: &a.anchor,
            line: a.line,
            via: &a.decision.via,
            decided_at: &a.decision.ts,
        }
    }
}

/// A decision anchored at a file: a view of one decided item in the ledger.
#[derive(Debug)]
pub(crate) struct Anchored<'l> {
    pub(crate) item: &'l Item,
    pub(crate) decision: &'l Decision,
    /// The anchor as the item gives it: `path:line`, or the path alone.
    pub(crate) anchor: Cow<'l, str>,
    /// The anchor's line, when it has one.
    pub(crate) line: Option<usize>,
}

/// Every item decided by its owner and still standing (not reopened, superseded, dismissed or
/// defaulted since) whose anchor is `path`, a repo-relative path.
///
/// A defaulted item is not listed: it is the agent's own call or an expired default, which the
/// digest reviews, not a decision. With `line`, the nearest anchor comes first and anchors with
/// no line last; otherwise, and between equals, the most recently changed item comes first.
pub(crate) fn anchored<'l>(
    ledger: &'l Ledger,
    path: &str,
    line: Option<usize>,
) -> Vec<Anchored<'l>> {
    let mut found = anchored_at(ledger, path);
    rank(&mut found, line);
    found
}

/// The decisions [`anchored`] lists, in no order.
fn anchored_at<'l>(ledger: &'l Ledger, path: &str) -> Vec<Anchored<'l>> {
    let path = normalised(path);
    ledger
        .items
        .values()
        .filter(|i| i.status == Status::Decided)
        .filter_map(|item| {
            let decision = item.decision.as_ref()?;
            let (anchor, line) = anchor_at(item, &path)?;
            Some(Anchored {
                item,
                decision,
                anchor,
                line,
            })
        })
        .collect()
}

/// Order `found` as [`anchored`] lists it, by distance from `line` when given, else newest first.
fn rank(found: &mut [Anchored<'_>], line: Option<usize>) {
    found.sort_by_key(|a| {
        let distance = line.zip(a.line).map(|(edit, at)| edit.abs_diff(at));
        // `false` sorts first: anchors at a known distance come ahead of those with no line.
        // Newest is by decision time: a review (D-0022) appends an event but decides nothing.
        let decision: &Decision = a.decision;
        (
            distance.is_none(),
            distance,
            std::cmp::Reverse((decision.ts.as_str(), a.item.last_position)),
        )
    });
}

impl Anchored<'_> {
    /// `<id> decided <choice>: <question> (<anchor>)` on one line. The choice is the option id
    /// and, when the card names it, its quoted title.
    pub(crate) fn line(&self) -> String {
        let option = &self.decision.option;
        let choice = match self.title() {
            Some(title) => format!("{option} \"{}\"", one_line(title, 60)),
            None => one_line(option, 80),
        };
        format!(
            "{} decided {choice}: {} ({})",
            self.item.id,
            one_line(&self.item.question, 120),
            self.anchor
        )
    }

    /// The chosen option's title on the item's card, when it has one.
    pub(crate) fn title(&self) -> Option<&str> {
        let card = self.item.card.as_ref()?;
        card.option(&self.decision.option).map(|o| o.title.as_str())
    }
}

/// `s` on one line, cut to at most `n` characters.
fn one_line(s: &str, n: usize) -> String {
    clip(&s.replace(['\r', '\n'], " "), n)
}

/// The 1-based line of `on_disk` where the first of `written` that is found there starts, its
/// leading line breaks passed over. Text that is empty, or only line breaks, names no line.
pub(crate) fn edited_line(on_disk: &str, written: &[&str]) -> Option<usize> {
    written.iter().find_map(|w| {
        let text = w.trim_start_matches(['\r', '\n']);
        if text.is_empty() {
            return None;
        }
        let at = on_disk.find(text)?;
        let before = on_disk.get(..at)?;
        before.matches('\n').count().checked_add(1)
    })
}

/// The item's anchor and its line when the anchor names `path`: its `at`, else the file it was
/// scanned from.
fn anchor_at<'l>(item: &'l Item, path: &str) -> Option<(Cow<'l, str>, Option<usize>)> {
    if let Some(at) = item.at.as_deref() {
        let (file, line) = parse_anchor(at);
        if normalised(file) == path {
            return Some((Cow::Borrowed(at), line));
        }
    }
    let source = item.source_path.as_deref()?;
    if normalised(source) != path {
        return None;
    }
    Some(match item.source_line {
        Some(line) => (Cow::Owned(format!("{source}:{line}")), Some(line)),
        None => (Cow::Borrowed(source), None),
    })
}

/// `path` with forward slashes and no leading `./`, so spellings of one repo-relative file match.
fn normalised(path: &str) -> Cow<'_, str> {
    if path.contains('\\') {
        Cow::Owned(path.replace('\\', "/").trim_start_matches("./").to_owned())
    } else {
        Cow::Borrowed(path.trim_start_matches("./"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use weighin_core::event::{
        Acknowledged, Channel, Decided, DefaultCause, Defaulted, LedgerEvent, Provenance, Raised,
        Reopened, Superseded,
    };
    use weighin_core::marker::{Authority, Door, Kind};
    use weighin_core::{EventEnvelope, ItemId};

    fn envelope(event: LedgerEvent) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: "test".into(),
            session_id: Some("s1".into()),
            worktree: None,
            branch: None,
            head: None,
            ts: "2026-10-19T00:00:00Z".into(),
        }
    }

    /// A ledger built one event at a time, each at the next store position.
    #[derive(Default)]
    struct Built {
        ledger: Ledger,
        next: u64,
    }

    impl Built {
        fn push(&mut self, event: LedgerEvent) {
            self.next += 1;
            self.ledger.apply(self.next, &envelope(event));
        }

        /// Append `event` stamped `ts`.
        fn push_at(&mut self, ts: &str, event: LedgerEvent) {
            self.next += 1;
            let at = EventEnvelope {
                ts: ts.into(),
                ..envelope(event)
            };
            self.ledger.apply(self.next, &at);
        }

        /// Raise `q` anchored `at`, and return its id.
        fn raise(&mut self, q: &str, at: Option<&str>) -> ItemId {
            let id = ItemId::mint("s1", q);
            self.push(LedgerEvent::ItemRaised(Raised {
                item_id: id.clone(),
                kind: Kind::Question,
                question: q.into(),
                did: None,
                flip: None,
                at: at.map(str::to_owned),
                sym: None,
                blocks: None,
                door_hint: Door::TwoWay,
                authority_hint: Authority::Authority,
                channel: Channel::Marker,
                quote: q.into(),
                source_path: None,
                source_line: None,
            }));
            id
        }

        fn decide(&mut self, id: &ItemId, option: &str) {
            self.push(LedgerEvent::ItemDecided(Decided {
                item_id: id.clone(),
                option: option.into(),
                decider: "human:ryan".into(),
                provenance: Provenance::UserDirected,
                note: String::new(),
                tree_hash: None,
                matched_recommendation: None,
                via: "chat".into(),
                seconds_to_decide: None,
                reason: "answered".into(),
            }));
        }

        /// Raise `q` at `at` and decide it with `option`.
        fn decided(&mut self, q: &str, at: &str, option: &str) -> ItemId {
            let id = self.raise(q, Some(at));
            self.decide(&id, option);
            id
        }
    }

    fn ids(found: &[Anchored<'_>]) -> Vec<String> {
        found.iter().map(|a| a.item.id.to_string()).collect()
    }

    #[test]
    fn a_decision_anchored_at_the_file_is_listed() {
        let mut b = Built::default();
        let id = b.decided(
            "Per series or per record?",
            "src/retention.rs:6",
            "per series",
        );

        let found = anchored(&b.ledger, "src/retention.rs", None);

        assert_eq!(ids(&found), [id.to_string()]);
    }

    #[test]
    fn a_decision_anchored_at_another_file_is_not_listed() {
        let mut b = Built::default();
        b.decided(
            "Per series or per record?",
            "src/retention.rs:6",
            "per series",
        );

        let found = anchored(&b.ledger, "src/other.rs", None);

        assert_eq!(ids(&found), Vec::<String>::new());
    }

    /// A review (D-0022) is a later event on the older decision; newest first still means the
    /// newest decision.
    #[test]
    fn a_reviewed_older_decision_stays_behind_a_newer_one() {
        let mut b = Built::default();
        let older = b.raise("Older?", Some("src/a.rs"));
        let decided = |b: &mut Built, id: &ItemId, ts: &str| {
            b.push_at(
                ts,
                LedgerEvent::ItemDecided(Decided {
                    item_id: id.clone(),
                    option: "A".into(),
                    decider: "human:ryan".into(),
                    provenance: Provenance::UserDirected,
                    note: String::new(),
                    tree_hash: None,
                    matched_recommendation: None,
                    via: "chat".into(),
                    seconds_to_decide: None,
                    reason: "answered".into(),
                }),
            );
        };
        decided(&mut b, &older, "2026-09-01T00:00:00Z");
        let newer = b.raise("Newer?", Some("src/a.rs"));
        decided(&mut b, &newer, "2026-09-20T00:00:00Z");
        b.push_at(
            "2026-12-01T00:00:00Z",
            LedgerEvent::ItemAcknowledged(Acknowledged {
                item_id: older.clone(),
                reason: "still holds".into(),
            }),
        );

        let found = anchored(&b.ledger, "src/a.rs", None);

        assert_eq!(ids(&found), [newer.to_string(), older.to_string()]);
    }

    #[test]
    fn decisions_reopened_superseded_or_defaulted_since_are_not_listed() {
        let mut b = Built::default();
        let reopened = b.decided("Reopened since?", "src/a.rs:1", "A");
        b.push(LedgerEvent::ItemReopened(Reopened {
            item_id: reopened,
            note: String::new(),
            reason: "reopened".into(),
        }));
        let superseded = b.decided("Superseded since?", "src/a.rs:2", "A");
        b.push(LedgerEvent::ItemSuperseded(Superseded {
            item_id: superseded,
            by: "kb-decision-x".into(),
            reason: "superseded".into(),
        }));
        let defaulted = b.raise("Defaulted?", Some("src/a.rs:3"));
        b.push(LedgerEvent::ItemDefaulted(Defaulted {
            item_id: defaulted,
            option: "A".into(),
            cause: DefaultCause::TwoWay,
            reason: "the agent's call".into(),
        }));
        let standing = b.decided("Still standing?", "src/a.rs:4", "B");

        let found = anchored(&b.ledger, "src/a.rs", None);

        assert_eq!(ids(&found), [standing.to_string()]);
    }

    #[test]
    fn a_scanned_source_line_anchors_a_decision_too() {
        let mut b = Built::default();
        let id = ItemId::mint("s1", "Shard the store?");
        b.push(LedgerEvent::ItemRaised(Raised {
            item_id: id.clone(),
            kind: Kind::Question,
            question: "Shard the store?".into(),
            did: None,
            flip: None,
            at: None,
            sym: None,
            blocks: None,
            door_hint: Door::Unsure,
            authority_hint: Authority::Authority,
            channel: Channel::Scan,
            quote: "- Shard the store?".into(),
            source_path: Some("docs/plan.md".into()),
            source_line: Some(4),
        }));
        b.decide(&id, "B");

        let found = anchored(&b.ledger, "docs/plan.md", None);

        let shown: Vec<(&str, Option<usize>)> =
            found.iter().map(|a| (a.anchor.as_ref(), a.line)).collect();
        assert_eq!(shown, [("docs/plan.md:4", Some(4))]);
    }

    #[test]
    fn anchors_and_paths_match_with_dot_slash_and_backslashes_normalised() {
        let mut b = Built::default();
        let id = b.decided("Which separator?", "./src\\win.rs:3", "slash");

        let found = anchored(&b.ledger, "src/win.rs", None);

        assert_eq!(ids(&found), [id.to_string()]);
    }

    #[test]
    fn with_a_line_the_nearest_anchor_comes_first_and_lineless_ones_last() {
        let mut b = Built::default();
        let whole = b.decided("Whole file?", "src/a.rs", "A");
        let far = b.decided("Far?", "src/a.rs:90", "A");
        let near = b.decided("Near?", "src/a.rs:12", "A");
        let nearer = b.decided("Nearer?", "src/a.rs:18", "A");

        let found = anchored(&b.ledger, "src/a.rs", Some(20));

        assert_eq!(
            ids(&found),
            [nearer, near, far, whole].map(|i| i.to_string())
        );
    }

    #[test]
    fn without_a_line_the_newest_decision_comes_first() {
        let mut b = Built::default();
        let first = b.raise("First raised, decided last?", Some("src/a.rs:1"));
        let second = b.decided("Second, decided first?", "src/a.rs:2", "A");
        b.decide(&first, "A");

        let found = anchored(&b.ledger, "src/a.rs", None);

        assert_eq!(ids(&found), [first, second].map(|i| i.to_string()));
    }

    #[test]
    fn a_decision_reads_as_one_line_naming_its_choice_question_and_anchor() {
        let mut b = Built::default();
        let id = b.decided(
            "Per series or per record?",
            "src/retention.rs:6",
            "per series",
        );

        let found = anchored(&b.ledger, "src/retention.rs", None);

        let lines: Vec<String> = found.iter().map(Anchored::line).collect();
        assert_eq!(
            lines,
            [format!(
                "{id} decided per series: Per series or per record? (src/retention.rs:6)"
            )]
        );
    }

    #[test]
    fn the_edited_line_is_where_the_new_text_starts_in_the_file() {
        let on_disk = "one\ntwo\nthree\nfour\n";

        let line = edited_line(on_disk, &["", "\nthree\nfo"]);

        assert_eq!(line, Some(3));
    }

    #[test]
    fn text_not_in_the_file_gives_no_edited_line() {
        assert_eq!(edited_line("one\ntwo\n", &["gone", "\n"]), None);
    }
}
