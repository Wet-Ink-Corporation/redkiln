//! The `ci-check` report: its wire format (`weighin.ci-check/1`), the rules that build it from
//! the folded journals and the attested decisions, and its one-line text form. No I/O.

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use weighin_core::marker::{Authority, Door};
use weighin_core::{Item, ItemId, Ledger};

pub(crate) use super::verify::{Reason, Violation};

/// The schema name and version the JSON report carries.
pub(crate) const SCHEMA: &str = "weighin.ci-check/1";

/// The report `ci-check` writes (OUT-01), a view of what [`build`] read: field order is wire
/// order.
#[derive(Debug, Serialize)]
pub(crate) struct CiReport<'a> {
    schema: &'static str,
    base: &'a str,
    head: &'a str,
    open_items: Vec<OpenItem<'a>>,
    decisions: Vec<DecisionOut<'a>>,
    journal: JournalCheck<'a>,
}

/// An item still open at head.
#[derive(Debug, Serialize)]
struct OpenItem<'a> {
    id: &'a ItemId,
    status: &'static str,
    kind: &'static str,
    /// The triaged door, or `null` before triage; never the raiser's hint.
    door: Option<&'static str>,
    /// The triaged authority, or `null` before triage; never the raiser's hint.
    authority: Option<&'static str>,
    blocks: Blocks,
    /// The deadline [`blocks_of`] read: the deferral's when deferred, else the triage's.
    resolve_by: Option<&'a str>,
    question: &'a str,
}

/// What an open item holds up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Blocks {
    /// It must be settled before the branch merges.
    Merge,
    /// It must be settled before the next release.
    Release,
    /// It has a date deadline, which holds up neither.
    None,
    /// Nothing recorded says.
    Unknown,
}

/// One decision the branch adds.
#[derive(Debug, Serialize)]
struct DecisionOut<'a> {
    id: &'a ItemId,
    option: Option<&'a str>,
    commit: Option<&'a str>,
    sources: Vec<Source>,
}

/// Where a decision is attested. The derive order is the output order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Source {
    /// An `item_decided` event added to a committed journal.
    Journal,
    /// A decision atom added under `.kb`.
    Atom,
    /// A `Weigh-In:` trailer in a commit message.
    Trailer,
}

impl Source {
    /// Which source's commit the report names first: the trailer's, then the atom's, then the
    /// journal line's.
    const COMMIT_PRIORITY: [Self; 3] = [Self::Trailer, Self::Atom, Self::Journal];
}

/// The append-only result.
#[derive(Debug, Serialize)]
struct JournalCheck<'a> {
    ok: bool,
    violations: Vec<&'a Violation>,
}

/// One source's word that an item was decided in the range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attested {
    pub(crate) id: ItemId,
    /// The option, when the source names one (atoms do not, B-28).
    pub(crate) option: Option<String>,
    pub(crate) source: Source,
    /// The commit that added it, when known.
    pub(crate) commit: Option<String>,
}

/// What [`build`] reads (API-10).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Inputs<'a> {
    /// The base commit's full sha.
    pub(crate) base: &'a str,
    /// The head commit's full sha.
    pub(crate) head: &'a str,
    /// The journals at head, folded.
    pub(crate) ledger: &'a Ledger,
    /// Items an atom at head or a trailer in the range settles, each with its latest such record.
    pub(crate) closing: &'a BTreeMap<ItemId, When>,
    /// Items a journal at head reopened, each with its latest reopen.
    pub(crate) reopens: &'a BTreeMap<ItemId, When>,
    /// Every source's word on the decisions the range adds, trailers oldest first.
    pub(crate) attested: &'a [Attested],
    pub(crate) violations: &'a [Violation],
}

/// The report: open items by id, decisions by (id, option), violations by (path, line).
pub(crate) fn build(inputs: Inputs<'_>) -> CiReport<'_> {
    let Inputs {
        base,
        head,
        ledger,
        closing,
        reopens,
        attested,
        violations,
    } = inputs;
    let open_items = ledger
        .items
        .values()
        .filter(|i| i.status.is_open())
        .filter(|i| still_open(closing.get(&i.id).copied(), reopens.get(&i.id).copied()))
        .map(|i| open_item(i, ledger))
        .collect();
    let mut violations: Vec<&Violation> = violations.iter().collect();
    violations.sort();
    CiReport {
        schema: SCHEMA,
        base,
        head,
        open_items,
        decisions: decisions(attested),
        journal: JournalCheck {
            ok: violations.is_empty(),
            violations,
        },
    }
}

/// When a record entered the history the report reads. The derive order is time order: before
/// the range (or outside it), in the range's commits oldest first, then not yet committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum When {
    /// Before the range: at base, or in a commit the range does not hold.
    Base,
    /// In the range's commit with this index, oldest first.
    Commit(usize),
    /// In the working tree, not committed.
    Uncommitted,
}

