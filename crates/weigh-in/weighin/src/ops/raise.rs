//! Raising an item: idempotent per session, linked to an open duplicate from another session,
//! and logged as a default when it is a two-way judgment call the agent already acted on.

use anyhow::{Result, bail};
use weighin_core::event::{Channel, DefaultCause, Defaulted, LedgerEvent, Linked, Raised};
use weighin_core::id::{EXTENDED_HEX_LEN, ItemId};
use weighin_core::marker::{Authority, Door, Kind, authority_tripwire, did_is_blocked};

use super::{RaiseInput, Session};
use crate::store::{Appended, Guard};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RaiseOutcome {
    New(ItemId),
    /// The same session raised it before (a hook firing twice).
    Existing(ItemId),
    /// Another open item already asks this; linked to it instead.
    Duplicate(ItemId),
}

impl RaiseOutcome {
    #[must_use]
    pub(crate) fn id(&self) -> &ItemId {
        match self {
            Self::New(i) | Self::Existing(i) | Self::Duplicate(i) => i,
        }
    }
}

/// How a raise was routed, read from its hints (unknown or missing values take the defaults).
#[derive(Clone, Copy)]
struct Hints {
    channel: Channel,
    door: Door,
    authority: Authority,
}

impl Hints {
    fn of(input: &RaiseInput) -> Self {
        let channel = match input.channel.as_deref() {
            Some("scan") => Channel::Scan,
            Some("sweep") => Channel::Sweep,
            Some("manual") => Channel::Manual,
            _ => Channel::Marker,
        };
        Self {
            channel,
            door: input.door.as_deref().map(Door::parse).unwrap_or_default(),
            authority: input
                .authority
                .as_deref()
                .map(Authority::parse)
                .unwrap_or_default(),
        }
    }
}

