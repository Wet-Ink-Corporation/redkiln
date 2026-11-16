//! `.redkiln/bench/config.json`, the repository's Bench settings.
//!
//! anneal's `.anneal/config.json` is read as a 0.x fallback (`repo_file`). This module holds
//! its bounded read (`read_text`), shared with `hook::fast_check` (`check_fast`), and the merge
//! gate's settings (`gate`):
//!
//! ```json
//! {"review": {"require_plugin": false},
//!  "ship": {"sensitive_globs": ["**/billing/**"]}}
//! ```
//!
//! `review.require_plugin` (default `false`): while false, a human `approved` verdict satisfies
//! `ship`'s review requirement; once true, only the review plugin's verdict does.
//! `ship.sensitive_globs` (default none): paths that force a full review, added to the
//! skip-rule's defaults (`ship::skip_rule::DEFAULT_SENSITIVE`), never replacing them. Each
//! reader ignores the other readers' keys, but `review` and `ship` are read strictly, so a
//! misspelt switch is an error rather than its default.
//!
//! `orchestrate.leads` (default `["orchestrate-lead"]`, at most 64): the subagent types the
//! brief hook lets spawn from inside a subagent (`orchestrate_leads`); a configured list
//! replaces the default, and `orchestrate` is read strictly too.
//!
//! Every error names the file read: the new one, or the old one when only it exists.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use redkiln_guard::glob::{Glob, GlobError};

use crate::repo_file;

/// The config file Bench reads, relative to the worktree root.
pub(crate) const CONFIG_PATH: &str = repo_file::CONFIG.path;

/// Most bytes of the config read.
const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// Most globs `ship.sensitive_globs` may list (H-13).
pub(crate) const MAX_SENSITIVE_GLOBS: usize = 256;

/// Why the config file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum FileError {
    /// The file exists but cannot be read.
    #[error("{path} cannot be read")]
    Read {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// The path is not a regular file (a directory, a symlink, a FIFO): never opened.
    #[error("{path} is not a regular file")]
    NotAFile {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
    /// The file resolves outside the worktree's own folder (a symlinked `.redkiln`,
    /// `.redkiln/bench` or `.anneal`): never opened, as the standing-orders and brief readers
    /// refuse theirs.
    #[error(
        "{path} resolves to {}, not this worktree's own {path}; make {dir} a plain directory",
        resolved.display(),
        dir = path.rsplit_once('/').map_or(*path, |(dir, _)| dir)
    )]
    NotOwn {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// Where it resolves.
        resolved: PathBuf,
    },
    /// The file is over `MAX_CONFIG_BYTES` (64 KiB).
    #[error("{path} is larger than {MAX_CONFIG_BYTES} bytes")]
    TooLarge {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
}

/// The config's text and the file it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfigText {
    /// The file read, relative to the worktree root.
    pub(crate) path: &'static str,
    /// Its text (lossy UTF-8).
    pub(crate) text: String,
}

/// The config in the worktree `root` (the new file, else the old one: `repo_file::CONFIG`);
/// `None` when there is neither.
///
/// # Errors
/// The [`FileError`] when the path is not a regular file, resolves outside the worktree's own
/// folder, cannot be read or is too large.
pub(crate) fn read_text(root: &Path) -> Result<Option<ConfigText>, FileError> {
    let chosen = repo_file::CONFIG.choose(root).path;
    let path = repo_file::under(root, chosen);
    let read = |source| FileError::Read {
        path: chosen,
        source,
    };
    // Checked before opening: a FIFO would block a hook, and a directory opens on unix only.
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(FileError::NotAFile { path: chosen }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read(error)),
    }
    let resolved = fs::canonicalize(&path).map_err(read)?;
    let own = repo_file::under(&fs::canonicalize(root).map_err(read)?, chosen);
    if resolved != own {
        return Err(FileError::NotOwn {
            path: chosen,
            resolved,
        });
    }
    let file = File::open(&path).map_err(read)?;
    let mut bytes = Vec::new();
    file.take(MAX_CONFIG_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(read)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_CONFIG_BYTES) {
        return Err(FileError::TooLarge { path: chosen });
    }
    Ok(Some(ConfigText {
        path: chosen,
        text: String::from_utf8_lossy(&bytes).into_owned(),
    }))
}

/// The merge gate's settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GateConfig {
    /// `review.require_plugin`: only the review plugin's verdict satisfies the review
    /// requirement.
    pub(crate) require_plugin: bool,
    /// `ship.sensitive_globs`: added to the skip-rule's default sensitive globs.
    pub(crate) sensitive_globs: Vec<Glob>,
}