/// Whether an item the journals leave open stays open, given the latest record that closes it
/// (an atom at head or a trailer in the range) and the latest journal reopen. A reopen counts
/// only when it is newer than what closed the item; a reopen in the same commit loses, since
/// that commit also records the decision. When both are from before the range, the journals'
/// own fold stands: an atom is never deleted (B-01), so it cannot speak to a later reopen.
fn still_open(closing: Option<When>, reopen: Option<When>) -> bool {
    match (closing, reopen) {
        (None, _) => true,
        (Some(_), None) => false,
        (Some(c), Some(r)) => c == When::Base || r > c,
    }
}

fn open_item<'a>(i: &'a Item, ledger: &Ledger) -> OpenItem<'a> {
    OpenItem {
        id: &i.id,
        status: i.status.as_str(),
        kind: i.kind.as_str(),
        door: i.door.map(Door::as_str),
        authority: i.authority.map(Authority::as_str),
        blocks: blocks_of(i, ledger),
        resolve_by: deadline(i),
        question: &i.question,
    }
}

/// The attestations as one entry per (item, option). An option-less attestation joins the one
/// optioned entry of its item when there is exactly one, and otherwise stands as the item's
/// entry with no option; two sources naming different options give two entries.
fn decisions(attested: &[Attested]) -> Vec<DecisionOut<'_>> {
    let mut by_item: BTreeMap<&ItemId, Vec<&Attested>> = BTreeMap::new();
    for a in attested {
        by_item.entry(&a.id).or_default().push(a);
    }
    let mut out = Vec::new();
    for (id, list) in by_item {
        let mut entries: BTreeMap<Option<&str>, Vec<&Attested>> = BTreeMap::new();
        let mut without = Vec::new();
        for a in list {
            match a.option.as_deref() {
                Some(option) => entries.entry(Some(option)).or_default().push(a),
                None => without.push(a),
            }
        }
        let joins = entries.len() == 1;
        if let (true, Some(only)) = (joins, entries.values_mut().next()) {
            only.extend(without);
        } else if !without.is_empty() {
            entries.insert(None, without);
        }
        out.extend(
            entries
                .into_iter()
                .map(|(option, list)| decision(id, option, &list)),
        );
    }
    out
}

fn decision<'a>(id: &'a ItemId, option: Option<&'a str>, list: &[&'a Attested]) -> DecisionOut<'a> {
    let sources: BTreeSet<Source> = list.iter().map(|a| a.source).collect();
    let commit = Source::COMMIT_PRIORITY.iter().find_map(|s| {
        list.iter()
            .filter(|a| a.source == *s)
            .find_map(|a| a.commit.as_deref())
    });
    DecisionOut {
        id,
        option,
        commit,
        sources: sources.into_iter().collect(),
    }
}

/// The deadline an item carries: its deferral's when deferred (the later word), else its
/// triage's.
fn deadline(item: &Item) -> Option<&str> {
    item.deferral
        .as_ref()
        .map(|d| d.resolve_by.as_str())
        .or(item.resolve_by.as_deref())
        .map(str::trim)
}

/// What `item` holds up, read from its deadline: `before-merge` (with or without a `:<note>`)
/// is `merge`, `before-release` is `release`, a date is `none`, and `blocks:<id>` is that
/// item's, while it is open. Anything else, no deadline, a closed target or a cycle is
/// `unknown`. The raiser's free-text `blocks` field is not read: parsing prose would be a guess.
pub(crate) fn blocks_of(item: &Item, ledger: &Ledger) -> Blocks {
    let mut seen: HashSet<&ItemId> = HashSet::new();
    let mut current = item;
    loop {
        if !seen.insert(&current.id) {
            return Blocks::Unknown;
        }
        let Some(by) = deadline(current) else {
            return Blocks::Unknown;
        };
        if by == "before-merge" || by.starts_with("before-merge:") {
            return Blocks::Merge;
        }
        if by == "before-release" {
            return Blocks::Release;
        }
        if is_date(by) {
            return Blocks::None;
        }
        let target = by
            .strip_prefix("blocks:")
            .and_then(ItemId::parse)
            .and_then(|t| ledger.items.get(&t))
            .filter(|t| t.status.is_open());
        match target {
            Some(t) => current = t,
            None => return Blocks::Unknown,
        }
    }
}

