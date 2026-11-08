//! Weigh-In 1.x records mapped for Core's import (Move 1 step 8, SPEC ST-15, JR-07): a 1.x
//! ledger's events under their own identities, and 1.x journal lines under the identity M1-A
//! gives them ([`identity`]).
//!
//! Each record becomes a `weighin.<type>` envelope, version 1, written by this weighin
//! ([`wire::to_new_event`], EV-01) with its 1.x `worktree` dropped, and goes through
//! [`import_mapped`], which stores it as a journal line is stored (`asserted`, JR-12). A record
//! that cannot be mapped is skipped and reported, never fatal (JR-10). Records are produced
//! lazily, so memory is the source and one batch. This mapping moves to `weighin-contract` with
//! [`wire`] at step 5.

pub(crate) mod identity;

use std::collections::HashMap;
use std::time::Instant;

use anyhow::{Context as _, Result, anyhow};
use redkiln_core::EventId;
use redkiln_core::envelope::{Skip, Writer};
use redkiln_core::journal::{
    LineError, MAX_LINE_BYTES, MAX_PROBLEMS, Mapped, MappedImport, Recording, import_mapped, lines,
};
use redkiln_core::store::Stored;
use weighin_core::EventEnvelope;

use self::identity::Canonical;
use super::{PAGE, Store, wire};
use crate::text::Report;

/// Weigh-In events the store holds, by canonical form, with their identity: where M1-A's "an
/// event imported from a ledger" is looked up (decision H8: every weighin event the store holds).
#[derive(Debug, Default)]
pub(crate) struct Held(HashMap<Canonical, EventId>);

impl Held {
    /// The identity of the held event whose canonical form is `canonical`.
    fn get(&self, canonical: &Canonical) -> Option<EventId> {
        self.0.get(canonical).copied()
    }
}

/// How one old source is imported: the record it gets (JR-13), and the deadline after which no
/// batch is stored (`None`: none).
#[derive(Debug)]
pub(crate) struct Import {
    pub(crate) recording: Recording,
    pub(crate) deadline: Option<Instant>,
}

/// What importing one old source did: Core's outcome, and why each record was skipped as
/// malformed (the first [`MAX_PROBLEMS`], in record order).
#[derive(Debug)]
pub(crate) struct OldImport {
    pub(crate) outcome: MappedImport,
    pub(crate) problems: Vec<(u64, String)>,
}

impl Store {
    /// Every weighin event in the store, by canonical form (its `worktree` dropped); the first of
    /// two with one form is kept. One this build cannot use is left out (EV-10).
    ///
    /// # Errors
    /// A store error, or an event weighin-core cannot encode.
    pub(crate) fn held(&self) -> Result<Held> {
        let filter = wire::filter()?;
        let mut held = HashMap::new();
        let mut after = 0;
        loop {
            let page = self
                .inner
                .read(&filter, after, PAGE)
                .context("reading the store")?;
            for stored in &page {
                after = stored.position;
                match wire::from_stored(stored) {
                    Ok(env) => {
                        let (canonical, _) = Canonical::of(env).context("encoding a held event")?;
                        held.entry(canonical).or_insert(stored.id);
                    }
                    Err(Skip::Unusable(_) | Skip::Foreign) => {}
                }
            }
            if page.len() < PAGE.get() {
                return Ok(Held(held));
            }
        }
    }

    /// The events of a 1.x ledger Core read from a copy, each under its own identity (ST-15),
    /// imported as `import` says.
    ///
    /// # Errors
    /// Core's import failed ([`redkiln_core::journal::ImportError`]).
    pub(crate) fn import_ledger(&self, events: &[Stored], import: Import) -> Result<OldImport> {
        let writer = &self.writer;
        let records = events
            .iter()
            .map(|stored| (stored.position, ledger_record(stored, writer)));
        self.import_records(records, import)
    }

    /// The lines of one 1.x journal (`bytes`, its whole file), each under the identity M1-A
    /// gives it against `held`, imported as `import` says; blank lines skipped, a torn last line
    /// passed over (JR-03).
    ///
    /// # Errors
    /// Core's import failed ([`redkiln_core::journal::ImportError`]).
    pub(crate) fn import_journal(
        &self,
        bytes: &[u8],
        held: &Held,
        import: Import,
    ) -> Result<OldImport> {
        let writer = &self.writer;
        let records = lines(bytes).filter_map(|line| {
            journal_record(line.text, line.complete, held, writer).map(|r| (line.number, r))
        });
        self.import_records(records, import)
    }

