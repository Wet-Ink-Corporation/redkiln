//! `skillcheck lint skills`: checks skill directories against the 21 skill design rules
//! (`docs/skill-rules.md`), under the profile of the plugin that ships them (`--plugin`).
//!
//! Per-skill checks (`checks_spec`, `checks_body`, `checks_prose`) run once per skill and are
//! cached by content hash; set-level checks (`checks_set`) run every time. A `SKILL.md` that
//! cannot be read as UTF-8 frontmatter plus body gets one finding and no further checks.

mod cache;
pub mod checker;
mod checks_body;
mod checks_prose;
mod checks_set;
mod checks_spec;
pub(crate) mod discover;
mod document;
pub mod finding;
pub(crate) mod frontmatter;
mod hooks;
pub(crate) mod listing;
pub(crate) mod read;

use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Args;

use crate::lint::checker::Checker;
use crate::lint::finding::{LintFinding, Location};
use crate::lint::hooks::hook_ids;
use crate::lint::read::{read_capped, read_text};
use crate::profile::{DEFAULT_PLUGIN, Profile};
use cache::{Cache, content_hash};
use checks_prose::Enforcers;
use discover::{SkillDir, Surroundings, discover};
use document::Skill;

/// Why `lint skills` could not run (`crate::lint::read`).
pub use crate::lint::read::LintError;

pub(crate) use checks_set::vocabulary;

/// Arguments of `skillcheck lint skills`.
#[derive(Debug, Clone, Args)]
pub struct SkillsArgs {
    /// Skill directories, or directories of skills (each subdirectory holding a SKILL.md)
    #[arg(value_name = "PATH", default_value = "plugin/skills")]
    pub paths: Vec<PathBuf>,
    /// The session-start bootstrap index to check against its 200-word budget (rule 4)
    #[arg(long, value_name = "FILE")]
    pub index: Option<PathBuf>,
    /// A hooks metadata file (`{"hooks": [{"id": ...}]}`) whose hook ids count as enforcers
    /// (rule 10) instead of bench's own hook ids
    #[arg(long, value_name = "FILE")]
    pub hooks: Option<PathBuf>,
    /// Content-hash snapshot file: unchanged skills replay their stored findings instead of
    /// being checked again
    #[arg(long, value_name = "FILE")]
    pub cache: Option<PathBuf>,
    /// The plugin whose binary, commands, hook ids and entry skills the rules read
    #[arg(long, value_name = "NAME", default_value = DEFAULT_PLUGIN)]
    pub plugin: String,
}

/// What a run found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// Every finding, sorted by file, line and checker.
    pub findings: Vec<LintFinding>,
    /// How many skill directories were linted.
    pub skills: usize,
    /// How many of them replayed cached per-skill findings.
    pub cached: usize,
}

/// Lints the skills under `args.paths`.
///
/// # Errors
/// [`LintError`] when a path is missing, a file cannot be read, or the snapshot cannot be read
/// or written. Rule violations are findings, never errors.
pub fn lint(args: &SkillsArgs, profile: &Profile) -> Result<Outcome, LintError> {
    let dirs = discover(&args.paths)?;
    let (hook_ids, mut findings) = hook_ids(args.hooks.as_deref(), profile.hook_ids)?;
    let enforcers = Enforcers {
        profile,
        hook_ids: &hook_ids,
    };
    let loaded = dirs
        .iter()
        .map(Loaded::read)
        .collect::<Result<Vec<_>, _>>()?;
    let mut skills = Vec::with_capacity(loaded.len());
    for file in &loaded {
        match file.parse() {
            Ok(skill) => skills.push(skill),
            Err(finding) => findings.push(finding),
        }
    }
    let mut cache = args.cache.as_deref().map(Cache::load).transpose()?;
    let mut cached = 0;
    for skill in &skills {
        let (skill_findings, hit) = per_skill(skill, enforcers, cache.as_mut())?;
        findings.extend(skill_findings);
        cached += usize::from(hit);
    }
    if !dirs.is_empty() {
        findings.extend(set_level(args, &dirs, &skills)?);
    }
    if let (Some(cache), Some(path)) = (cache.as_mut(), args.cache.as_deref()) {
        cache.retain(&dirs.iter().map(|d| d.dir.display().to_string()).collect());
        cache.save(path)?;
    }
    findings.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    Ok(Outcome {
        findings,
        skills: dirs.len(),
        cached,
    })
}