/// The keys [`gate`] reads; the rest of the file belongs to other readers.
#[derive(Debug, Default, Deserialize)]
struct GateDto {
    #[serde(default)]
    review: ReviewDto,
    #[serde(default)]
    ship: ShipDto,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDto {
    #[serde(default)]
    require_plugin: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShipDto {
    #[serde(default)]
    sensitive_globs: Vec<String>,
}

/// Why the merge gate's settings could not be read.
#[derive(Debug, thiserror::Error)]
pub enum GateConfigError {
    /// The file could not be read.
    #[error(transparent)]
    File(#[from] FileError),
    /// Not a JSON object, or `review` or `ship` is not as documented.
    #[error(
        "{path}: `review` must be {{\"require_plugin\": <bool>}} and `ship` \
         {{\"sensitive_globs\": [<glob>, ...]}}"
    )]
    Json {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// What did not parse.
        #[source]
        source: serde_json::Error,
    },
    /// `ship.sensitive_globs` lists more than `MAX_SENSITIVE_GLOBS` (256).
    #[error("{path}: ship.sensitive_globs lists more than {MAX_SENSITIVE_GLOBS} globs")]
    TooManyGlobs {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
    /// A glob in `ship.sensitive_globs` was refused.
    #[error("{path}: ship.sensitive_globs {glob:?}")]
    Glob {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// The glob as written.
        glob: String,
        /// Why it was refused.
        #[source]
        source: GlobError,
    },
}

/// The merge gate's settings from the config in the worktree `root`; the defaults when there
/// is no config file.
///
/// # Errors
/// The [`GateConfigError`] when the file cannot be read or its settings are not valid.
pub(crate) fn gate(root: &Path) -> Result<GateConfig, GateConfigError> {
    match read_text(root)? {
        Some(config) => gate_in(config.path, &config.text),
        None => Ok(GateConfig::default()),
    }
}

/// The merge gate's settings from the text of [`CONFIG_PATH`].
///
/// # Errors
/// As [`gate`], for the settings.
#[cfg(test)]
pub(crate) fn gate_from_text(text: &str) -> Result<GateConfig, GateConfigError> {
    gate_in(CONFIG_PATH, text)
}

/// The merge gate's settings from `text`, read from `path`.
///
/// # Errors
/// As [`gate`], for the settings.
fn gate_in(path: &'static str, text: &str) -> Result<GateConfig, GateConfigError> {
    let dto: GateDto =
        serde_json::from_str(text).map_err(|source| GateConfigError::Json { path, source })?;
    if dto.ship.sensitive_globs.len() > MAX_SENSITIVE_GLOBS {
        return Err(GateConfigError::TooManyGlobs { path });
    }
    let sensitive_globs = dto
        .ship
        .sensitive_globs
        .into_iter()
        .map(|glob| match Glob::new(&glob) {
            Ok(compiled) => Ok(compiled),
            // The error owns the glob it names.
            Err(source) => Err(GateConfigError::Glob { path, glob, source }),
        })
        .collect::<Result<_, _>>()?;
    Ok(GateConfig {
        require_plugin: dto.review.require_plugin,
        sensitive_globs,
    })
}

/// The `agent_type`s that may spawn a subagent from inside a subagent when the config names
/// none (`orchestrate.leads`).
pub(crate) const DEFAULT_LEADS: [&str; 1] = ["orchestrate-lead"];

/// Most names `orchestrate.leads` may list (H-13).
pub(crate) const MAX_LEADS: usize = 64;

/// The key the brief hook reads; the rest of the file belongs to other readers.
#[derive(Debug, Default, Deserialize)]
struct LeadsDto {
    #[serde(default)]
    orchestrate: OrchestrateDto,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrchestrateDto {
    #[serde(default)]
    leads: Option<Vec<String>>,
}

/// Most bytes of one name in `orchestrate.leads` (H-13).
pub(crate) const MAX_LEAD_BYTES: usize = 128;

/// Why `orchestrate.leads` could not be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LeadsError {
    /// The file could not be read.
    #[error(transparent)]
    File(#[from] FileError),
    /// Not a JSON object, or `orchestrate` is not as documented.
    #[error("{path}: `orchestrate` must be {{\"leads\": [<agent type>, ...]}}")]
    Json {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// What did not parse.
        #[source]
        source: serde_json::Error,
    },
    /// `orchestrate.leads` lists more than `MAX_LEADS` (64) names.
    #[error("{path}: orchestrate.leads lists more than {MAX_LEADS} names")]
    TooMany {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
    /// A name in `orchestrate.leads` is not one an `agent_type` can be.
    #[error(
        "{path}: orchestrate.leads[{index}] {problem}; a lead is 1 to {MAX_LEAD_BYTES} bytes \
         of printable text"
    )]
    BadName {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// The name's place in the list, from 0.
        index: usize,
        /// What is wrong with it.
        problem: NameProblem,
    },
}

/// What is wrong with a name in `orchestrate.leads`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum NameProblem {
    /// It is empty.
    #[error("is empty")]
    Empty,
    /// It is longer than [`MAX_LEAD_BYTES`].
    #[error("is longer than {MAX_LEAD_BYTES} bytes")]
    TooLong,
    /// It holds a control character, such as a line break.
    #[error("holds a control character")]
    Control,
}

/// The orchestrate leads: the `agent_type`s allowed to spawn subagents from inside a subagent,
/// each 1 to [`MAX_LEAD_BYTES`] bytes with no control character, at most [`MAX_LEADS`]. An
/// empty list lets no subagent spawn. They remember the config file they were read from (or,
/// for the default, the one that would set them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Leads {
    names: Vec<String>,
    path: &'static str,
}