/// `YYYY-MM-DD`, alone or followed by a time (`T…` or ` …`).
fn is_date(s: &str) -> bool {
    let Some((date, rest)) = s.as_bytes().split_first_chunk::<10>() else {
        return false;
    };
    let shape = date.iter().enumerate().all(|(n, b)| match n {
        4 | 7 => *b == b'-',
        _ => b.is_ascii_digit(),
    });
    shape && matches!(rest.first(), None | Some(b'T' | b' '))
}

/// The commit of each final line in `git blame --porcelain` output, by 1-based line number.
/// Lines not committed yet (the all-zero sha) are left out.
pub(crate) fn blame_lines(porcelain: &str) -> HashMap<u64, String> {
    porcelain
        .lines()
        .filter(|l| !l.starts_with('\t'))
        .filter_map(|l| {
            let mut words = l.split(' ');
            let sha = words.next()?;
            let is_sha = matches!(sha.len(), 40 | 64) && sha.bytes().all(|b| b.is_ascii_hexdigit());
            if !is_sha || sha.bytes().all(|b| b == b'0') {
                return None;
            }
            let line = words.nth(1)?.parse().ok()?;
            Some((line, sha.to_owned()))
        })
        .collect()
}

/// The text form: one summary line, then one line per violation.
///
/// # Errors
/// Fails when `out` cannot be written.
pub(crate) fn render_text(r: &CiReport<'_>, out: &mut impl Write) -> std::io::Result<()> {
    let count = |b: Blocks| r.open_items.iter().filter(|i| i.blocks == b).count();
    let journal = if r.journal.ok {
        "journal ok".to_owned()
    } else {
        format!("journal: {} violation(s)", r.journal.violations.len())
    };
    writeln!(
        out,
        "ci-check {}..{}: {} open ({} block merge, {} block release, {} unknown) · {} decision(s) · {journal}",
        short(r.base),
        short(r.head),
        r.open_items.len(),
        count(Blocks::Merge),
        count(Blocks::Release),
        count(Blocks::Unknown),
        r.decisions.len(),
    )?;
    for v in &r.journal.violations {
        let line = v.line.map_or_else(|| "-".to_owned(), |n| n.to_string());
        let reason = match v.reason {
            Reason::Removed => "removed",
            Reason::Missing => "missing",
            Reason::Altered => "altered",
            Reason::Undecodable => "undecodable",
        };
        writeln!(out, "  {}:{line} {reason}", v.path)?;
    }
    Ok(())
}

