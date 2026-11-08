//! The export checkpoint onto journal refs (SPEC JR-07 to JR-09).
//!
//! It exports every durable event except import records. It first asks, with no network, which
//! journals hold such an event that neither the clone's copy of the journal ref nor its copy of
//! the inbox branch holds; a checkpoint with nothing new touches no remote. Then it fetches the
//! journal refs once and writes each such journal: direct first, and after any failure other
//! than a rejection, through the inbox for the rest of the checkpoint. With no remote
//! configured, it moves the local refs only. A scoped checkpoint (`scope.rs`) runs its first
//! journal through all of that before it examines the others.

use super::direct::{Direct, Inbox, Inboxed, merge_direct, prepare_relay, write_inbox};
use super::git::Fetched;
use super::oid::Oid;
use super::scope::{RefsScope, checkpoint_refs_since};
use super::{
    Breach, Ctx, InboxCopy, JOURNAL_REFS, RefsError, RefsTarget, State, read_inbox, read_journal,
};

use crate::journal::merge::merge;
use crate::journal::name::JournalName;
use crate::store::Store;
use crate::suite::RepoClass;

/// What a checkpoint onto journal refs did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum RefsCheckpoint {
    /// A `client` repository: nothing was written (JR-09, ST-13).
    Client,
    /// The journals that needed it were written, but for those listed as broken or deferred.
    Written(RefsExported),
}

/// The journals a checkpoint wrote, and those it could not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RefsExported {
    /// Lines the checkpoint's journals gained, over every journal.
    pub appended: u64,
    /// Journal refs moved: pushed, or, with no remote, moved locally.
    pub direct: Vec<JournalName>,
    /// Inbox branches pushed for the relay.
    pub relayed: Vec<JournalName>,
    /// Journal refs left alone because they break JR-05.
    pub broken: Vec<(JournalName, Breach)>,
    /// Journals not written this time; their events stay in the store for the next checkpoint.
    pub deferred: Vec<(JournalName, Deferral)>,
    /// Why the direct way was given up for the inbox, when it was.
    pub relay_reason: Option<String>,
    /// No remote is configured, so only local refs moved.
    pub local_only: bool,
    /// Events left out because they are not envelopes, so have no line. Import records are
    /// left out too (JR-13), and are not counted here.
    pub unexportable: u64,
}

/// Why a journal was not written this time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deferral {
    /// The checkpoint's deadline passed first.
    Late,
    /// Every retry of a rejected push was rejected.
    RetriesSpent,
    /// The remote default branch already holds `.redkiln/inbox/<name>.jsonl`.
    InboxOccupied,
    /// The inbox branch's file is larger than [`crate::journal::MAX_JOURNAL_BYTES`], so it
    /// cannot be merged; the relay refuses it too, and a person deletes it.
    InboxTooLarge,
    /// Merging the checkpoint's lines would make the journal larger than
    /// [`crate::journal::MAX_JOURNAL_BYTES`], so nothing was written.
    TooLarge,
    /// A fetch or push failed, with what git said.
    Failed(String),
}

/// The most git calls a checkpoint makes after its target's deadline passes.
///
/// No journal, push or fetch starts after it, and no run of calls between two checks of it is
/// longer than this (reading a journal ref: seven calls; its inbox copy: six). Each call is
/// bounded by the target's `timeout`, so a checkpoint ends by `deadline +
/// MAX_CALLS_PAST_DEADLINE × timeout`, plus its store reads.
pub const MAX_CALLS_PAST_DEADLINE: u32 = 7;

/// Exports every durable event of `store` that its journal lacks into the journal refs of
/// `target` (JR-07, JR-08); in a `client` repository, writes nothing (JR-09).
///
/// Import records are not exported: they stay in the store (JR-13). No journal is examined or
/// started after the target's deadline; those left are deferred.
///
/// # Errors
/// [`RefsError`] when the store cannot be read, or git cannot run or fails at a local step. A
/// journal ref or inbox that cannot be read whole is reported in the outcome, not as an error.
pub fn checkpoint_refs(
    store: &Store,
    class: RepoClass,
    target: &RefsTarget<'_>,
) -> Result<RefsCheckpoint, RefsError> {
    checkpoint_refs_since(store, class, target, RefsScope::since(0)).map(|scoped| scoped.outcome)
}

