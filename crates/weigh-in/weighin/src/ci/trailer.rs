//! Decisions recorded as `Weigh-In:` trailers in the commits of a range. git's own trailer
//! parser is not used: a squash merge puts the trailers of its commits mid-body, where git does
//! not look, so every body line is read.

use weighin_core::ItemId;

use crate::ops::Trailer;

/// One `Weigh-In: <id> decided <option>` trailer, and the commit whose message holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TrailerDecision {
    /// The decided item.
    pub(crate) id: ItemId,
    /// The option the trailer names.
    pub(crate) option: String,
    /// The full sha of the commit.
    pub(crate) commit: String,
}

/// Every trailer in `log`, the output of `git log --format=%H%x00%B%x1e`: records split on
/// `\x1e`, each a sha and a message split on `\0`, and every message line that
/// [`Trailer::parse`] accepts. Returned oldest commit first (git prints the newest first), in
/// message order within a commit.
pub(crate) fn parse_log(log: &str) -> Vec<TrailerDecision> {
    let mut commits: Vec<Vec<TrailerDecision>> = log
        .split('\x1e')
        .filter_map(|record| record.split_once('\0'))
        .map(|(sha, body)| {
            let commit = sha.trim();
            body.lines()
                .filter_map(Trailer::parse)
                .map(|(id, option)| TrailerDecision {
                    id,
                    option: option.to_owned(),
                    commit: commit.to_owned(),
                })
                .collect()
        })
        .collect();
    commits.reverse();
    commits.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decision(id: &str, option: &str, commit: &str) -> TrailerDecision {
        TrailerDecision {
            id: ItemId::parse(id).expect("a valid id"),
            option: option.to_owned(),
            commit: commit.to_owned(),
        }
    }

    #[test]
    fn trailers_mid_body_are_found_oldest_commit_first() {
        // `git log --format=%H%x00%B%x1e` prints the newest commit first.
        let log = "bbb\0Squash merge (#7)\n\n* Weigh-In: wi-0000b2 decided B\nWeigh-In: wi-0000b3 decided C\n\nMore prose.\n\x1e\naaa\0First\n\nWeigh-In: wi-0000a1 decided per series\n\x1e\nccc\0No trailer here\n\x1e\n";

        let found = parse_log(log);

        assert_eq!(
            found,
            vec![
                decision("wi-0000a1", "per series", "aaa"),
                decision("wi-0000b3", "C", "bbb"),
            ]
        );
    }
}