impl Leads {
    /// The leads `names` read from `path`.
    ///
    /// # Errors
    /// [`LeadsError::TooMany`] over [`MAX_LEADS`] names; [`LeadsError::BadName`] for the first
    /// name that is empty, too long or holds a control character.
    fn new(path: &'static str, names: Vec<String>) -> Result<Self, LeadsError> {
        if names.len() > MAX_LEADS {
            return Err(LeadsError::TooMany { path });
        }
        let problem = |name: &str| {
            if name.is_empty() {
                Some(NameProblem::Empty)
            } else if name.len() > MAX_LEAD_BYTES {
                Some(NameProblem::TooLong)
            } else if name.chars().any(char::is_control) {
                Some(NameProblem::Control)
            } else {
                None
            }
        };
        if let Some((index, problem)) = names
            .iter()
            .enumerate()
            .find_map(|(index, name)| problem(name).map(|problem| (index, problem)))
        {
            return Err(LeadsError::BadName {
                path,
                index,
                problem,
            });
        }
        Ok(Self { names, path })
    }

    /// Whether `agent_type` is one of the leads (exactly, as Claude Code reports it).
    pub(crate) fn contains(&self, agent_type: &str) -> bool {
        self.names.iter().any(|name| name == agent_type)
    }

    /// The leads, in the configured order.
    pub(crate) fn names(&self) -> &[String] {
        &self.names
    }

    /// The config file that set them, or for the default the one that would.
    pub(crate) fn path(&self) -> &'static str {
        self.path
    }

    /// [`DEFAULT_LEADS`], as `path` would have set them.
    fn default_leads(path: &'static str) -> Self {
        Self {
            names: DEFAULT_LEADS.map(str::to_owned).to_vec(),
            path,
        }
    }
}

/// The names, joined with `, `.
impl fmt::Display for Leads {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.names().join(", "))
    }
}

/// The orchestrate leads from the config in the worktree `root`. [`DEFAULT_LEADS`] when the
/// file or the key is absent; a configured list replaces it.
///
/// # Errors
/// The [`LeadsError`] when the file cannot be read or `orchestrate` is not valid.
pub(crate) fn orchestrate_leads(root: &Path) -> Result<Leads, LeadsError> {
    match read_text(root)? {
        Some(config) => leads_in(config.path, &config.text),
        None => Ok(Leads::default_leads(CONFIG_PATH)),
    }
}

/// The orchestrate leads from the text of [`CONFIG_PATH`].
///
/// # Errors
/// As [`orchestrate_leads`], for the setting.
#[cfg(test)]
pub(crate) fn leads_from_text(text: &str) -> Result<Leads, LeadsError> {
    leads_in(CONFIG_PATH, text)
}

/// The orchestrate leads from `text`, read from `path`.
///
/// # Errors
/// As [`orchestrate_leads`], for the setting.
fn leads_in(path: &'static str, text: &str) -> Result<Leads, LeadsError> {
    let dto: LeadsDto =
        serde_json::from_str(text).map_err(|source| LeadsError::Json { path, source })?;
    match dto.orchestrate.leads {
        Some(names) => Leads::new(path, names),
        None => Ok(Leads::default_leads(path)),
    }
}

#[cfg(test)]
mod tests;
