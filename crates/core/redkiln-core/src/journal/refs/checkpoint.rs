//! The export checkpoint onto journal refs (SPEC JR-07 to JR-09).
//!
//! It first asks, with no network, which journals hold a durable event that neither the clone's
//! copy of the journal ref nor its copy of the inbox branch holds; a checkpoint with nothing new
//! touches no remote. Then it fetches the journal refs once and writes each such journal: direct
//! first, and after any failure other than a rejection, through the inbox for the rest of the
//! checkpoint. With no remote configured, it moves the local refs only.

use super::direct::{Direct, Inbox, Inboxed, merge_direct, prepare_relay, write_inbox};
use super::git::Fetched;
use super::{
    Breach, Ctx, InboxCopy, JOURNAL_REFS, RefsError, RefsTarget, State, read_inbox, read_journal,
};
use crate::journal::export::grouped;
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
    /// Events that are not envelopes, so have no line. Import records, which are never
    /// exported, are not counted.
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

/// Exports every durable event of `store` that its journal lacks into the journal refs of
/// `target` (JR-07, JR-08); in a `client` repository, writes nothing (JR-09).
///
/// Import records are not exported: they stay in the store (JR-13). No journal is started after
/// the target's deadline; those left are deferred.
///
/// # Errors
/// [`RefsError`] when the store cannot be read, or git cannot run or fails at a local step. A
/// journal ref or inbox that cannot be read whole is reported in the outcome, not as an error.
pub fn checkpoint_refs(
    store: &Store,
    class: RepoClass,
    target: &RefsTarget<'_>,
) -> Result<RefsCheckpoint, RefsError> {
    if class == RepoClass::Client {
        return Ok(RefsCheckpoint::Client);
    }
    let (journals, unexportable) = grouped(store).map_err(RefsError::Store)?;
    let mut ctx = Ctx::local(target);
    let mut report = RefsExported {
        unexportable,
        ..RefsExported::default()
    };
    let mut pending = Vec::new();
    for (name, own) in journals {
        if lacks(&ctx, &name, &own)? {
            pending.push((name, own));
        }
    }
    if pending.is_empty() {
        return Ok(RefsCheckpoint::Written(report));
    }
    if ctx.late() {
        report.deferred = late(pending);
        return Ok(RefsCheckpoint::Written(report));
    }
    if ctx.repo.has_remote(target.remote)? {
        ctx.remote = Some(target.remote);
        let all = format!("+{JOURNAL_REFS}*:{JOURNAL_REFS}*");
        if let Fetched::Failed(said) = ctx.repo.fetch(target.remote, &[all], false)? {
            // Each deferred journal carries its own copy of the reason.
            report.deferred = pending
                .into_iter()
                .map(|(name, _)| (name, Deferral::Failed(said.clone())))
                .collect();
            return Ok(RefsCheckpoint::Written(report));
        }
    } else {
        report.local_only = true;
    }
    export(&ctx, &mut report, pending)?;
    Ok(RefsCheckpoint::Written(report))
}

/// Writes each pending journal directly until a push fails other than by rejection, then the
/// rest to their inbox branches.
fn export(
    ctx: &Ctx<'_>,
    report: &mut RefsExported,
    pending: Vec<(JournalName, Vec<u8>)>,
) -> Result<(), RefsError> {
    let mut base = None;
    for (name, own) in pending {
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
            let tip = match &base {
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

/// Whether `own` holds a line that neither the clone's journal ref nor its inbox copy holds.
fn lacks(ctx: &Ctx<'_>, name: &JournalName, own: &[u8]) -> Result<bool, RefsError> {
    let journal = match read_journal(ctx, name)? {
        State::Good { file, .. } => file,
        State::Absent | State::Broken(_) => Vec::new(),
    };
    let known = match read_inbox(ctx, name)? {
        InboxCopy::Read { file, .. } => merge(&journal, &file).bytes,
        // An inbox too large to read holds nothing this check can count on.
        InboxCopy::Absent | InboxCopy::TooLarge => journal,
    };
    Ok(merge(&known, own).added > 0)
}