impl Session<'_> {
    /// Raise a question. Idempotent per session; an open duplicate from another session is linked.
    ///
    /// # Errors
    /// Fails on an empty question or a store error.
    pub(crate) fn raise(&mut self, input: &RaiseInput) -> Result<RaiseOutcome> {
        let outcome = self.raise_item(input)?;
        if let RaiseOutcome::New(id) = &outcome {
            self.default_acted_call(input, id)?;
        }
        Ok(outcome)
    }

    /// Raise `input` without the acted-call default [`Session::raise`] then writes, so a caller
    /// can note the new id before that second write.
    fn raise_item(&mut self, input: &RaiseInput) -> Result<RaiseOutcome> {
        let q = input.q.trim();
        if q.is_empty() {
            bail!("a raised item needs a question");
        }
        let session = self
            .env
            .session
            .clone()
            .unwrap_or_else(|| "no-session".into());
        let minted = ItemId::mint(&session, q);
        if let Some(known) = self.already_raised(q, &session, &minted)? {
            return Ok(known);
        }
        self.append_raised(input, minted, &session)
    }

    /// The item already asking `q`, if any, noting that `session` raised it again.
    fn already_raised(
        &mut self,
        q: &str,
        session: &str,
        minted: &ItemId,
    ) -> Result<Option<RaiseOutcome>> {
        if let Some(closed) = self
            .ledger
            .any_with_question(q)
            .filter(|i| !i.status.is_open())
        {
            // Settled (decided, defaulted, dismissed): never re-asked. Record the re-raise only.
            let cid = closed.id.clone();
            if closed.session_id.as_deref() != Some(session)
                && !closed.also_raised_in.iter().any(|s| s.as_str() == session)
            {
                self.write(
                    &cid,
                    vec![LedgerEvent::ItemLinked(Linked {
                        item_id: cid.clone(),
                        kind: "raised-again".into(),
                        target: session.to_owned(),
                        reason: "a settled question was raised again; not re-asked".into(),
                    })],
                )?;
            }
            return Ok(Some(RaiseOutcome::Existing(cid)));
        }
        let Some(existing) = self.ledger.open_with_question(q) else {
            return Ok(None);
        };
        let eid = existing.id.clone();
        if &eid == minted || existing.session_id.as_deref() == Some(session) {
            return Ok(Some(RaiseOutcome::Existing(eid)));
        }
        if !existing
            .also_raised_in
            .iter()
            .any(|s| s.as_str() == session)
        {
            self.write(
                &eid,
                vec![LedgerEvent::ItemLinked(Linked {
                    item_id: eid.clone(),
                    kind: "raised-again".into(),
                    target: session.to_owned(),
                    reason: "the same question was raised in another session".into(),
                })],
            )?;
        }
        Ok(Some(RaiseOutcome::Duplicate(eid)))
    }

    /// Append the raise under `minted`, or under its extended form when another question holds
    /// the short id. `Existing` when a racing writer raised the same question first.
    fn append_raised(
        &mut self,
        input: &RaiseInput,
        minted: ItemId,
        session: &str,
    ) -> Result<RaiseOutcome> {
        let q = input.q.trim();
        let mut id = minted;
        for _ in 0..2 {
            let env = self
                .env
                .envelope(self.raised_event(input, id.clone()), &self.actor);
            match self
                .store()?
                .append(std::slice::from_ref(&env), &Guard::NotYetRaised(id.clone()))?
            {
                Appended::At(p) => {
                    self.ledger.apply(p, &env);
                    return Ok(RaiseOutcome::New(id));
                }
                Appended::Conflict => {
                    self.ledger = self.store()?.ledger()?;
                    let same = self.ledger.get(&id).is_some_and(|i| {
                        weighin_core::id::question_key(&i.question)
                            == weighin_core::id::question_key(q)
                    });
                    if same {
                        return Ok(RaiseOutcome::Existing(id));
                    }
                    id = ItemId::mint_with_len(session, q, EXTENDED_HEX_LEN);
                }
            }
        }
        bail!("could not mint a free id for this question")
    }

    fn raised_event(&self, input: &RaiseInput, id: ItemId) -> LedgerEvent {
        let q = input.q.trim();
        let hints = Hints::of(input);
        LedgerEvent::ItemRaised(Raised {
            item_id: id,
            kind: input.kind.as_deref().map(Kind::parse).unwrap_or_default(),
            question: q.to_owned(),
            did: input.did.clone(),
            flip: input.flip.clone(),
            at: input.at.as_deref().map(|a| self.env.relative_anchor(a)),
            sym: input.sym.clone(),
            blocks: input.blocks.clone(),
            door_hint: hints.door,
            authority_hint: hints.authority,
            channel: hints.channel,
            quote: input.quote.clone().unwrap_or_else(|| q.to_owned()),
            source_path: input.source_path.clone(),
            source_line: input.source_line,
        })
    }

    /// Decision wi-4d8b6a: a judgment-bound two-way door the agent already acted on is its own
    /// call, logged as a default and listed in the digest; it never becomes a question.
    fn default_acted_call(&mut self, input: &RaiseInput, id: &ItemId) -> Result<()> {
        let hints = Hints::of(input);
        let did = input
            .did
            .as_deref()
            .map(str::trim)
            .filter(|d| !d.is_empty() && !did_is_blocked(d));
        // The tripwire keeps authority-sounding questions (compliance, isolation, retention, ...)
        // open for the sitting even when the agent called them judgment calls.
        let tripwire = authority_tripwire(&format!(
            "{} {} {}",
            input.q.trim(),
            did.unwrap_or(""),
            input.flip.as_deref().unwrap_or("")
        ));
        if hints.channel == Channel::Marker
            && hints.door == Door::TwoWay
            && hints.authority == Authority::Judgment
            && tripwire.is_none()
            && let Some(did) = did
        {
            self.write(id, vec![LedgerEvent::ItemDefaulted(Defaulted { item_id: id.clone(), option: did.to_owned(), cause: DefaultCause::TwoWay, reason: "judgment-bound two-way door the agent acted on; logged, reversible from the digest".into() })])?;
        }
        Ok(())
    }

    /// Raise `r` into the next `@N` slot. The slot is taken even when the raise fails, and is
    /// filled as soon as the item exists, before its acted-call default is written.
    pub(super) fn apply_raise(&mut self, r: &RaiseInput) -> Result<String> {
        let raised = self.raise_item(r);
        // The slot keeps its own copy of the id; the outcome goes on to the receipt.
        self.raised
            .push(raised.as_ref().ok().map(|o| o.id().clone()));
        let out = raised?;
        if let RaiseOutcome::New(id) = &out {
            self.default_acted_call(r, id)?;
        }
        let status = self.ledger.get(out.id()).map_or("?", |i| i.status.as_str());
        Ok(match out {
            RaiseOutcome::New(id) => format!("raised {id} ({status})"),
            RaiseOutcome::Existing(id) => format!("{id} already raised"),
            RaiseOutcome::Duplicate(id) => format!("linked to open {id}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::ops::Session;
    use crate::ops::test_support::{ONE_WAY, TWO_WAY, after, env_in, id_of, run};

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn raise_receipts_name_new_existing_and_linked_items() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let first = env_in(dir.path(), "s1");
        let second = env_in(dir.path(), "s2");
        let raise = serde_json::json!([{"op": "raise", "q": ONE_WAY, "door": "one-way"}]);
        let id = id_of(ONE_WAY);

        let new = run(&mut Session::open(&first, "t")?, &raise)?;
        let again = run(&mut Session::open(&first, "t")?, &raise)?;
        let elsewhere = run(&mut Session::open(&second, "t")?, &raise)?;

        assert_eq!(new, [format!("✓ raised {id} (raised)")]);
        assert_eq!(again, [format!("✓ {id} already raised")]);
        assert_eq!(elsewhere, [format!("✓ linked to open {id}")]);
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn an_acted_two_way_judgment_call_is_raised_as_a_default() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let batch = serde_json::json!([
            {"op": "raise", "q": TWO_WAY, "door": "two-way", "authority": "judgment", "did": "named it apply"},
            {"op": "ack", "id": "@1"}
        ]);

        let out = run(&mut Session::open(&env, "t")?, &batch)?;

        let id = id_of(TWO_WAY);
        assert_eq!(
            out,
            [
                format!("✓ raised {id} (defaulted)"),
                format!("✓ acknowledged default on {id}"),
            ]
        );
        Ok(())
    }

    /// wi-64c8ea: `blocked` followed by what the agent did instead is still blocked, so the item
    /// stays a question rather than a logged call.
    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_blocked_call_with_an_explanation_stays_raised() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let batch = serde_json::json!([
            {"op": "raise", "q": TWO_WAY, "door": "two-way", "authority": "judgment", "did": "blocked; left the guard as it is"}
        ]);

        let out = run(&mut Session::open(&env, "t")?, &batch)?;

        assert_eq!(out, [format!("✓ raised {} (raised)", id_of(TWO_WAY))]);
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_raise_without_a_question_is_refused() -> Result<()> {
        let out = after(&[], &[serde_json::json!({"op": "raise", "q": "  "})])?;

        assert_eq!(out, ["✗ a raised item needs a question"]);
        Ok(())
    }
}
