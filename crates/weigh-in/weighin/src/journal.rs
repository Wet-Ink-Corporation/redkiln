//! Committed journals: a ledger's events as JSON lines under `.kb/_ledger/journal/`.
//!
//! A cloud session's ledger goes with its container (D-0014), and with it every presentation,
//! dismissal, deferral and default the pilot is judged on (#23); only decisions survive, as
//! atoms. So every session keeps its own events in `<DIR>/<session>.jsonl`: the binary appends
//! them after each capture hook and each command that changes the ledger ([`sync`], B-28), the
//! file is committed with the session's work, and `stats` folds every journal with the ledger.
//! An event held twice, in the ledger and a journal or in two journals, counts once.
//! `weighin journal` prints the whole ledger in the same line format.

mod append;

pub(crate) use append::{Skip, Synced, WAIT, kept, sync, sync_events};

use anyhow::{Context as _, Result};
use std::collections::HashSet;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use weighin_core::{EventEnvelope, Ledger};

/// Where journals live, relative to the worktree root.
pub(crate) const DIR: &str = ".kb/_ledger/journal";

/// One line of a journal file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Line<'a> {
    /// 1-based line number.
    pub(crate) number: u64,
    /// The line's bytes without its `\n` and one trailing `\r`.
    pub(crate) text: &'a [u8],
    /// Whether a `\n` ended the line; only the last line of a file can lack one.
    pub(crate) complete: bool,
}

impl Line<'_> {
    /// Whether the line is UTF-8 whitespace only.
    pub(crate) fn is_blank(&self) -> bool {
        std::str::from_utf8(self.text).is_ok_and(|l| l.trim().is_empty())
    }
}

/// The lines of a journal file's `bytes`, in order.
pub(crate) fn lines(bytes: &[u8]) -> impl Iterator<Item = Line<'_>> {
    (1_u64..)
        .zip(bytes.split_inclusive(|&b| b == b'\n'))
        .map(|(number, piece)| match piece.strip_suffix(b"\n") {
            Some(text) => Line {
                number,
                text: text.strip_suffix(b"\r").unwrap_or(text),
                complete: true,
            },
            None => Line {
                number,
                text: piece,
                complete: false,
            },
        })
}

/// The journal line for `env`: its JSON and a newline. `write` and `sync` both use it, so a
/// session's own journal holds the bytes `weighin journal` prints.
///
/// # Errors
/// Fails when the event cannot be encoded.
pub(crate) fn line(env: &EventEnvelope) -> serde_json::Result<Vec<u8>> {
    let mut bytes = env.to_json()?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write `events` as one JSON envelope per line, in the order given.
///
/// # Errors
/// Fails when an event cannot be encoded or `out` cannot be written.
pub(crate) fn write(events: &[(u64, EventEnvelope)], out: &mut impl Write) -> Result<()> {
    for (_, env) in events {
        out.write_all(&line(env)?)?;
    }
    Ok(())
}

/// A journal file as read from the worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JournalFile {
    /// The path relative to the worktree root, with `/` separators.
    pub(crate) path: String,
    /// The file's contents.
    pub(crate) bytes: Vec<u8>,
}

/// Every `*.jsonl` file directly under `<root>/DIR`, in name order. No journal directory reads
/// as no files.
///
/// # Errors
/// Fails when the directory or a journal cannot be read.
pub(crate) fn files(root: &Path) -> Result<Vec<JournalFile>> {
    let dir = root.join(DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    let mut paths = entries
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<PathBuf>>>()
        .with_context(|| format!("reading {}", dir.display()))?;
    paths.retain(|p| p.extension().is_some_and(|x| x == "jsonl"));
    paths.sort();
    paths
        .iter()
        .map(|p| {
            // Bytes, not a string: a line cut short can end inside a multi-byte character.
            let bytes = std::fs::read(p).with_context(|| format!("reading {}", p.display()))?;
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default();
            Ok(JournalFile {
                path: format!("{DIR}/{name}"),
                bytes,
            })
        })
        .collect()
}

/// Every event in the journals under `root`: files in name order, lines in file order, blank
/// lines passed over. No journal directory reads as no events. A file's last line that has no
/// trailing newline and does not decode is passed over too: a run killed mid-append cut it short.
/// The next write to that journal drops such a line and appends its event again while the
/// session's ledger is there, but a journal committed before that, or after the container went,
/// still holds it.
///
/// # Errors
/// Fails when the directory or a journal cannot be read, or any other line is not an event; the
/// error names the file and line.
pub(crate) fn read(root: &Path) -> Result<Vec<EventEnvelope>> {
    let mut events = Vec::new();
    for file in files(root)? {
        parse(&file.path, &file.bytes, &mut events)?;
    }
    Ok(events)
}

/// Append the events in `bytes`, the contents of the journal `file`, to `events`, as [`read`]
/// describes. A line that is not UTF-8 does not decode.
///
/// # Errors
/// Fails when a line other than a cut-short last line is not an event.
fn parse(file: &str, bytes: &[u8], events: &mut Vec<EventEnvelope>) -> Result<()> {
    for line in lines(bytes).filter(|l| !l.is_blank()) {
        match EventEnvelope::from_json(line.text) {
            Ok(env) => events.push(env),
            // A cut-short last line is a write the next run repeats, not a failure to report.
            Err(_) if !line.complete => {}
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("{file} line {}: not a ledger event", line.number));
            }
        }
    }
    Ok(())
}

