//! The committed 1.x journals `ci-check` reads (B-28, JR-05): those in the checkout at head,
//! decoded, and the base revision's, checked against them for the append-only rule. Moved out of
//! `ci.rs` unchanged (#48).

use anyhow::{Context as _, Result};
use std::collections::HashSet;
use std::path::Path;
use weighin_core::EventEnvelope;

use super::report::Violation;
use super::verify;
use crate::{git, journal};

/// A journal in the checkout, decoded.
pub(super) struct HeadJournal {
    pub(super) path: String,
    bytes: Vec<u8>,
    pub(super) events: Vec<(u64, EventEnvelope)>,
}

/// Every journal in the checkout, and the lines of them that are not events.
pub(super) struct HeadJournals {
    pub(super) files: Vec<HeadJournal>,
    pub(super) violations: Vec<Violation>,
}

impl HeadJournals {
    pub(super) fn read(root: &Path) -> Result<Self> {
        let mut files = Vec::new();
        let mut violations = Vec::new();
        for f in journal::files(root)? {
            let (events, bad) = verify::decode(&f.path, &f.bytes);
            violations.extend(bad);
            files.push(HeadJournal {
                path: f.path,
                bytes: f.bytes,
                events,
            });
        }
        Ok(Self { files, violations })
    }

    fn bytes_of(&self, path: &str) -> Option<&[u8]> {
        self.files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.bytes.as_slice())
    }
}

/// The base journals' append-only violations, and the keys of every complete base line.
pub(super) struct BaseCheck {
    pub(super) violations: Vec<Violation>,
    pub(super) keys: HashSet<Vec<u8>>,
}

pub(super) fn check_base(root: &Path, base_sha: &str, head: &HeadJournals) -> Result<BaseCheck> {
    let mut check = BaseCheck {
        violations: Vec::new(),
        keys: HashSet::new(),
    };
    let dir = format!("{}/", journal::DIR);
    for path in tree_paths(root, base_sha, &dir)? {
        let is_journal = path
            .strip_prefix(&dir)
            // As `journal::files` picks them in the checkout.
            .is_some_and(|name| {
                !name.contains('/') && Path::new(name).extension().is_some_and(|x| x == "jsonl")
            });
        if !is_journal {
            continue;
        }
        let base = git::output(root, &["show", &format!("{base_sha}:{path}")])
            .with_context(|| format!("reading {path} at {base_sha}"))?;
        check
            .violations
            .extend(verify::verify(&path, &base, head.bytes_of(&path)));
        check.keys.extend(
            journal::lines(&base)
                .filter(|l| l.complete && !l.is_blank())
                .map(|l| verify::key(l.text)),
        );
    }
    Ok(check)
}

/// The paths of the files under `prefix` in the tree of `rev`.
pub(super) fn tree_paths(root: &Path, rev: &str, prefix: &str) -> Result<Vec<String>> {
    let listed = git::run(
        root,
        &["ls-tree", "-r", "-z", "--name-only", rev, "--", prefix],
    )?;
    Ok(listed
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}
