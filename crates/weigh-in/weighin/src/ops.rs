//! Ledger operations. Every command, every hook and the `batch` entry point goes through
//! [`Session`], which is the single write path: it reads the ledger, validates, appends under a
//! guard, and performs the side effects (atoms, source annotations) that follow a decision.
//!
//! This module holds the session itself; its operations live by family: what an operation is
//! and how a batch parses (`input`), raising (`raise`), triage, grounding and presenting
//! (`triage`), settling an item (`settle`: decide, defer, default, dismiss, reopen), relating it
//! (`relate`: supersede, link, ack), and the atoms and source annotations a decision writes back
//! (`records`), beside the paths scanned and annotated (`paths`) and the commit trailer
//! (`trailer`).

mod input;
mod paths;
mod raise;
mod records;
mod relate;
mod settle;
#[cfg(test)]
mod test_support;
mod trailer;
mod triage;

pub(crate) use input::{DecideOp, DeferOp, Op, RaiseInput, parse_ops};
pub(crate) use paths::{is_markdown, scannable};
pub(crate) use raise::RaiseOutcome;
pub(crate) use trailer::Trailer;

use anyhow::{Context as _, Result, anyhow, bail};
use std::fmt::Write as _;
use weighin_core::event::{EventEnvelope, LedgerEvent};
use weighin_core::id::ItemId;
use weighin_core::{Item, Ledger, Status};

use self::input::BadOp;
use self::relate::{ack_event, link_event, supersede_event};
use self::settle::{default_event, dismiss_event, reopen_event};
use self::triage::{ground_event, triage_event};
use crate::env::{self, Env};
use crate::git;
use crate::store::{Appended, Guard, Store};

/// The ledger as it stands, for commands that only read, or `None` when no store exists yet.
///
/// Never creates the store. A ledger is created by the first item raised, and a folder that
/// refuses deletes (B-01) could not be rid of one a read left behind.
///
/// # Errors
/// Fails when the store's path cannot be checked (so an unreadable ledger is never taken for a
/// missing one), or when an existing store cannot be opened or read.
pub(crate) fn stored_ledger(env: &Env) -> Result<Option<Ledger>> {
    existing_store(env)?.map(|s| s.ledger()).transpose()
}

/// Every ledger event in position order, or none when no store exists yet. Never creates the
/// store; see [`stored_ledger`].
///
/// # Errors
/// See [`stored_ledger`].
pub(crate) fn stored_events(env: &Env) -> Result<Vec<(u64, EventEnvelope)>> {
    Ok(existing_store(env)?
        .map(|s| s.read_all())
        .transpose()?
        .unwrap_or_default())
}

/// The store, opened only when it exists already. See [`stored_ledger`].
fn existing_store(env: &Env) -> Result<Option<Store>> {
    let path = env.store_path();
    if path
        .try_exists()
        .with_context(|| format!("checking {}", path.display()))?
    {
        Store::open(&path).map(Some)
    } else {
        Ok(None)
    }
}

/// [`stored_ledger`], with no store read as an empty ledger.
///
/// # Errors
/// See [`stored_ledger`].
pub(crate) fn read_ledger(env: &Env) -> Result<Ledger> {
    Ok(stored_ledger(env)?.unwrap_or_default())
}

/// The working tree's hash, read from git at most once per session.
#[derive(Debug)]
enum TreeCache {
    Unread,
    /// `None` when there is no repository or git could not hash the tree.
    Read(Option<String>),
}

/// The one event an operation appends to an item, and the receipt printed for it.
struct Change {
    event: LedgerEvent,
    receipt: String,
}

pub(crate) struct Session<'a> {
    pub(crate) env: &'a Env,
    /// The store, once one exists: it is created by the first append, so a command that fails
    /// before writing anything leaves no ledger behind (B-01).
    store: Option<Store>,
    pub(crate) ledger: Ledger,
    tree: TreeCache,
    actor: String,
    /// One slot per raise op earlier in this batch, so later ops can say `@1`, `@2`, ...: the
    /// raised id, or `None` when that raise failed (it keeps its place, so no ref shifts).
    raised: Vec<Option<ItemId>>,
    /// The date atoms and source annotations carry (`YYYY-MM-DD`), read once when the session
    /// opens so every write in a batch carries the same day.
    today: String,
}

impl<'a> Session<'a> {
    /// Open the session on the store, if there is one yet; it is created by the first append.
    ///
    /// # Errors
    /// Fails when the store's path cannot be checked, or an existing store cannot be opened or
    /// read.
    pub(crate) fn open(env: &'a Env, actor: &str) -> Result<Self> {
        let store = existing_store(env)?;
        let ledger = match &store {
            Some(s) => s.ledger()?,
            None => Ledger::default(),
        };
        Ok(Self {
            env,
            store,
            ledger,
            tree: TreeCache::Unread,
            actor: actor.to_owned(),
            raised: Vec::new(),
            today: env::today(),
        })
    }