    /// `records` through Core's import, each one that could not be mapped reported.
    fn import_records(
        &self,
        records: impl Iterator<Item = (u64, Result<Mapped>)>,
        import: Import,
    ) -> Result<OldImport> {
        let Import {
            recording,
            deadline,
        } = import;
        let mut problems = Vec::new();
        let records = Reported {
            records,
            problems: &mut problems,
        };
        let outcome = import_mapped(&self.inner, records, recording, deadline)
            .context("importing into the store")?;
        let report = match &outcome {
            MappedImport::Completed(report) | MappedImport::Stopped(report) => report,
        };
        // Core names why it refused a mapped record; an unmapped one is already reported.
        problems.extend(
            report
                .problems
                .iter()
                .filter(|p| !matches!(p.reason, LineError::Unmapped))
                .map(|p| (p.line, Report(&p.reason).to_string())),
        );
        problems.sort_by_key(|(number, _)| *number);
        problems.truncate(MAX_PROBLEMS);
        Ok(OldImport { outcome, problems })
    }
}

/// Records as Core takes them: one that could not be mapped is [`Mapped::Unmapped`], and why is
/// kept in `problems` (at most [`MAX_PROBLEMS`]).
struct Reported<'p, I> {
    records: I,
    problems: &'p mut Vec<(u64, String)>,
}

impl<I: Iterator<Item = (u64, Result<Mapped>)>> Iterator for Reported<'_, I> {
    type Item = (u64, Mapped);

    fn next(&mut self) -> Option<Self::Item> {
        let (number, record) = self.records.next()?;
        Some((
            number,
            record.unwrap_or_else(|e| {
                if self.problems.len() < MAX_PROBLEMS {
                    self.problems.push((number, format!("{e:#}")));
                }
                Mapped::Unmapped
            }),
        ))
    }
}

/// A 1.x ledger event as Core imports it: under its own identity (ST-15).
fn ledger_record(stored: &Stored, writer: &Writer) -> Result<Mapped> {
    let env = EventEnvelope::from_json(&stored.data).context("not a 1.x ledger event")?;
    if env.event.event_type() != stored.event_type {
        return Err(anyhow!(
            "the event is {} but stored as {}",
            env.event.event_type(),
            stored.event_type
        ));
    }
    let (_, env) = Canonical::of(env).context("encoding the event")?;
    let (event, _) = wire::to_new_event(env, writer)?;
    Ok(Mapped::Event {
        id: stored.id,
        event,
    })
}

/// A 1.x journal line as Core imports it: under the identity of the held event it equals, else
/// its content identity (M1-A). `None` for a blank line, and for a last line with no `\n` that
/// does not parse: a writer stopped mid-line (JR-03).
fn journal_record(
    text: &[u8],
    complete: bool,
    held: &Held,
    writer: &Writer,
) -> Option<Result<Mapped>> {
    if text.iter().all(u8::is_ascii_whitespace) {
        return None;
    }
    if text.len() > MAX_LINE_BYTES {
        return Some(Err(anyhow!(
            "the line is {} bytes, more than {MAX_LINE_BYTES}",
            text.len()
        )));
    }
    let env = match EventEnvelope::from_json(text) {
        Ok(env) => env,
        Err(_) if !complete => return None,
        Err(e) => return Some(Err(anyhow::Error::new(e).context("not a 1.x ledger event"))),
    };
    Some(journal_event(env, held, writer))
}

/// A parsed 1.x journal line under its identity (M1-A).
fn journal_event(env: EventEnvelope, held: &Held, writer: &Writer) -> Result<Mapped> {
    let (canonical, env) = Canonical::of(env).context("encoding the event")?;
    let id = match held.get(&canonical) {
        Some(id) => id,
        None => canonical.content_id()?,
    };
    let (event, _) = wire::to_new_event(env, writer)?;
    Ok(Mapped::Event { id, event })
}

#[cfg(test)]
mod tests;
