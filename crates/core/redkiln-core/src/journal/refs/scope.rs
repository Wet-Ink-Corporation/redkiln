//! Checkpoints scoped by a store position (SPEC JR-08): which journals they examine, in what
//! order, and how far they settle the store.

use std::collections::HashSet;

use super::checkpoint::{RefsCheckpoint, RefsExported, refs_of};
use super::{RefsError, RefsTarget};
use crate::journal::export::{Group, grouped};
use crate::journal::name::JournalName;
use crate::store::Store;
use crate::suite::RepoClass;

/// Which journals a scoped checkpoint examines first.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefsScope<'a> {
    /// The store position examined after: a journal whose last event is at or before it is
    /// settled already. 0 is none.
    pub since: u64,
    /// A journal examined, and written, before any other, when it is in scope: the caller's
    /// current work, which then never waits behind older journals.
    pub first: Option<&'a JournalName>,
}

impl RefsScope<'_> {
    /// The scope after `since`, with no journal first.
    #[must_use]
    pub const fn since(since: u64) -> Self {
        Self { since, first: None }
    }
}

/// A checkpoint scoped by [`checkpoint_refs_since`], and how far it read the store.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub struct ScopedCheckpoint {
    /// What the checkpoint did.
    pub outcome: RefsCheckpoint,
    /// How far the store is settled: every journal whose last event is at or before this
    /// position was examined and neither deferred nor broken (or is at or before `since`).
    /// That is the last position read when nothing was deferred or broken, else the position
    /// just before the first such journal's last event, journals taken oldest last event
    /// first, whatever order they were examined in. A later checkpoint scoped since it goes on
    /// from there. 0 in a `client` repository, or when nothing is settled.
    pub through: u64,
}

/// [`super::checkpoint_refs`], examining only the journals with an event after `scope.since`.
///
/// The journals whose last event is at or before it are neither read nor written; `since` 0
/// examines every journal. `scope.first` goes first, then the others oldest last event first.
/// A caller that passes an earlier checkpoint's `through` examines only what is not settled
/// yet; a full checkpoint now and then still catches a ref moved by anyone else.
///
/// # Errors
/// As [`super::checkpoint_refs`].
pub fn checkpoint_refs_since(
    store: &Store,
    class: RepoClass,
    target: &RefsTarget<'_>,
    scope: RefsScope<'_>,
) -> Result<ScopedCheckpoint, RefsError> {
    if class == RepoClass::Client {
        return Ok(client());
    }
    scoped(store, target, scope, Older::Skipped)
}

/// [`checkpoint_refs_since`], then every journal at or before `scope.since` too: a sweep that
/// writes new work before it checks history again.
///
/// `scope.first` goes first, then the journals past `since`, oldest last event first, then the
/// others in the same order. `through` is settled over the journals past `since` only, so it
/// is never below `since` whatever the older journals give: those are reported, and a later
/// sweep tries them again.
///
/// # Errors
/// As [`super::checkpoint_refs`].
pub fn sweep_refs_since(
    store: &Store,
    class: RepoClass,
    target: &RefsTarget<'_>,
    scope: RefsScope<'_>,
) -> Result<ScopedCheckpoint, RefsError> {
    if class == RepoClass::Client {
        return Ok(client());
    }
    scoped(store, target, scope, Older::ExaminedAfter)
}

/// A `client` repository's checkpoint: nothing written, nothing settled (JR-09).
const fn client() -> ScopedCheckpoint {
    ScopedCheckpoint {
        outcome: RefsCheckpoint::Client,
        through: 0,
    }
}

/// What a scoped checkpoint does with the journals whose last event is at or before `since`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Older {
    Skipped,
    ExaminedAfter,
}

fn scoped(
    store: &Store,
    target: &RefsTarget<'_>,
    scope: RefsScope<'_>,
    older: Older,
) -> Result<ScopedCheckpoint, RefsError> {
    let grouped = grouped(store).map_err(RefsError::Store)?;
    let (mut newer, mut earlier): (Vec<(JournalName, Group)>, Vec<_>) = grouped
        .journals
        .into_iter()
        .partition(|(_, group)| group.last > scope.since);
    // Oldest last event first, so a checkpoint cut short still settles a prefix of positions
    // and the next one, scoped since it, goes on from there.
    newer.sort_by_key(|(_, group)| group.last);
    // The names go to the writers and into the report; this copy maps them back to positions.
    let positions: Vec<(JournalName, u64)> = newer
        .iter()
        .map(|(name, group)| (name.clone(), group.last))
        .collect();
    if older == Older::ExaminedAfter {
        earlier.sort_by_key(|(_, group)| group.last);
        newer.append(&mut earlier);
    }
    // Journal names are unique, so `first` holds the one journal `scope.first` names, if any.
    let (first, rest): (Vec<_>, Vec<_>) = newer
        .into_iter()
        .partition(|(name, _)| scope.first == Some(name));
    let lines = |(name, group): (JournalName, Group)| (name, group.lines);
    let report = refs_of(
        target,
        [first.into_iter().map(lines), rest.into_iter().map(lines)],
        grouped.unexportable,
    )?;
    let through = settled(&positions, &report).unwrap_or(grouped.through);
    Ok(ScopedCheckpoint {
        outcome: RefsCheckpoint::Written(report),
        through,
    })
}

/// The position just before the last event of the first journal of `positions` that `report`
/// deferred or found broken; `None` when it deferred and broke none.
fn settled(positions: &[(JournalName, u64)], report: &RefsExported) -> Option<u64> {
    let unsettled: HashSet<&JournalName> = report
        .deferred
        .iter()
        .map(|(name, _)| name)
        .chain(report.broken.iter().map(|(name, _)| name))
        .collect();
    positions
        .iter()
        .find(|(name, _)| unsettled.contains(name))
        .map(|(_, last)| last.saturating_sub(1))
}
