//! The journal sources a clone holds (SPEC JR-05, JR-07): its journal refs, inbox copies and
//! broken refs, read through Core's [`read_local`] with no fetch and no store. Each complete
//! line is mapped as `journal import` would store it ([`from_line`]); each journal ref's JR-05
//! flaws, each ref Core could not read whole and each malformed line become `ref` violations.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use anyhow::{Context as _, Result};
use redkiln_core::EventId;
use redkiln_core::journal::lines;
use redkiln_core::journal::refs::{Breach, Flaw, LocalFile, LocalSource, SourceKind, read_local};
use redkiln_core::location::GIT_TIMEOUT;
use weighin_contract::ci_check::v2::{JournalKind, Reason, RefKind, RefRead, Violation};
use weighin_core::EventEnvelope;

use crate::store::{LineSkip, from_line};

/// What the clone's journal sources hold.
#[derive(Debug, Default)]
pub(super) struct RefJournals {
    /// Their events, in Core's source order, then file and line order.
    pub(super) events: Vec<EventEnvelope>,
    /// Their flaws, refs not read whole and malformed lines.
    pub(super) violations: Vec<Violation>,
    /// Each source read.
    pub(super) read: Vec<RefRead>,
    /// Weighin events this build cannot read, by type (EV-10), each event counted once.
    pub(super) unreadable: BTreeMap<String, u64>,
    /// The identities of the events counted in `unreadable`: one event may sit in several
    /// sources (an inbox copy and its ref) or files (a rewritten predecessor and the tip).
    counted: HashSet<EventId>,
}

/// Every local journal source of the repository at `root`.
///
/// # Errors
/// git cannot run or fails at a local step, or prints something Core cannot read.
pub(super) fn read(root: &Path) -> Result<RefJournals> {
    let mut found = RefJournals::default();
    let local = read_local(root, GIT_TIMEOUT, |file| decode(file, &mut found))
        .context("reading the journal refs")?;
    for source in local.sources {
        let (read, violations) = split(source)?;
        found.violations.extend(violations);
        found.read.push(read);
    }
    found
        .violations
        .extend(local.passed_over.into_iter().map(passed_over));
    Ok(found)
}

/// The complete, non-blank lines of `file` into `found`: events, unreadable counts and
/// `undecodable` violations. A last line with no `\n` is passed over (JR-03); Core already
/// reports its file as unended.
fn decode(file: LocalFile<'_>, found: &mut RefJournals) {
    for line in lines(file.bytes).filter(|l| l.complete && !l.is_blank()) {
        match from_line(line.number, line.text) {
            Ok(event) => found.events.push(event),
            Err(LineSkip::Foreign) => {}
            Err(LineSkip::Unusable { origin, event_type }) => {
                if found.counted.insert(origin) {
                    let count = found.unreadable.entry(event_type).or_default();
                    *count = count.saturating_add(1);
                }
            }
            Err(LineSkip::Malformed) => found.violations.push(Violation {
                journal: file.refname.to_owned(),
                kind: JournalKind::Ref,
                commit: Some(file.commit.to_owned()),
                line: Some(line.number),
                reason: Reason::Undecodable,
            }),
        }
    }
}

/// `source` as the report lists it, and its JR-05 flaws as violations, oldest first.
///
/// # Errors
/// Its commit count does not fit a `u64`.
fn split(source: LocalSource) -> Result<(RefRead, Vec<Violation>)> {
    let LocalSource {
        refname,
        kind,
        tip,
        commits,
        breaks,
    } = source;
    let violations = breaks
        .into_iter()
        .map(|b| Violation {
            // Each violation names its ref: the name is copied per flaw.
            journal: refname.clone(),
            kind: JournalKind::Ref,
            commit: Some(b.commit),
            line: None,
            reason: reason_of(b.flaw),
        })
        .collect();
    let read = RefRead {
        refname,
        kind: match kind {
            SourceKind::Journal => RefKind::Journal,
            SourceKind::Inbox => RefKind::Inbox,
            SourceKind::Broken => RefKind::Broken,
        },
        tip,
        commits: u64::try_from(commits).context("counting a ref's commits")?,
    };
    Ok((read, violations))
}

/// The reason a flaw is reported under; exhaustive, so a new Core breach is a compile error.
const fn reason_of(flaw: Flaw) -> Reason {
    match flaw {
        Flaw::Breach(Breach::Merge) => Reason::Merge,
        Flaw::Breach(Breach::Tree) => Reason::Tree,
        Flaw::Breach(Breach::Unended) => Reason::Unended,
        Flaw::Breach(Breach::TooLarge) => Reason::TooLarge,
        Flaw::Rewritten => Reason::Rewritten,
    }
}

/// A ref, or a ref prefix, Core could not read whole.
fn passed_over(refname: String) -> Violation {
    Violation {
        journal: refname,
        kind: JournalKind::Ref,
        commit: None,
        line: None,
        reason: Reason::Unreadable,
    }
}

#[cfg(test)]
mod tests;
