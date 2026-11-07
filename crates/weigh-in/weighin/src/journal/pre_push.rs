//! git's `pre-push` hook input (git hook protocol, kept at the edge like hook payloads): which
//! refs a push sends, so a push of only journal refs and inbox branches exports nothing (JR-08).

use std::io::{self, Read};

use redkiln_core::journal::refs::INBOX_BRANCHES;

/// The most of git's pre-push input read (H-13): about 10,000 refs.
pub(crate) const MAX_INPUT_BYTES: u64 = 1024 * 1024;

/// Where every journal ref, broken ref and clone copy lives on a remote.
const REDKILN_REFS: &str = "refs/redkiln/";

/// What a push sends, as far as the checkpoint cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pushing {
    /// Only journal refs and inbox branches, or nothing: the push is a checkpoint's own.
    OnlyJournals,
    /// Anything else, or input this reader cannot judge.
    Other,
}

/// What git's pre-push lines on `input` push: `<local ref> SP <local oid> SP <remote ref> SP
/// <remote oid> LF`, judged by the remote ref. [`Pushing::OnlyJournals`] when every remote ref is
/// under `refs/redkiln/` or `refs/heads/redkiln/inbox/`, and when there are no lines. A
/// malformed line, or input past [`MAX_INPUT_BYTES`], is [`Pushing::Other`], so the push exports
/// rather than skips.
///
/// # Errors
/// Reading `input` failed.
pub(crate) fn pushing(input: impl Read) -> io::Result<Pushing> {
    let mut bytes = Vec::new();
    input
        .take(MAX_INPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_INPUT_BYTES) {
        return Ok(Pushing::Other);
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(Pushing::Other);
    };
    let only_journals = text.split_terminator('\n').all(journal_only_line);
    Ok(if only_journals {
        Pushing::OnlyJournals
    } else {
        Pushing::Other
    })
}

/// Whether `line` has git's four fields and pushes to a journal ref or an inbox branch.
fn journal_only_line(line: &str) -> bool {
    let mut fields = line.split(' ');
    match (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) {
        (Some(_), Some(_), Some(remote), Some(_), None) => {
            remote.starts_with(REDKILN_REFS) || remote.starts_with(INBOX_BRANCHES)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_INPUT_BYTES, Pushing, pushing};

    const SHA: &str = "1111111111111111111111111111111111111111";
    const ZERO: &str = "0000000000000000000000000000000000000000";

    fn judged(input: &str) -> Pushing {
        pushing(input.as_bytes()).expect("read")
    }

    #[test]
    fn journal_refs_and_inbox_branches_only_is_only_journals() {
        let input = format!(
            "refs/redkiln/journal/s1 {SHA} refs/redkiln/journal/s1 {ZERO}\n\
             refs/heads/redkiln/inbox/s2 {SHA} refs/heads/redkiln/inbox/s2 {SHA}\n"
        );
        assert_eq!(judged(&input), Pushing::OnlyJournals);
    }

    #[test]
    fn any_other_remote_ref_is_other() {
        let input = format!(
            "refs/redkiln/journal/s1 {SHA} refs/redkiln/journal/s1 {ZERO}\n\
             refs/heads/main {SHA} refs/heads/main {SHA}\n"
        );
        assert_eq!(judged(&input), Pushing::Other);
    }

    #[test]
    fn a_deletion_is_judged_by_its_remote_ref() {
        let main = format!("(delete) {ZERO} refs/heads/main {SHA}\n");
        assert_eq!(judged(&main), Pushing::Other);
        let inbox = format!("(delete) {ZERO} refs/heads/redkiln/inbox/s1 {SHA}\n");
        assert_eq!(judged(&inbox), Pushing::OnlyJournals);
    }

    #[test]
    fn no_lines_is_only_journals() {
        assert_eq!(judged(""), Pushing::OnlyJournals);
    }

    #[test]
    fn a_malformed_line_is_other() {
        let three = format!("refs/redkiln/journal/s1 {SHA} refs/redkiln/journal/s1\n");
        assert_eq!(judged(&three), Pushing::Other);
        let blank = format!("\nrefs/redkiln/journal/s1 {SHA} refs/redkiln/journal/s1 {ZERO}\n");
        assert_eq!(judged(&blank), Pushing::Other);
        assert_eq!(
            pushing(&b"refs/heads/redkiln/inbox/\xff a b c\n"[..]).expect("read"),
            Pushing::Other
        );
    }

    #[test]
    fn input_past_the_cap_is_other() {
        let line = format!("refs/redkiln/journal/s1 {SHA} refs/redkiln/journal/s1 {ZERO}\n");
        let lines = usize::try_from(MAX_INPUT_BYTES).expect("fits") / line.len() + 1;
        assert_eq!(judged(&line.repeat(lines)), Pushing::Other);
    }
}