/// The per-skill findings, replayed from `cache` when the content hash matches. Returns
/// whether it was a cache hit.
fn per_skill(
    skill: &Skill<'_>,
    enforcers: Enforcers<'_>,
    cache: Option<&mut Cache>,
) -> Result<(Vec<LintFinding>, bool), LintError> {
    let around = Surroundings::read(skill.dir)?;
    let Some(cache) = cache else {
        return Ok((check_skill(skill, &around, enforcers), false));
    };
    let key = skill.dir.dir.display().to_string();
    let hash = content_hash(skill, &around, enforcers);
    if let Some(stored) = cache.lookup(&key, &hash) {
        return Ok((stored.to_vec(), true));
    }
    let findings = check_skill(skill, &around, enforcers);
    cache.store(key, hash, findings.clone());
    Ok((findings, false))
}

/// The per-skill checks. They read only `skill`, `around` and `enforcers` (the profile and the
/// hook ids), which is exactly what [`content_hash`] covers.
fn check_skill(
    skill: &Skill<'_>,
    around: &Surroundings,
    enforcers: Enforcers<'_>,
) -> Vec<LintFinding> {
    let mut findings = checks_spec::check(skill, enforcers.profile);
    findings.extend(checks_body::check(skill, &around.references));
    findings.extend(checks_prose::check(skill, enforcers, around.has_scripts));
    findings
}

fn set_level(
    args: &SkillsArgs,
    dirs: &[SkillDir],
    skills: &[Skill<'_>],
) -> Result<Vec<LintFinding>, LintError> {
    let names: BTreeSet<&str> = dirs.iter().map(|d| d.name.as_str()).collect();
    let root = args
        .paths
        .first()
        .map_or_else(String::new, |p| p.display().to_string());
    let mut findings: Vec<LintFinding> = checks_set::listing_budget(skills).into_iter().collect();
    findings.extend(checks_set::siblings(skills, &names));
    findings.extend(checks_set::overlap(skills));
    let index = match &args.index {
        Some(path) => Some((path.display().to_string(), read_text(path)?)),
        None => None,
    };
    findings.extend(match &index {
        None => checks_set::bootstrap_index(None, &root),
        Some((path, Ok(text))) => checks_set::bootstrap_index(Some((path, text)), &root),
        Some((path, Err(size))) => Some(LintFinding::error(
            Checker::BootstrapIndex,
            Location::new(path.as_str(), 1),
            format!("the bootstrap index is {size} bytes, far over its 200-word budget."),
            "keep only what anneal is, the entry skills with triggers, and the user-invoked \
             commands.",
        )),
    });
    findings.extend(checks_set::outside_skills(&root));
    Ok(findings)
}

/// A `SKILL.md` as read from disk.
#[derive(Debug)]
struct Loaded<'d> {
    dir: &'d SkillDir,
    file: String,
    size: u64,
    bytes: Option<Vec<u8>>,
}

impl<'d> Loaded<'d> {
    fn read(dir: &'d SkillDir) -> Result<Self, LintError> {
        let path = dir.skill_file();
        let capped = read_capped(&path)?;
        Ok(Self {
            dir,
            file: path.display().to_string(),
            size: capped.size,
            bytes: capped.bytes,
        })
    }

    /// The parsed skill, or the one finding that stops it from being checked further.
    fn parse(&self) -> Result<Skill<'_>, LintFinding> {
        let at = Location::new(self.file.as_str(), 1);
        let Some(bytes) = &self.bytes else {
            return Err(checks_body::oversized(at, &format!("{} bytes", self.size)));
        };
        let unparseable = |line, reason: &str| {
            LintFinding::error(
                Checker::Frontmatter,
                Location::new(self.file.as_str(), line),
                format!("frontmatter does not parse: {reason}."),
                "write `---`, then `key: value` lines (or `>`/`|` blocks), then `---`.",
            )
        };
        let text = std::str::from_utf8(bytes).map_err(|error| {
            let reason = format!("SKILL.md is not UTF-8 after byte {}", error.valid_up_to());
            unparseable(1, &reason)
        })?;
        let doc = frontmatter::parse(text).map_err(|e| unparseable(e.line, e.reason))?;
        Ok(Skill::new(self.dir, &self.file, self.size, doc))
    }
}