/// Writes each journal of each of `batches` that its journal ref lacks, a batch at a time: every
/// journal of a batch is examined and written before the next batch is examined, so the first
/// batch never waits for a long scan of the others.
pub(super) fn refs_of<J: Iterator<Item = (JournalName, Vec<u8>)>>(
    target: &RefsTarget<'_>,
    batches: impl IntoIterator<Item = J>,
    unexportable: u64,
) -> Result<RefsExported, RefsError> {
    let mut run = Run {
        ctx: Ctx::local(target),
        remote: target.remote,
        fetched: Fetch::NotYet,
        base: None,
        report: RefsExported {
            unexportable,
            ..RefsExported::default()
        },
    };
    for batch in batches {
        run.batch(batch)?;
    }
    Ok(run.report)
}

/// One checkpoint's state across its batches.
struct Run<'a> {
    ctx: Ctx<'a>,
    /// The target's remote, set on `ctx` once it is known to be configured.
    remote: &'a str,
    fetched: Fetch,
    /// The relay's base, fetched once a checkpoint, when a journal first goes to the inbox.
    base: Option<Result<Oid, String>>,
    report: RefsExported,
}

/// Whether a checkpoint has fetched the journal refs yet.
enum Fetch {
    NotYet,
    /// Fetched, or no remote is configured.
    Done,
    /// The fetch failed, with what git said: every journal left to write is deferred.
    Failed(String),
}

impl Run<'_> {
    /// Examines `journals` and writes those their journal ref lacks; once the deadline passes,
    /// every journal not examined or started is deferred.
    fn batch(
        &mut self,
        mut journals: impl Iterator<Item = (JournalName, Vec<u8>)>,
    ) -> Result<(), RefsError> {
        let mut pending = Vec::new();
        while let Some((name, own)) = journals.next() {
            let lacking = if self.ctx.late() {
                None
            } else {
                lacks(&self.ctx, &name, &own)?
            };
            match lacking {
                Some(true) => pending.push((name, own)),
                Some(false) => {}
                None => {
                    // Not examined in time: each is deferred, whether it lacks a line or not.
                    pending.push((name, own));
                    pending.extend(journals);
                    self.report.deferred.extend(late(pending));
                    return Ok(());
                }
            }
        }
        if pending.is_empty() {
            return Ok(());
        }
        if self.ctx.late() {
            self.report.deferred.extend(late(pending));
            return Ok(());
        }
        self.fetch()?;
        if let Fetch::Failed(said) = &self.fetched {
            // Each deferred journal carries its own copy of the reason.
            self.report.deferred.extend(
                pending
                    .into_iter()
                    .map(|(name, _)| (name, Deferral::Failed(said.clone()))),
            );
            return Ok(());
        }
        export(&self.ctx, &mut self.report, &mut self.base, pending)
    }

    /// Fetches the journal refs from the remote, once a checkpoint; with no remote configured,
    /// only local refs move from then on.
    fn fetch(&mut self) -> Result<(), RefsError> {
        if !matches!(self.fetched, Fetch::NotYet) {
            return Ok(());
        }
        if !self.ctx.repo.has_remote(self.remote)? {
            self.report.local_only = true;
            self.fetched = Fetch::Done;
            return Ok(());
        }
        self.ctx.remote = Some(self.remote);
        let all = format!("+{JOURNAL_REFS}*:{JOURNAL_REFS}*");
        self.fetched = match self.ctx.repo.fetch(self.remote, &[all], false)? {
            Fetched::Failed(said) => Fetch::Failed(said),
            Fetched::Done | Fetched::Missing => Fetch::Done,
        };
        Ok(())
    }
}