    /// Fix the date this session writes, so a test does not depend on the wall clock.
    #[cfg(test)]
    fn with_today(mut self, today: &str) -> Self {
        today.clone_into(&mut self.today);
        self
    }

    fn tree_hash(&mut self) -> Option<String> {
        if let TreeCache::Read(t) = &self.tree {
            // The cache keeps its copy for the next event; the caller's event owns this one.
            return t.clone();
        }
        let t = self.env.repo_root.as_ref().and_then(|r| git::tree_hash(r));
        self.tree = TreeCache::Read(t.clone());
        t
    }

    /// The store, created on the first call when there was none.
    fn store(&mut self) -> Result<&Store> {
        let env = self.env;
        match &mut self.store {
            Some(store) => Ok(store),
            slot @ None => Ok(slot.insert(Store::open(&env.store_path())?)),
        }
    }

    /// Append events for `id` guarded on the item being unchanged since this session read it.
    fn write(&mut self, id: &ItemId, events: Vec<LedgerEvent>) -> Result<()> {
        let since = self.ledger.get(id).map_or(0, |i| i.last_position);
        let envs: Vec<_> = events
            .into_iter()
            .map(|e| self.env.envelope(e, &self.actor))
            .collect();
        match self
            .store()?
            .append(&envs, &Guard::ItemUnchangedSince(id.clone(), since))?
        {
            Appended::At(last) => {
                let n = u64::try_from(envs.len()).context("too many events in one append")?;
                let first = last + 1 - n;
                for (position, e) in (first..).zip(&envs) {
                    self.ledger.apply(position, e);
                }
                Ok(())
            }
            Appended::Conflict => {
                self.ledger = self.store()?.ledger()?;
                bail!(
                    "{id} changed while this was being written (another session acted on it); re-read with `weighin show {id}` and retry"
                )
            }
        }
    }

    fn item(&self, needle: &str) -> Result<Item> {
        if let Some(n) = needle.strip_prefix('@') {
            // The parse error adds nothing the message does not already say.
            let Ok(k) = n.parse::<usize>() else {
                bail!("`{needle}`: refs are @1, @2, ... (raises earlier in this batch)");
            };
            let slot = self.raised.get(k.wrapping_sub(1)).ok_or_else(|| {
                anyhow!(
                    "`{needle}`: only {} raise(s) so far in this batch",
                    self.raised.len()
                )
            })?;
            let Some(id) = slot else {
                bail!("`{needle}`: raise {k} in this batch failed, so it names no item");
            };
            return self
                .ledger
                .get(id)
                .cloned()
                .ok_or_else(|| anyhow!("{id} vanished"));
        }
        self.ledger
            .resolve(needle)
            .cloned()
            .ok_or_else(|| anyhow!("no single item matches `{needle}` (try `weighin open --all`)"))
    }

    /// Apply one operation and return a one-line receipt.
    ///
    /// # Errors
    /// Fails when the operation is invalid for the item or the store refuses it.
    pub(crate) fn apply(&mut self, op: &Op) -> Result<String> {
        match op {
            Op::Raise(r) => self.apply_raise(r),
            Op::Present { id, surface } => self.present(id, surface.as_deref()),
            Op::Decide(d) => self.decide(d),
            Op::Defer(d) => self.defer(d),
            Op::Triage(t) => self.change(&t.id, |item| triage_event(item, t)),
            Op::Ground { id, card, reason } => {
                self.change(id, |item| ground_event(item, card, reason.as_deref()))
            }
            Op::Default {
                id,
                option,
                cause,
                reason,
            } => self.change(id, |item| default_event(item, option, cause, reason)),
            Op::Dismiss {
                id,
                why,
                note,
                reason,
            } => self.change(id, |item| dismiss_event(item, why, note.as_deref(), reason)),
            Op::Reopen { id, note, reason } => {
                self.change(id, |item| reopen_event(item, note.as_deref(), reason))
            }
            Op::Supersede { id, by, reason } => {
                self.change(id, |item| Ok(supersede_event(item, by, reason)))
            }
            Op::Link {
                id,
                kind,
                target,
                reason,
            } => self.change(id, |item| {
                Ok(link_event(item, kind, target, reason.as_deref()))
            }),
            Op::Ack { id, reason } => self.ack(id, reason.as_deref()),
        }
    }

    /// Acknowledge a default, or review a standing decision; a review also moves the decision
    /// atom's `last_reviewed` to today (D-0022).
    fn ack(&mut self, id: &str, reason: Option<&str>) -> Result<String> {
        let item = self.item(id)?;
        let Change { event, mut receipt } = ack_event(&item, reason)?;
        self.write(&item.id, vec![event])?;
        if item.status == Status::Decided
            && let Some(path) = self.mark_decision_atom_reviewed(&item.id)?
        {
            write!(receipt, " · atom {path}")?;
        }
        Ok(receipt)
    }