/// A sha's first seven characters, as git abbreviates it.
fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use weighin_core::EventEnvelope;

    const A: &str = "wi-0000a1";
    const B: &str = "wi-0000b2";
    const C: &str = "wi-0000c3";

    fn id(s: &str) -> ItemId {
        ItemId::parse(s).expect("a valid id")
    }

    fn raised(item: &str) -> String {
        format!(
            r#"{{"type":"item_raised","item_id":"{item}","kind":"question","question":"Question {item}?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"q","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:36Z"}}"#
        )
    }

    fn triaged(item: &str, resolve_by: &str) -> String {
        format!(
            r#"{{"type":"item_triaged","item_id":"{item}","door":"one-way","blast":"file","authority":"judgment","route":"present","resolve_by":"{resolve_by}","reason":"r","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}}"#
        )
    }

    fn deferred(item: &str, resolve_by: &str) -> String {
        format!(
            r#"{{"type":"item_deferred","item_id":"{item}","flip":"f","trigger":"t","resolve_by":"{resolve_by}","default_option":"A","owner":"ryan","reason":"","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:38Z"}}"#
        )
    }

    fn dismissed(item: &str) -> String {
        format!(
            r#"{{"type":"item_dismissed","item_id":"{item}","why":{{"kind":"moot"}},"reason":"r","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:39Z"}}"#
        )
    }

    fn reopened(item: &str) -> String {
        format!(
            r#"{{"type":"item_reopened","item_id":"{item}","reason":"r","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:40Z"}}"#
        )
    }

    fn ledger(lines: &[String]) -> Ledger {
        let events: Vec<EventEnvelope> = lines
            .iter()
            .map(|l| EventEnvelope::from_json(l.as_bytes()).expect("a test event"))
            .collect();
        Ledger::fold((1_u64..).zip(events.iter()))
    }

    fn blocks(lines: &[String], item: &str) -> Blocks {
        let l = ledger(lines);
        blocks_of(l.items.get(&id(item)).expect("the item"), &l)
    }

    #[test]
    fn an_untriaged_item_blocks_unknown() {
        assert_eq!(blocks(&[raised(A)], A), Blocks::Unknown);
    }

    #[test]
    fn each_triage_deadline_maps_to_its_blocks() {
        let cases = [
            ("before-merge", Blocks::Merge),
            ("before-merge:pr-12", Blocks::Merge),
            (" before-release ", Blocks::Release),
            ("2026-11-01", Blocks::None),
            ("2026-11-01T10:00:00Z", Blocks::None),
            ("2026-1-01", Blocks::Unknown),
            ("next sitting", Blocks::Unknown),
        ];

        let got = cases.map(|(by, _)| blocks(&[raised(A), triaged(A, by)], A));

        assert_eq!(got, cases.map(|(_, b)| b));
    }

    #[test]
    fn a_deferral_deadline_overrides_the_triage_deadline() {
        let lines = [
            raised(A),
            triaged(A, "before-merge"),
            deferred(A, "before-release"),
        ];

        assert_eq!(blocks(&lines, A), Blocks::Release);
    }

    #[test]
    fn blocks_on_an_open_item_inherit_its_blocks() {
        let lines = [
            raised(A),
            triaged(A, &format!("blocks:{B}")),
            raised(B),
            triaged(B, "before-merge"),
        ];

        assert_eq!(blocks(&lines, A), Blocks::Merge);
    }

    #[test]
    fn blocks_on_a_closed_item_or_a_cycle_are_unknown() {
        let closed = [
            raised(A),
            triaged(A, &format!("blocks:{B}")),
            raised(B),
            triaged(B, "before-merge"),
            dismissed(B),
        ];
        let cycle = [
            raised(A),
            triaged(A, &format!("blocks:{B}")),
            raised(B),
            triaged(B, &format!("blocks:{A}")),
        ];

        let got = [blocks(&closed, A), blocks(&cycle, A)];

        assert_eq!(got, [Blocks::Unknown, Blocks::Unknown]);
    }

    fn attested(
        item: &str,
        option: Option<&str>,
        source: Source,
        commit: Option<&str>,
    ) -> Attested {
        Attested {
            id: id(item),
            option: option.map(str::to_owned),
            source,
            commit: commit.map(str::to_owned),
        }
    }

    fn timeline(entries: &[(&str, When)]) -> BTreeMap<ItemId, When> {
        entries.iter().map(|(s, w)| (id(s), *w)).collect()
    }

    fn report(
        ledger: &Ledger,
        closing: &[(&str, When)],
        reopens: &[(&str, When)],
        attested: &[Attested],
    ) -> serde_json::Value {
        let (closing, reopens) = (timeline(closing), timeline(reopens));
        let (base, head) = ("b".repeat(40), "h".repeat(40));
        let r = build(Inputs {
            base: &base,
            head: &head,
            ledger,
            closing: &closing,
            reopens: &reopens,
            attested,
            violations: &[],
        });
        serde_json::to_value(&r).expect("the report serialises")
    }

    fn open_ids(r: &serde_json::Value) -> Vec<&str> {
        r["open_items"]
            .as_array()
            .expect("a list")
            .iter()
            .filter_map(|i| i["id"].as_str())
            .collect()
    }

    #[test]
    fn an_item_closed_by_an_atom_is_not_open_unless_the_journal_reopened_it() {
        let l = ledger(&[raised(A), raised(B), dismissed(B), reopened(B), raised(C)]);

        let r = report(
            &l,
            &[(A, When::Base), (B, When::Base)],
            &[(B, When::Base)],
            &[],
        );

        assert_eq!(open_ids(&r), [B, C]);
    }

    #[test]
    fn a_reopen_before_the_range_does_not_outrank_a_trailer_in_it() {
        let l = ledger(&[raised(A), dismissed(A), reopened(A)]);

        let r = report(&l, &[(A, When::Commit(0))], &[(A, When::Base)], &[]);

        assert_eq!(open_ids(&r), Vec::<&str>::new());
    }

    #[test]
    fn a_reopen_keeps_an_item_open_only_when_newer_than_what_closed_it() {
        let cases = [
            (None, None, true),
            (Some(When::Base), None, false),
            (Some(When::Base), Some(When::Base), true),
            (Some(When::Commit(1)), Some(When::Base), false),
            (Some(When::Commit(1)), Some(When::Commit(1)), false),
            (Some(When::Commit(1)), Some(When::Commit(2)), true),
            (Some(When::Commit(1)), Some(When::Uncommitted), true),
            (Some(When::Uncommitted), Some(When::Commit(2)), false),
        ];

        let got = cases.map(|(closing, reopen, _)| still_open(closing, reopen));

        assert_eq!(got, cases.map(|(_, _, open)| open));
    }

    #[test]
    fn an_open_item_reports_its_triage_not_its_hints() {
        let l = ledger(&[raised(A), triaged(A, "before-merge")]);

        let r = report(&l, &[], &[], &[]);

        let expected = json!([{
            "id": A, "status": "triaged", "kind": "question", "door": "one-way",
            "authority": "judgment", "blocks": "merge", "resolve_by": "before-merge",
            "question": "Question wi-0000a1?"
        }]);
        assert_eq!(r["open_items"], expected);
    }

    #[test]
    fn an_option_less_attestation_joins_the_one_entry_for_its_item() {
        let attested = vec![
            attested(A, None, Source::Atom, Some("atom-sha")),
            attested(A, Some("B"), Source::Trailer, Some("trailer-sha")),
            attested(A, Some("B"), Source::Journal, None),
        ];

        let r = report(&Ledger::default(), &[], &[], &attested);

        let expected = json!([
            {"id": A, "option": "B", "commit": "trailer-sha", "sources": ["journal", "atom", "trailer"]}
        ]);
        assert_eq!(r["decisions"], expected);
    }

    #[test]
    fn sources_that_disagree_give_one_entry_per_option_in_order() {
        let attested = vec![
            attested(B, Some("A"), Source::Trailer, Some("t2")),
            attested(A, Some("Z"), Source::Journal, Some("j1")),
            attested(A, Some("Y"), Source::Trailer, Some("t1")),
            attested(A, None, Source::Atom, None),
        ];

        let r = report(&Ledger::default(), &[], &[], &attested);

        let expected = json!([
            {"id": A, "option": null, "commit": null, "sources": ["atom"]},
            {"id": A, "option": "Y", "commit": "t1", "sources": ["trailer"]},
            {"id": A, "option": "Z", "commit": "j1", "sources": ["journal"]},
            {"id": B, "option": "A", "commit": "t2", "sources": ["trailer"]}
        ]);
        assert_eq!(r["decisions"], expected);
    }

    #[test]
    fn blame_lines_map_final_line_numbers_to_committed_shas() {
        let sha = "1".repeat(40);
        let zero = "0".repeat(40);
        let porcelain = format!(
            "{sha} 1 1 2\nauthor t\nfilename j.jsonl\n\tline one\n{sha} 2 2\n\tline two\n{zero} 3 3 1\nauthor Not Committed Yet\nfilename j.jsonl\n\tline three\n"
        );

        let lines = blame_lines(&porcelain);

        let expected: HashMap<u64, String> = [(1, sha.clone()), (2, sha)].into();
        assert_eq!(lines, expected);
    }

    #[test]
    fn the_text_form_is_one_summary_line_and_one_line_per_violation() -> std::io::Result<()> {
        let l = ledger(&[raised(A), triaged(A, "before-merge"), raised(B)]);
        let none = BTreeMap::new();
        let attested = [attested(C, Some("A"), Source::Trailer, Some("t"))];
        let violations = [Violation {
            path: "j.jsonl".to_owned(),
            line: Some(2),
            reason: Reason::Missing,
        }];
        let r = build(Inputs {
            base: "abcdef0123",
            head: "0123456789",
            ledger: &l,
            closing: &none,
            reopens: &none,
            attested: &attested,
            violations: &violations,
        });
        let mut out = Vec::new();

        render_text(&r, &mut out)?;

        assert_eq!(
            String::from_utf8_lossy(&out),
            "ci-check abcdef0..0123456: 2 open (1 block merge, 0 block release, 1 unknown) · 1 decision(s) · journal: 1 violation(s)\n  j.jsonl:2 missing\n"
        );
        Ok(())
    }
}