/// The ledger folded from the journals' events followed by its own, each event once (the first
/// copy kept). Order is the sources' own, never timestamps: each journal in its file order, then
/// the ledger in position order. Journals come first because they can hold history the ledger
/// lost: a session resumed in a new container keeps its id but starts an empty ledger, its hook
/// appends the new ledger's events after the old ones, and a question raised again mints the
/// same item id. Folding that ledger first would replay the older events after the newer. A
/// journal of the ledger's own store holds a prefix of it, which this order folds unchanged.
///
/// # Errors
/// Fails when an event cannot be encoded for comparison.
pub(crate) fn pool(
    ledger: Vec<(u64, EventEnvelope)>,
    journal: Vec<EventEnvelope>,
) -> Result<Ledger> {
    if journal.is_empty() {
        return Ok(Ledger::fold(ledger.iter().map(|(p, e)| (*p, e))));
    }
    let mut seen = HashSet::new();
    let mut events = Vec::with_capacity(ledger.len().saturating_add(journal.len()));
    for env in journal
        .into_iter()
        .chain(ledger.into_iter().map(|(_, e)| e))
    {
        if seen.insert(env.to_json()?) {
            events.push(env);
        }
    }
    Ok(Ledger::fold((1_u64..).zip(events.iter())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAISED: &str = r#"{"type":"item_raised","item_id":"wi-b29e54","kind":"question","question":"Keep the cache warm or cold?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"Keep the cache warm or cold?","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:36Z"}"#;
    const PRESENTED: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"","actor":"cli","session_id":"s","ts":"2026-10-09T03:29:37Z"}"#;

    fn events() -> Result<Vec<EventEnvelope>> {
        Ok(vec![
            EventEnvelope::from_json(RAISED.as_bytes())?,
            EventEnvelope::from_json(PRESENTED.as_bytes())?,
        ])
    }

    #[test]
    fn an_event_in_the_ledger_and_a_journal_folds_once() -> Result<()> {
        let ledger = (1_u64..).zip(events()?).collect();

        let pooled = pool(ledger, events()?)?;

        let item = pooled
            .resolve("wi-b29e54")
            .ok_or_else(|| anyhow::anyhow!("item missing"))?;
        assert_eq!((pooled.head, item.times_presented), (2, 1));
        Ok(())
    }

    /// A folder whose one journal, `s.jsonl`, holds `text`.
    fn folder_with_journal(text: impl AsRef<[u8]>) -> Result<tempfile::TempDir> {
        let tmp = tempfile::tempdir()?;
        let dir = tmp.path().join(DIR);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("s.jsonl"), text)?;
        Ok(tmp)
    }

    #[test]
    fn a_last_line_cut_short_is_skipped() -> Result<()> {
        let cut = PRESENTED
            .get(..40)
            .ok_or_else(|| anyhow::anyhow!("short"))?;
        let tmp = folder_with_journal(format!("{RAISED}\n{cut}"))?;

        let read = read(tmp.path())?;

        assert_eq!(read, vec![EventEnvelope::from_json(RAISED.as_bytes())?]);
        Ok(())
    }

    #[test]
    fn a_last_line_cut_inside_a_character_is_skipped() -> Result<()> {
        let presented = PRESENTED.replace(r#""reason":"""#, r#""reason":"warm — cold""#);
        let dash = presented
            .find('—')
            .ok_or_else(|| anyhow::anyhow!("no dash"))?;
        let mut text = format!("{RAISED}\n").into_bytes();
        text.extend(
            presented
                .as_bytes()
                .get(..=dash)
                .ok_or_else(|| anyhow::anyhow!("short"))?,
        );
        let tmp = folder_with_journal(text)?;

        let read = read(tmp.path())?;

        assert_eq!(read, vec![EventEnvelope::from_json(RAISED.as_bytes())?]);
        Ok(())
    }

    #[test]
    fn a_journal_with_crlf_line_endings_reads_every_event() -> Result<()> {
        let tmp = folder_with_journal(format!("{RAISED}\r\n{PRESENTED}\r\n"))?;

        let read = read(tmp.path())?;

        assert_eq!(read, events()?);
        Ok(())
    }

    #[test]
    fn a_bad_line_in_the_middle_fails_naming_file_and_line() -> Result<()> {
        let tmp = folder_with_journal(format!("{RAISED}\nnot json\n{PRESENTED}"))?;

        let e = read(tmp.path()).expect_err("the middle line is not an event");

        let msg = format!("{e:#}");
        assert!(msg.contains("s.jsonl line 2: not a ledger event"), "{msg}");
        Ok(())
    }

    #[test]
    fn lines_strips_newlines_and_one_cr_and_marks_an_unterminated_last_line() {
        let text = b"a\r\nb\n\nc\r\r\nd";

        let got: Vec<Line<'_>> = lines(text).collect();

        let line = |number, text: &'static [u8], complete| Line {
            number,
            text,
            complete,
        };
        assert_eq!(
            got,
            vec![
                line(1, b"a", true),
                line(2, b"b", true),
                line(3, b"", true),
                line(4, b"c\r", true),
                line(5, b"d", false),
            ]
        );
    }

    #[test]
    fn a_journal_line_is_the_envelope_json_and_a_newline() -> Result<()> {
        let env = EventEnvelope::from_json(RAISED.as_bytes())?;
        let mut expected = env.to_json()?;
        expected.push(b'\n');

        assert_eq!(line(&env)?, expected);
        Ok(())
    }

    #[test]
    fn a_bad_last_line_that_ends_in_a_newline_fails() -> Result<()> {
        let tmp = folder_with_journal(format!("{RAISED}\nnot json\n"))?;

        let e = read(tmp.path()).expect_err("a whole line was written, and it is not an event");

        let msg = format!("{e:#}");
        assert!(msg.contains("s.jsonl line 2: not a ledger event"), "{msg}");
        Ok(())
    }
}
