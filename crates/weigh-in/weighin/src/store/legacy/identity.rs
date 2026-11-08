//! The identity a weighin 1.x journal line takes when no event the store holds matches it
//! (M1-A, as amended by decisions H1 and H2, 2026-11-08), and the canonical form both the match
//! and the identity are taken from.
//!
//! The canonical form of a 1.x event is weighin-core's own encoding of it, the key 1.x deduped
//! journals by (`EventEnvelope::to_json`), with its 1.x `worktree` dropped: the stored fact keeps
//! no 1.x worktree (Core refuses a directory name as a `WorktreeId`), so a line can only be
//! compared with the store without it (H1). A line no held event matches is stored under
//! [`JOURNAL_STORE`] at [`position_of`] the first eight bytes of the canonical form's sha256.
//! Identities are permanent, so this module is pinned by golden tests.

use std::num::NonZeroU64;

use anyhow::{Context as _, Result};
use redkiln_core::EventId;
use redkiln_core::ids::{Digest, parse_event_ref};
use weighin_core::EventEnvelope;

/// The text the journal store id is the sha256 of (M1-A).
#[cfg(test)]
const JOURNAL_STORE_SEED: &[u8] = b"weighin-1x-journal";

/// The first 32 hex of sha256("weighin-1x-journal"): the store id of every 1.x journal line no
/// held event matches (M1-A). The tests recompute it from its seed.
pub(crate) const JOURNAL_STORE: &str = "8f161dc3d6d01eac2c7949e3b17ffa04";

/// 2^63 - 1: the largest position happenstance 0.4.5 stores, and the rule's modulus (H2).
const MODULUS: NonZeroU64 = match NonZeroU64::new(0x7fff_ffff_ffff_ffff) {
    Some(modulus) => modulus,
    None => NonZeroU64::MIN,
};

/// A 1.x event's canonical form (M1-A, H1), kept as its sha256.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Canonical {
    digest: Digest,
}

impl Canonical {
    /// `env` with its 1.x `worktree` dropped, handed back for the store, and the canonical form
    /// of what is left.
    ///
    /// # Errors
    /// weighin-core cannot encode the event.
    pub(crate) fn of(mut env: EventEnvelope) -> serde_json::Result<(Self, EventEnvelope)> {
        env.worktree = None;
        let digest = Digest::sha256(&env.to_json()?);
        Ok((Self { digest }, env))
    }

    /// The canonical form of `env`, which is left as it was: its `worktree` is set aside while
    /// it is encoded. For a reader that keeps the 1.x event whole (`stats`, decision H10).
    ///
    /// # Errors
    /// weighin-core cannot encode the event.
    pub(crate) fn of_kept(env: &mut EventEnvelope) -> serde_json::Result<Self> {
        let worktree = env.worktree.take();
        let encoded = env.to_json();
        env.worktree = worktree;
        Ok(Self {
            digest: Digest::sha256(&encoded?),
        })
    }

    /// The content identity (M1-A, as amended): [`JOURNAL_STORE`] at [`position_of`] the
    /// digest's first eight bytes, read big-endian.
    ///
    /// # Errors
    /// Never in practice: a sha256 digest has 64 hex digits and the position is in range.
    pub(crate) fn content_id(&self) -> Result<EventId> {
        let first8 = self
            .digest
            .hex()
            .get(..16)
            .context("a sha256 digest has 64 hex digits")?;
        let first8 = u64::from_str_radix(first8, 16).context("reading the digest's first bytes")?;
        let text = format!("{JOURNAL_STORE}:{}", position_of(first8));
        parse_event_ref(&text).with_context(|| format!("the content identity {text}"))
    }
}

/// `(first8 % (2^63 - 1)) + 1`: never 0, never past 2^63 - 1, which happenstance 0.4.5 refuses
/// as an origin position. The one place the M1-A position rule lives (H2).
pub(crate) fn position_of(first8: u64) -> NonZeroU64 {
    NonZeroU64::MIN.saturating_add(first8 % MODULUS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1.x journal line, as 1.x wrote it.
    const LINE: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"shown in chat","actor":"cli","session_id":"s1","worktree":"repo","ts":"2026-10-09T03:29:37Z"}"#;

    fn parsed(text: &str) -> EventEnvelope {
        EventEnvelope::from_json(text.as_bytes()).expect("a 1.x event")
    }

    #[test]
    fn the_journal_store_id_is_the_first_32_hex_of_sha256_of_its_seed() {
        let digest = Digest::sha256(JOURNAL_STORE_SEED);
        assert_eq!(digest.hex().get(..32), Some(JOURNAL_STORE));
    }

    #[test]
    fn the_position_is_never_zero_and_never_past_i64_max() {
        let max = u64::try_from(i64::MAX).expect("positive");
        for (first8, expected) in [
            (0, 1),
            (max - 1, max),
            (max, 1),
            (1 << 63, 2),
            (u64::MAX, 2),
        ] {
            assert_eq!(position_of(first8).get(), expected, "{first8}");
        }
    }

    #[test]
    fn the_canonical_form_drops_the_worktree_and_keeps_every_other_field() {
        let env = parsed(LINE);
        let (canonical, kept) = Canonical::of(env.clone()).expect("encoded");

        assert_eq!(kept.worktree, None);
        let mut expected = env;
        expected.worktree = None;
        assert_eq!(kept, expected, "every other field kept");
        let (without, _) = Canonical::of(expected).expect("encoded");
        assert_eq!(canonical, without);
    }

    #[test]
    fn two_spellings_of_one_event_have_one_canonical_form() {
        let respelled = r#"{ "ts":"2026-10-09T03:29:37Z", "actor":"cli","session_id":"s1",
            "type":"item_presented", "reason":"shown in chat","surface":"chat","item_id":"wi-b29e54" }"#;
        let crlf = respelled.replace('\n', "\r\n");

        let one = Canonical::of(parsed(LINE)).expect("encoded").0;
        for other in [respelled, crlf.as_str()] {
            assert_eq!(
                Canonical::of(parsed(other)).expect("encoded").0,
                one,
                "{other}"
            );
        }
    }

    #[test]
    fn a_fixed_line_has_a_fixed_identity() {
        // The canonical form: weighin-core's encoding with `worktree` null.
        const CANONICAL: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","tree_hash":null,"reason":"shown in chat","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-09T03:29:37Z"}"#;
        // Computed outside Rust:
        //   printf '%s' "$CANONICAL" | sha256sum
        //     ffd6e1e944f259a2ae51756f00dab0dd203408ffffbc1fa474e2ef35a601e33f
        //   python3 -c 'h="ffd6e1e944f259a2"; print(int(h[:16],16) % (2**63-1) + 1)'
        //     9211798479833291172
        // The first eight bytes are past 2^63, so the modulus is exercised.
        const IDENTITY: &str = "8f161dc3d6d01eac2c7949e3b17ffa04:9211798479833291172";

        let env = parsed(LINE);
        let mut dropped = env.clone();
        dropped.worktree = None;
        assert_eq!(
            dropped.to_json().expect("encoded"),
            CANONICAL.as_bytes(),
            "the canonical bytes"
        );
        let (canonical, _) = Canonical::of(env).expect("encoded");
        assert_eq!(
            canonical.content_id().expect("an identity").to_string(),
            IDENTITY
        );
    }
}