    /// Apply one op of a batch as [`parse_ops`] returned it, and return its receipt. An op that
    /// did not parse fails with serde's error; a raise among them still takes its `@N` slot, so
    /// a ref to it is refused and later refs keep their place.
    ///
    /// # Errors
    /// Fails when the op did not parse, or as [`Session::apply`] does.
    pub(crate) fn apply_parsed(&mut self, op: Result<Op, BadOp>) -> Result<String> {
        match op {
            Ok(op) => self.apply(&op),
            Err(bad) => {
                if bad.raise {
                    self.raised.push(None);
                }
                Err(bad.error.into())
            }
        }
    }

    /// Resolve `id`, let `build` validate it and name its one event, append that, and return the
    /// receipt.
    fn change(&mut self, id: &str, build: impl FnOnce(&Item) -> Result<Change>) -> Result<String> {
        let item = self.item(id)?;
        let Change { event, receipt } = build(&item)?;
        self.write(&item.id, vec![event])?;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::path::Path;

    use super::Session;
    use super::test_support::{ONE_WAY, after, card, env_in, id_of, raised, run};

    /// A CRLF source file whose line asks [`ONE_WAY`], for the decision to annotate.
    fn source(dir: &Path) -> Result<()> {
        let text = format!("# Plan\r\n\r\n{ONE_WAY}\r\n- tail\r\n");
        std::fs::write(dir.join("plan.md"), text)?;
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_sitting_prints_each_receipt_unchanged() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        source(dir.path())?;
        let batch = serde_json::json!([
            {"op": "raise", "q": ONE_WAY, "door": "one-way", "authority": "authority", "source_path": "plan.md"},
            {"op": "triage", "id": "@1", "door": "one-way", "blast": "system", "route": "present", "reason": "r"},
            {"op": "ground", "id": "@1", "card": card()},
            {"op": "present", "id": "@1"},
            {"op": "decide", "id": "@1", "option": "B", "seconds": 5},
            {"op": "reopen", "id": "@1", "reason": "r"},
            {"op": "supersede", "id": "@1", "by": "kb-0001", "reason": "r"},
            {"op": "link", "id": "@1", "kind": "relates", "target": "wi-000000"}
        ]);

        let out = run(
            &mut Session::open(&env, "t")?.with_today("2026-01-02"),
            &batch,
        )?;

        let id = id_of(ONE_WAY);
        let atom = format!(
            ".kb/_intake/decisions/{id}-should-retention-attach-per-record-or-per-series.md"
        );
        assert_eq!(
            out,
            [
                format!("✓ raised {id} (raised)"),
                format!("✓ triaged {id} · one-way · system · present"),
                format!("✓ grounded {id} · 3 options · recommendation earned"),
                format!("✓ presented {id} (chat)"),
                format!(
                    "✓ decided {id} → B (override) · atom {atom} · annotated plan.md · trailer `Weigh-In: {id} decided B`"
                ),
                format!("✓ reopened {id}"),
                format!("✓ {id} superseded by kb-0001"),
                format!("✓ linked {id} → relates wi-000000"),
            ]
        );
        let annotated = std::fs::read_to_string(dir.path().join("plan.md"))?;
        assert_eq!(
            annotated,
            format!(
                "# Plan\r\n\r\n{ONE_WAY} → Weigh-In {id}: decided B (2026-01-02)\r\n- tail\r\n"
            )
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn refs_that_name_no_item_are_refused() -> Result<()> {
        let out = after(
            &raised(),
            &[
                serde_json::json!({"op": "present", "id": "@x"}),
                serde_json::json!({"op": "present", "id": "@3"}),
                serde_json::json!({"op": "present", "id": "wi-zzzzzz"}),
            ],
        )?;

        assert_eq!(
            out,
            [
                "✗ `@x`: refs are @1, @2, ... (raises earlier in this batch)",
                "✗ `@3`: only 1 raise(s) so far in this batch",
                "✗ no single item matches `wi-zzzzzz` (try `weighin open --all`)",
            ]
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_ref_to_a_failed_raise_is_refused_and_later_refs_keep_their_place() -> Result<()> {
        let out = after(
            &[],
            &[
                serde_json::json!({"op": "raise", "q": "   "}),
                serde_json::json!({"op": "raise", "q": ONE_WAY}),
                serde_json::json!({"op": "dismiss", "id": "@1", "why": "moot", "reason": "r"}),
                serde_json::json!({"op": "triage", "id": "@2", "door": "one-way", "blast": "file", "route": "present", "reason": "r"}),
            ],
        )?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out,
            [
                "✗ a raised item needs a question".to_owned(),
                format!("✓ raised {id} (raised)"),
                "✗ `@1`: raise 1 in this batch failed, so it names no item".to_owned(),
                format!("✓ triaged {id} · one-way · file · present"),
            ]
        );
        Ok(())
    }
}