/// Writes each pending journal directly until a push fails other than by rejection, then the
/// rest to their inbox branches, on the relay's `base`, fetched here the first time it is needed.
fn export(
    ctx: &Ctx<'_>,
    report: &mut RefsExported,
    base: &mut Option<Result<Oid, String>>,
    pending: Vec<(JournalName, Vec<u8>)>,
) -> Result<(), RefsError> {
    for (name, own) in pending {
        // No journal starts after the deadline: a relay write would read the base first.
        if ctx.late() {
            report.deferred.push((name, Deferral::Late));
            continue;
        }
        let name = if report.relay_reason.is_none() {
            match direct(ctx, report, name, &own)? {
                Some(name) => name,
                None => continue,
            }
        } else {
            name
        };
        if let Some(remote) = ctx.remote {
            // The relay's base is fetched only once, and not after the deadline.
            if base.is_none() && ctx.late() {
                report.deferred.push((name, Deferral::Late));
                continue;
            }
            let tip = match base {
                Some(tip) => tip,
                None => base.insert(prepare_relay(ctx, remote)?),
            };
            let inbox = match tip {
                Ok(base) => Ok(Inbox { remote, base }),
                Err(said) => Err(said.as_str()),
            };
            relayed(ctx, inbox, report, (name, &own))?;
        }
    }
    Ok(())
}

/// Every journal of `pending`, deferred as started after the deadline.
fn late(pending: Vec<(JournalName, Vec<u8>)>) -> Vec<(JournalName, Deferral)> {
    pending
        .into_iter()
        .map(|(name, _)| (name, Deferral::Late))
        .collect()
}

/// Writes `name` directly, recording the outcome; the name back when the direct way failed and
/// the journal goes to the relay instead (the reason is then in the report).
fn direct(
    ctx: &Ctx<'_>,
    report: &mut RefsExported,
    name: JournalName,
    own: &[u8],
) -> Result<Option<JournalName>, RefsError> {
    match merge_direct(ctx, &name, own)? {
        Direct::UpToDate => {}
        Direct::Moved(added) => {
            report.appended = report.appended.saturating_add(added);
            report.direct.push(name);
        }
        Direct::Broken(breach) => report.broken.push((name, breach)),
        Direct::TooLarge => report.deferred.push((name, Deferral::TooLarge)),
        Direct::Spent => report.deferred.push((name, Deferral::RetriesSpent)),
        Direct::Late => report.deferred.push((name, Deferral::Late)),
        Direct::Failed(said) => {
            report.relay_reason = Some(said);
            return Ok(Some(name));
        }
    }
    Ok(None)
}

/// Writes `name`'s inbox branch on `base`, recording the outcome.
fn relayed(
    ctx: &Ctx<'_>,
    inbox: Result<Inbox<'_>, &str>,
    report: &mut RefsExported,
    (name, own): (JournalName, &[u8]),
) -> Result<(), RefsError> {
    let inbox = match inbox {
        Ok(inbox) => inbox,
        Err(said) => {
            report
                .deferred
                .push((name, Deferral::Failed(said.to_owned())));
            return Ok(());
        }
    };
    let deferral = match write_inbox(ctx, inbox, &name, own)? {
        Inboxed::Same => return Ok(()),
        Inboxed::Pushed(added) => {
            report.appended = report.appended.saturating_add(added);
            report.relayed.push(name);
            return Ok(());
        }
        Inboxed::Occupied => Deferral::InboxOccupied,
        Inboxed::InboxTooLarge => Deferral::InboxTooLarge,
        Inboxed::TooLarge => Deferral::TooLarge,
        Inboxed::Spent => Deferral::RetriesSpent,
        Inboxed::Late => Deferral::Late,
        Inboxed::Failed(said) => Deferral::Failed(said),
    };
    report.deferred.push((name, deferral));
    Ok(())
}

/// Whether `own` holds a line that neither the clone's journal ref nor its inbox copy holds;
/// `None` when the deadline passed between the two reads.
fn lacks(ctx: &Ctx<'_>, name: &JournalName, own: &[u8]) -> Result<Option<bool>, RefsError> {
    let journal = match read_journal(ctx, name)? {
        State::Good { file, .. } => file,
        State::Absent | State::Broken(_) => Vec::new(),
    };
    if ctx.late() {
        return Ok(None);
    }
    let known = match read_inbox(ctx, name)? {
        InboxCopy::Read { file, .. } => merge(&journal, &file).bytes,
        // An inbox too large to read holds nothing this check can count on.
        InboxCopy::Absent | InboxCopy::TooLarge => journal,
    };
    Ok(Some(merge(&known, own).added > 0))
}
