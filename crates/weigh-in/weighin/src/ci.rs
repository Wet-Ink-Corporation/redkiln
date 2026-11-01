//! `weighin ci-check --base <ref>`: the branch report CI reads (B-28). It reads only git and the
//! checkout, never the ledger, since CI has none: the open items of the journals at head, the
//! decisions the branch adds (from journals, atoms and trailers, each with its commit), and
//! whether every complete journal line at base is still there at head. Its findings are data:
//! it exits 0 whenever it writes a report, and 1 only on its own errors.

mod atom;
mod report;
mod trailer;
mod verify;

use anyhow::{Context as _, Result, anyhow};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::Path;
use weighin_core::{EventEnvelope, ItemId, LedgerEvent};

use self::report::{Attested, Inputs, Source, When};
use self::verify::Violation;
use crate::env::Env;
use crate::{git, journal};

/// Write the report on the branch at HEAD against `base`: JSON when `json`, else one summary
/// line and a line per violation.
///
/// # Errors
/// Fails when the folder is not a git worktree, `base` names no commit, git fails, a journal or
/// atom cannot be read, or `out` cannot be written.
pub(crate) fn run(env: &Env, base: &str, json: bool, out: &mut impl Write) -> Result<()> {
    let root = env
        .repo_root
        .as_deref()
        .ok_or_else(|| anyhow!("ci-check needs a git repository"))?;
    let base_sha = commit(root, &format!("{base}^{{commit}}"))
        .with_context(|| format!("resolving --base {base}"))?;
    let head_sha = commit(root, "HEAD").context("resolving HEAD")?;
    let order = Order::of(root, format!("{base_sha}..{head_sha}"))?;

    let head = HeadJournals::read(root)?;
    let base_check = check_base(root, &base_sha, &head)?;
    let (mut attested, reopens) = journal_records(root, &head, &base_check.keys, &order)?;
    let (from_atoms, mut closing) = atom_decisions(root, &base_sha, atoms(root)?, &order)?;
    attested.extend(from_atoms);
    let log = git::run(root, &["log", "--format=%H%x00%B%x1e", &order.range])?;
    for t in trailer::parse_log(&log) {
        // The id is both a closing item and an attestation's: two owners.
        latest(&mut closing, t.id.clone(), order.when(Some(&t.commit)));
        attested.push(Attested {
            id: t.id,
            option: Some(t.option),
            source: Source::Trailer,
            commit: Some(t.commit),
        });
    }
    let HeadJournals {
        files,
        mut violations,
    } = head;
    let events = files
        .into_iter()
        .flat_map(|f| f.events.into_iter().map(|(_, e)| e))
        .collect();
    let ledger = journal::pool(Vec::new(), events)?;
    violations.extend(base_check.violations);
    let report = report::build(Inputs {
        base: &base_sha,
        head: &head_sha,
        ledger: &ledger,
        closing: &closing,
        reopens: &reopens,
        attested: &attested,
        violations: &violations,
    });
    if json {
        // Encoded first and written as text: serde's writer would hide a closed stdout inside
        // its own error, and `report` would no longer tell it from a failure.
        writeln!(out, "{}", serde_json::to_string(&report)?)?;
    } else {
        report::render_text(&report, out)?;
    }
    Ok(())
}

/// The range, `<base>..<head>` by full shas, and its commits in time order, so records can be
/// placed in it.
struct Order {
    range: String,
    index: HashMap<String, usize>,
}

impl Order {
    /// The commits of `range`, oldest first.
    fn of(root: &Path, range: String) -> Result<Self> {
        let listed = git::run(root, &["rev-list", "--reverse", &range])?;
        let index = listed
            .lines()
            .enumerate()
            .map(|(n, sha)| (sha.to_owned(), n))
            .collect();
        Ok(Self { range, index })
    }

    /// When a record added by `commit` entered the history: `None` is not yet committed, and a
    /// commit outside the range is from before it.
    fn when(&self, commit: Option<&str>) -> When {
        match commit {
            None => When::Uncommitted,
            Some(sha) => self.index.get(sha).map_or(When::Base, |n| When::Commit(*n)),
        }
    }
}

/// Keep the later of `when` and what `map` holds for `id`.
fn latest(map: &mut BTreeMap<ItemId, When>, id: ItemId, when: When) {
    let slot = map.entry(id).or_insert(when);
    *slot = (*slot).max(when);
}

/// The full sha of the commit `rev` names.
fn commit(root: &Path, rev: &str) -> Result<String> {
    let sha = git::run(root, &["rev-parse", "--verify", "--end-of-options", rev])?;
    Ok(sha.trim().to_owned())
}

/// A journal in the checkout, decoded.
struct HeadJournal {
    path: String,
    bytes: Vec<u8>,
    events: Vec<(u64, EventEnvelope)>,
}

/// Every journal in the checkout, and the lines of them that are not events.
struct HeadJournals {
    files: Vec<HeadJournal>,
    violations: Vec<Violation>,
}

