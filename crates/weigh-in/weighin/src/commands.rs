//! What each command does. `main.rs` parses the command line and dispatches here; the modules
//! group the commands by what they touch: the config (`init`), the items as they stand (`view`),
//! the write path (`write`), the defaults and the untriaged (`digest`), the health numbers
//! (`stats`) and the board (`board`).

mod board;
mod digest;
mod init;
mod stats;
mod view;
mod write;

pub(crate) use board::{board_apply, board_export};
pub(crate) use digest::digest;
pub(crate) use init::init;
pub(crate) use stats::stats;
pub(crate) use view::{context_pack, open, show, trailer};
pub(crate) use write::{apply_one, batch, raise, scan};

use anyhow::{Result, anyhow};
use std::io::{self, Write};
use weighin_core::Item;

use crate::{env, text};

/// The receipts of a command that changes the ledger, one line per change. A failed write stops
/// the receipts but not the changes: every operation is still applied, and [`Receipts::finish`]
/// returns the failure at the end, so a reader that stops early (`weighin batch | head -1`)
/// never leaves a batch half applied.
struct Receipts<W> {
    out: W,
    /// The first write that failed; nothing more is written after it.
    failed: Option<io::Error>,
}

impl<W: Write> Receipts<W> {
    fn new(out: W) -> Self {
        Self { out, failed: None }
    }

    /// Write one receipt line, unless an earlier one failed.
    fn line(&mut self, line: std::fmt::Arguments<'_>) {
        if self.failed.is_none()
            && let Err(e) = writeln!(self.out, "{line}")
        {
            self.failed = Some(e);
        }
    }

    /// The command's result: its own failure first, since the changes matter more than their
    /// receipts, then the write that failed.
    ///
    /// # Errors
    /// Fails when `outcome` is a failure or a receipt could not be written.
    fn finish(self, outcome: Result<()>) -> Result<()> {
        match (outcome, self.failed) {
            (Ok(()), None) => Ok(()),
            (Ok(()), Some(written)) => Err(written.into()),
            (Err(e), None) => Err(e),
            // Not chained: a closed stdout in the chain would make `report` exit 0 in silence.
            (Err(e), Some(written)) => Err(anyhow!("{e:#}; the receipts stopped at {written}")),
        }
    }
}

/// `s` on one line, cut to at most `n` characters.
fn clip(s: &str, n: usize) -> String {
    text::clip(&s.replace('\n', " "), n)
}

/// Two lines per item, about 60 tokens.
fn write_line(out: &mut impl Write, i: &Item) -> io::Result<()> {
    let mut meta = vec![
        i.door().as_str().to_owned(),
        i.authority().as_str().to_owned(),
        i.status.as_str().to_owned(),
        format!("{} ago", env::age(&i.raised_ts)),
    ];
    if i.channel != weighin_core::event::Channel::Marker {
        meta.push(i.channel.as_str().to_owned());
    }
    if !i.also_raised_in.is_empty() {
        meta.push(format!("raised in {} sessions", i.also_raised_in.len() + 1));
    }
    if i.card.is_some() {
        meta.push("card".into());
    }
    writeln!(
        out,
        "{} [{}] {}",
        i.id,
        meta.join(" · "),
        clip(&i.question, 140)
    )?;
    let mut detail = Vec::new();
    if let Some(d) = &i.did {
        detail.push(format!("did: {}", clip(d, 70)));
    }
    if let Some(f) = &i.flip_hint {
        detail.push(format!("flip: {}", clip(f, 60)));
    }
    if let Some(a) = &i.at {
        detail.push(format!("at: {a}"));
    }
    if let Some(d) = &i.deferral {
        detail.push(format!(
            "waiting for: {} (default {} by {})",
            clip(&d.trigger, 50),
            d.default_option,
            d.resolve_by
        ));
    }
    if let Some(d) = &i.default {
        detail.push(format!("took: {}", clip(&d.option, 60)));
    }
    if !detail.is_empty() {
        writeln!(out, "  {}", detail.join(" · "))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::clip;

    #[test]
    fn clip_keeps_short_text_and_ellipsizes_long_text() {
        assert_eq!(clip("a\nb", 3), "a b");
        assert_eq!(clip("abcdef", 4), "abc…");
        assert_eq!(clip("abc", 0), "");
    }
}
