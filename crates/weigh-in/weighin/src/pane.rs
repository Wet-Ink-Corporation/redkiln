//! What the local board pane of a Claude Code mod needs from the CLI (#115, D-0017): the board
//! state on stdout, never a file and never published, and one answer at a time from stdin. The
//! pane works under a `local` or an `artifact` board, shows nothing under `off`, and leaves items
//! no sitting has grounded read-only.

use anyhow::{Context as _, Result, bail};
use std::io::{Read, Write};
use weighin_core::Ledger;
use weighin_core::board::{self, Answer, Scope};

use crate::env::{BoardMode, BoardUse, Env};
use crate::ops::read_ledger;

/// The most bytes of answers read from stdin: the Artifact `db` store's document cap, which no
/// one answer comes near (H-13).
pub(crate) const STDIN_CAP: u64 = 256 * 1024;

/// Whether the board may be shown in this folder: only when its config chose a `local` or an
/// `artifact` board. A folder never initialised has chosen neither.
///
/// # Errors
/// Fails when the folder has no config, a broken one, or a board mode of `off`.
pub(crate) fn allowed(env: &Env) -> Result<()> {
    env.board_allows(&BoardUse {
        modes: &[BoardMode::Local, BoardMode::Artifact],
        before: ", before using the board pane",
        refusing: "not using the board pane",
        other_mode: "the board shows nothing here (`weighin init --board local` changes the mode)",
    })
}

/// `board export --stdout`: the board state document as one JSON line on `out`. Reads the ledger
/// and never creates it; writes no file.
///
/// # Errors
/// Fails as [`allowed`] does, or when the ledger cannot be read or the output written.
pub(crate) fn export(env: &Env, scope: Scope, now: &str, out: &mut impl Write) -> Result<()> {
    allowed(env)?;
    let ledger = read_ledger(env)?;
    let state = board::export(&ledger, scope, env.config.caps.board_focus, now);
    writeln!(out, "{}", serde_json::to_string(&state)?)?;
    Ok(())
}

/// Refuse an answer for an item no sitting has grounded: the pane shows it read-only, since the
/// five filters have not run on it (D-0017). An unknown item is left to the answer's validation.
///
/// # Errors
/// Fails when the answer names an item with no card.
pub(crate) fn grounded(ledger: &Ledger, answer: &Answer) -> Result<()> {
    match ledger.resolve(&answer.item_id) {
        Some(item) if item.card.is_none() => bail!(
            "{} was not grounded by a sitting, so it is read-only in the pane",
            item.id
        ),
        Some(_) | None => Ok(()),
    }
}

/// At most [`STDIN_CAP`] bytes of `input`, as text.
///
/// # Errors
/// Fails when `input` cannot be read, is not UTF-8, or holds more than the cap.
pub(crate) fn read_capped(input: impl Read) -> Result<String> {
    let mut bytes = Vec::new();
    let read = input
        .take(STDIN_CAP.saturating_add(1))
        .read_to_end(&mut bytes)?;
    // The size is checked before the encoding: too much input is refused as such.
    if u64::try_from(read).map_or(true, |n| n > STDIN_CAP) {
        bail!("more than {STDIN_CAP} bytes of answers on stdin");
    }
    String::from_utf8(bytes).context("the answers on stdin are not UTF-8")
}

#[cfg(test)]
mod tests {
    use super::{STDIN_CAP, read_capped};

    #[test]
    fn answers_past_the_cap_are_refused_unread() {
        let big = vec![b' '; usize::try_from(STDIN_CAP).expect("the cap fits") + 1];

        let read = read_capped(big.as_slice());

        assert_eq!(
            read.map_err(|e| e.to_string()),
            Err(format!("more than {STDIN_CAP} bytes of answers on stdin"))
        );
    }

    #[test]
    fn bytes_past_the_cap_are_refused_for_their_size_before_their_encoding() {
        let big = vec![0xff_u8; usize::try_from(STDIN_CAP).expect("the cap fits") + 1];

        let read = read_capped(big.as_slice());

        assert_eq!(
            read.map_err(|e| e.to_string()),
            Err(format!("more than {STDIN_CAP} bytes of answers on stdin"))
        );
    }

    #[test]
    fn answers_that_are_not_utf8_are_refused() {
        let read = read_capped(&[b'{', 0xff, b'}'][..]);

        assert_eq!(
            read.map_err(|e| e.to_string()),
            Err("the answers on stdin are not UTF-8".to_owned())
        );
    }

    #[test]
    fn answers_within_the_cap_are_read_whole() {
        let read = read_capped(&b"{\"item_id\": \"wi-1\"}"[..]);

        assert_eq!(read.ok().as_deref(), Some("{\"item_id\": \"wi-1\"}"));
    }
}