impl HeadJournals {
    fn read(root: &Path) -> Result<Self> {
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
struct BaseCheck {
    violations: Vec<Violation>,
    keys: HashSet<Vec<u8>>,
}

fn check_base(root: &Path, base_sha: &str, head: &HeadJournals) -> Result<BaseCheck> {
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
fn tree_paths(root: &Path, rev: &str, prefix: &str) -> Result<Vec<String>> {
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

/// What the journals at head record beyond the folded items: each `item_decided` event no
/// complete base line holds, with the commit that added its line, and when each item was last
/// reopened. A reopen a base line holds is from before the range.
fn journal_records(
    root: &Path,
    head: &HeadJournals,
    base_keys: &HashSet<Vec<u8>>,
    order: &Order,
) -> Result<(Vec<Attested>, BTreeMap<ItemId, When>)> {
    let tracked: HashSet<String> = tree_paths(root, "HEAD", journal::DIR)?
        .into_iter()
        .collect();
    let mut attested = Vec::new();
    let mut reopens = BTreeMap::new();
    for file in &head.files {
        let mut new = Vec::new();
        for (n, e) in &file.events {
            let at_base = base_keys.contains(&e.to_json()?);
            match &e.event {
                LedgerEvent::ItemReopened(r) if at_base => {
                    latest(&mut reopens, r.item_id.clone(), When::Base);
                }
                LedgerEvent::ItemDecided(_) | LedgerEvent::ItemReopened(_) if !at_base => {
                    new.push((*n, &e.event));
                }
                _ => {}
            }
        }
        if new.is_empty() {
            continue;
        }
        let mut blame = blame(root, &file.path, &tracked)?;
        for (n, event) in new {
            let commit = blame.remove(&n);
            // The ids and the option are copied: the event goes on to the fold of open items.
            match event {
                LedgerEvent::ItemReopened(r) => {
                    latest(
                        &mut reopens,
                        r.item_id.clone(),
                        order.when(commit.as_deref()),
                    );
                }
                LedgerEvent::ItemDecided(d) => attested.push(Attested {
                    id: d.item_id.clone(),
                    option: Some(d.option.clone()),
                    source: Source::Journal,
                    commit,
                }),
                _ => {}
            }
        }
    }
    Ok((attested, reopens))
}

/// The commit of each line of the journal at `path`: the working tree's blame, so its line
/// numbers are those of the bytes read. A journal not committed at HEAD has none to name.
fn blame(root: &Path, path: &str, tracked: &HashSet<String>) -> Result<HashMap<u64, String>> {
    if !tracked.contains(path) {
        return Ok(HashMap::new());
    }
    let porcelain = git::run(root, &["blame", "--porcelain", "--", path])?;
    Ok(report::blame_lines(&porcelain))
}

/// The accepted decision atoms in the checkout's `.kb`, by repo-relative path. The walk skips
/// `_ledger` and does not follow symbolic links, so it ends (H-13).
fn atoms(root: &Path) -> Result<Vec<(String, atom::AtomDecision)>> {
    let mut found = Vec::new();
    let mut stack = vec![root.join(".kb")];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
        };
        for entry in entries {
            let entry = entry.with_context(|| format!("reading {}", dir.display()))?;
            let path = entry.path();
            let kind = entry
                .file_type()
                .with_context(|| format!("reading {}", path.display()))?;
            if kind.is_dir() && entry.file_name() != "_ledger" {
                stack.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|x| x == "md") {
                let bytes =
                    std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
                // A file that is not UTF-8 is not an atom the writer made.
                if let Some(a) = std::str::from_utf8(&bytes).ok().and_then(atom::parse) {
                    found.push((relative(root, &path), a));
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

/// `path` relative to `root`, with `/` separators.
fn relative(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The attestations of the atoms at head that the base tree does not have, each with the
/// commit that added it, and the items every atom at head closes.
fn atom_decisions(
    root: &Path,
    base_sha: &str,
    atoms: Vec<(String, atom::AtomDecision)>,
    order: &Order,
) -> Result<(Vec<Attested>, BTreeMap<ItemId, When>)> {
    let at_base: HashSet<String> = tree_paths(root, base_sha, ".kb/")?.into_iter().collect();
    let log = git::run(
        root,
        &[
            "log",
            "--diff-filter=A",
            "--no-renames",
            "-z",
            "--format=commit %H",
            "--name-only",
            &order.range,
            "--",
            ".kb/",
        ],
    )?;
    let added = added_by(&log);
    let mut attested = Vec::new();
    let mut closing = BTreeMap::new();
    for (path, a) in atoms {
        let when = if at_base.contains(&path) {
            When::Base
        } else {
            let commit = added.get(path.as_str()).copied();
            attested.push(Attested {
                // Both a closing item and an attestation's: two owners.
                id: a.id.clone(),
                option: a.option,
                source: Source::Atom,
                commit: commit.map(str::to_owned),
            });
            order.when(commit)
        };
        latest(&mut closing, a.id, when);
    }
    Ok((attested, closing))
}

/// The commit that added each path, from `git log -z --format='commit %H' --name-only -- .kb/`:
/// fields split on NUL, so a path git would quote comes through as it is, each commit's
/// `commit <sha>` field followed by a newline and its paths. git prints the newest commit
/// first, so the oldest is kept. A path cannot pass for a commit field: every path starts `.kb/`.
fn added_by(log: &str) -> HashMap<&str, &str> {
    let mut added = HashMap::new();
    let mut current = None;
    for field in log.split('\0').map(|f| f.trim_start_matches('\n')) {
        match field.strip_prefix("commit ") {
            Some(sha) => current = Some(sha),
            None => {
                if let (Some(sha), false) = (current, field.is_empty()) {
                    added.insert(field, sha);
                }
            }
        }
    }
    added
}
