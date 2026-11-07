//! Where the CLI is running: repository, common dir, worktree, branch, session and config.

mod location;

use anyhow::{Result, bail};
use redkiln_core::ids::{Host, SessionId, WorktreeId};
use redkiln_core::location::{LocateError, Workspace};
use redkiln_core::store::Wait;
use redkiln_core::suite::{self, ClassSource, Owner, Suite};
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::path::{Component, Path, PathBuf};
use weighin_core::event::{EventEnvelope, LedgerEvent};

pub(crate) use self::location::{Notice, StoreAt, Vars};
use crate::git;
use crate::text::Report;

/// git's answer to `args` in `cwd`, trimmed; `None` when git fails or prints nothing (a branch
/// on a detached head, a head before the first commit).
fn git_line(cwd: &Path, args: &[&str]) -> Option<String> {
    git::run(cwd, args)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Whose repository a folder is (decision wi-7c21e4). A closed set, so a mistyped `client` is
/// refused rather than taken for an own repository with the publishing board (#169).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum RepoClass {
    /// One of the owner's own repositories.
    Own,
    /// A client's repository: its items stay local.
    Client,
}

impl RepoClass {
    /// The class as the config file and `init` spell it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Client => "client",
        }
    }

    /// The board a repository of this class gets when `init` names no board: client
    /// repositories keep theirs local.
    #[must_use]
    pub(crate) const fn default_board(self) -> BoardMode {
        match self {
            Self::Own => BoardMode::Artifact,
            Self::Client => BoardMode::Local,
        }
    }
}

impl From<suite::RepoClass> for RepoClass {
    fn from(class: suite::RepoClass) -> Self {
        match class {
            suite::RepoClass::Own => Self::Own,
            suite::RepoClass::Client => Self::Client,
        }
    }
}

impl From<RepoClass> for suite::RepoClass {
    fn from(class: RepoClass) -> Self {
        match class {
            RepoClass::Own => Self::Own,
            RepoClass::Client => Self::Client,
        }
    }
}

impl std::fmt::Display for RepoClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a folder's board may be shown (decision wi-7c21e4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum BoardMode {
    /// Published to claude.ai as an Artifact.
    Artifact,
    /// Only the local board pane; nothing is published.
    Local,
    /// No board at all.
    Off,
}

impl BoardMode {
    /// The mode as the config file and `init` spell it.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Local => "local",
            Self::Off => "off",
        }
    }
}

impl std::fmt::Display for BoardMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct BoardConfig {
    pub(crate) mode: BoardMode,
    pub(crate) url: Option<String>,
}

impl Default for BoardConfig {
    fn default() -> Self {
        Self {
            mode: BoardMode::Artifact,
            url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Caps {
    pub(crate) chat_cards: usize,
    pub(crate) board_focus: usize,
    /// Upper bound on a context pack, in characters (about 600 tokens).
    pub(crate) context_chars: usize,
    /// Days after its newest decide or review that a standing two-way decision comes due a look
    /// in the digest; 0 turns ageing off (D-0022).
    pub(crate) review_age_days: u32,
}

impl Default for Caps {
    fn default() -> Self {
        Self {
            chat_cards: 4,
            board_focus: 5,
            context_chars: 2400,
            review_age_days: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ScanConfig {
    pub(crate) markdown: bool,
    pub(crate) todos: bool,
    /// Repo-relative path prefixes never scanned; a single-segment entry (`node_modules/`) also
    /// matches at any depth (`ops::scannable`).
    pub(crate) exclude: Vec<String>,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            markdown: true,
            todos: true,
            exclude: vec![
                "target/".into(),
                "node_modules/".into(),
                ".git/".into(),
                ".kb/".into(),
                "dist/".into(),
                "vendor/".into(),
            ],
        }
    }
}

/// `.kb/_ledger/config.json`, committed with the repo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Config {
    /// weigh-in 1.x's class, never read here and never set: the class is suite.toml's, and Core
    /// reads this key only below it (M1-F). Kept so a rewrite carries it through for a 1.x
    /// binary, and still parsed, so a typo stays a broken config (#169).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) repo_class: Option<RepoClass>,
    /// The owner below suite.toml's `owner` (ST-12).
    pub(crate) owner: String,
    pub(crate) board: BoardConfig,
    pub(crate) caps: Caps,
    pub(crate) scan: ScanConfig,
    /// Where the ledger lives, when not the default. Relative paths resolve against the folder
    /// root; `~/` is the home directory. For a folder synced by `OneDrive` or Dropbox and used
    /// from more than one computer, point this outside the synced tree.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) store: Option<String>,
    /// Whether sessions keep their events in `.kb/_ledger/journal/<session>.jsonl` (B-28).
    pub(crate) journal: bool,
}

/// The owner when git names no user either (B-31).
const FALLBACK_OWNER: &str = "ryan";

/// The owner a folder with no usable config gets (B-31): `git config user.name` as read from
/// `dir`, as a slug; else the local part of `user.email`; else [`FALLBACK_OWNER`].
fn owner_from_git(dir: &Path) -> String {
    // `--get-regexp` exits 1 when neither key is set, which is the fallback, not a failure.
    git::run(dir, &["config", "--get-regexp", r"^user\.(name|email)$"])
        .ok()
        .and_then(|listed| owner_in(&listed))
        .unwrap_or_else(|| FALLBACK_OWNER.to_owned())
}

/// The owner `git config --get-regexp` output names: the last `user.name` as a slug, else the
/// local part of the last `user.email`; `None` when neither gives a slug.
fn owner_in(listed: &str) -> Option<String> {
    let last = |key: &str| {
        listed
            .lines()
            .filter_map(|l| l.split_once(' '))
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| v)
            .next_back()
    };
    last("user.name").and_then(slug).or_else(|| {
        last("user.email")
            .and_then(|e| e.split('@').next())
            .and_then(slug)
    })
}

/// `name` as lower-case ASCII words joined with `-`: any other ASCII character separates words,
/// and characters outside ASCII are dropped. `None` when no word is left.
fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii() {
            gap = true;
        }
    }
    (!out.is_empty()).then_some(out)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_class: None,
            owner: FALLBACK_OWNER.into(),
            board: BoardConfig::default(),
            caps: Caps::default(),
            scan: ScanConfig::default(),
            store: None,
            journal: true,
        }
    }
}

impl Config {
    pub(crate) const PATH: &'static str = ".kb/_ledger/config.json";

    /// The config under `root`, or `None` when the folder has none. One leading UTF-8 byte order
    /// mark, which Windows editors and PowerShell 5 write, is passed over.
    ///
    /// # Errors
    /// Fails when the file is there but cannot be read, or does not parse as a config.
    pub(crate) fn load(root: &Path) -> Result<Option<Self>, ConfigError> {
        const BOM: &[u8] = b"\xEF\xBB\xBF";
        let path = root.join(Self::PATH);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(ConfigError::Read { path, source }),
        };
        serde_json::from_slice(bytes.strip_prefix(BOM).unwrap_or(&bytes))
            .map(Some)
            .map_err(|source| ConfigError::Parse { path, source })
    }

    /// # Errors
    /// Fails when the directory or file cannot be written.
    pub(crate) fn save(&self, root: &Path) -> Result<PathBuf> {
        let p = root.join(Self::PATH);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(self)? + "\n")?;
        Ok(p)
    }
}

/// Why a folder's `.kb/_ledger/config.json` cannot be used.
#[derive(Debug)]
pub(crate) enum ConfigError {
    /// The file is there but cannot be read (permissions, a directory in its place).
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The file was read but is not a config (a JSON typo, a value of the wrong type).
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "cannot read {}", path.display()),
            Self::Parse { path, .. } => write!(f, "cannot parse {}", path.display()),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

/// Where [`Env::config`] came from. Only `board export` treats a missing file differently from
/// the defaults (decision wi-7c21e4: the board mode must have been chosen).
#[derive(Debug)]
pub(crate) enum ConfigSource {
    /// No config file: the defaults.
    Absent,
    /// The folder's config file.
    File,
    /// A config file that cannot be used; the defaults stand in for it, which only hooks accept.
    Broken(ConfigError),
}

/// A use of the board that only some board modes allow, and how [`Env::board_allows`] words its
/// refusals.
pub(crate) struct BoardUse {
    /// The board modes that allow it.
    pub(crate) modes: &'static [BoardMode],
    /// The end of the refusal with no config, after "for a client's repository".
    pub(crate) before: &'static str,
    /// The start of the refusal under a broken config: `not exporting the board`.
    pub(crate) refusing: &'static str,
    /// Why another mode refuses, after "this repo's board mode is `<mode>`: ".
    pub(crate) other_mode: &'static str,
}

#[derive(Debug)]
pub(crate) struct Env {
    pub(crate) cwd: PathBuf,
    pub(crate) repo_root: Option<PathBuf>,
    pub(crate) common_dir: Option<PathBuf>,
    pub(crate) branch: Option<String>,
    pub(crate) head: Option<String>,
    /// The worktree's directory name, which the board names the repository by.
    pub(crate) worktree: Option<String>,
    /// The worktree's id (ID-06), which events carry: two clones can share a directory name.
    pub(crate) worktree_id: Option<WorktreeId>,
    pub(crate) session: Option<String>,
    /// Why the session a surface gave cannot be used (D10): a command stops on it, a hook runs
    /// with no session and `SessionStart` says so.
    pub(crate) session_error: Option<String>,
    pub(crate) config: Config,
    pub(crate) config_source: ConfigSource,
    /// The store, or why it could not be located (ST-01): a command stops on it, a hook writes
    /// nothing.
    pub(crate) store: Result<StoreAt, LocateError>,
    /// What locating the store took or found that the user should hear about.
    pub(crate) notices: Vec<Notice>,
    /// The worktree's `.redkiln/suite.toml` (ST-12), read with the store; `None` when the store
    /// could not be located, so the class is `client` (ST-13).
    pub(crate) suite: Option<Suite>,
    /// How long a durable write waits for another writer's lock (ST-09).
    pub(crate) wait: Wait,
    /// `--store` and the process values the store was located from, for [`Env::relocate`].
    flag: Option<PathBuf>,
    vars: Vars,
    /// The owner git names, for a folder with no usable config: asked of git only when an owner
    /// is needed (see [`Env::owner`]), so hooks and reads never spawn git for it.
    pub(crate) git_owner: OnceCell<String>,
}

/// Whether this process runs inside Cowork's cloud workspace, where the project directory is a
/// scratch folder deleted with the session and the user's files are on their computer.
#[must_use]
pub(crate) fn cowork() -> bool {
    std::env::var("CLAUDE_CODE_ENTRYPOINT").is_ok_and(|v| v == "remote_cowork")
}

/// `path` with its directory resolved: the nearest existing ancestor of its parent is
/// canonicalized and the rest folded on (`..` pops, `.` is skipped), so a file not written yet
/// resolves too. The file name is never resolved, so an in-repo symlink keeps its own name.
/// `None` when no ancestor resolves.
fn resolve(path: &Path) -> Option<PathBuf> {
    let (dir, name) = match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => (dir, Some(name)),
        _ => (path, None),
    };
    let resolved_dir = dir.ancestors().find_map(|ancestor| {
        let tail = dir.strip_prefix(ancestor).ok()?;
        std::fs::canonicalize(ancestor)
            .ok()
            .map(|resolved| fold(resolved, tail))
    })?;
    Some(match name {
        Some(n) => resolved_dir.join(n),
        None => resolved_dir,
    })
}

/// `tail` applied to `base` component by component: `..` drops the last component and `.` is
/// skipped. The tail never reached the filesystem, so its `..` cannot be left for the OS to fold.
fn fold(base: PathBuf, tail: &Path) -> PathBuf {
    tail.components().fold(base, |mut out, c| {
        match c {
            // At the filesystem root `..` stays at the root, as the OS treats it.
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
        out
    })
}

/// An anchor's path and its position suffix (`:line` or `:line:col`, or empty). Only trailing
/// numbers count as a position, so a Windows drive letter keeps its colon.
pub(crate) fn split_anchor(at: &str) -> (&str, &str) {
    let is_num = |s: &str| s.parse::<usize>().is_ok();
    let path = match at.rsplit_once(':') {
        Some((head, last)) if is_num(last) => match head.rsplit_once(':') {
            Some((p, mid)) if is_num(mid) => p,
            _ => head,
        },
        _ => at,
    };
    at.strip_prefix(path)
        .map_or((at, ""), |position| (path, position))
}

/// The variables a session id is read from when the surface gave none, in order.
const SESSION_VARS: [&str; 3] = [
    "WEIGHIN_SESSION",
    "CLAUDE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ID",
];

/// The first session a surface gives (`given`, from `given_by`, then [`SESSION_VARS`]), as
/// written. An empty value, or a `${…}` a surface did not substitute, is no session and the next
/// is read; a value the store refuses stops the search with an error naming where it came from
/// (D10: never truncated, EV-11).
fn find_session(given: Option<String>, given_by: &str) -> Result<Option<String>, String> {
    let from_vars = SESSION_VARS
        .iter()
        .filter_map(|&name| std::env::var(name).ok().map(|value| (name, value)));
    for (source, value) in given
        .map(|value| (given_by, value))
        .into_iter()
        .chain(from_vars)
    {
        match SessionId::from_surface(&value) {
            Ok(Some(_)) => return Ok(Some(value)),
            Ok(None) => {}
            Err(_) => {
                return Err(format!(
                    "the session id from {source} has a control character or is over 247 bytes, which the store refuses"
                ));
            }
        }
    }
    Ok(None)
}

impl Env {
    /// Discover the environment from `cwd` (absolute): the store as [`location`] finds it, the
    /// worktree, the config, and the session `given` by `given_by` (`--session`, or the hook's
    /// `session_id`) or the process environment.
    #[must_use]
    pub(crate) fn discover(
        cwd: &Path,
        given: Option<(String, &str)>,
        flag: Option<PathBuf>,
        vars: Vars,
    ) -> Self {
        let mut config = None;
        let found = location::locate_store(cwd, flag.as_deref(), &vars, |root| {
            let loaded = Config::load(root);
            // Copied: the config keeps its own for `init` to save back.
            let key = loaded
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .and_then(|c| c.store.clone());
            config = Some(loaded);
            key
        });
        let (store, notices, suite, workspace) = match found {
            Ok(found) => (
                Ok(found.store),
                found.notices,
                Some(found.suite),
                Some(found.workspace),
            ),
            Err(e) => (Err(e), Vec::new(), None, None),
        };
        let (repo_root, common_dir) = match workspace {
            Some(Workspace::Git { root, common_dir }) => (Some(root), Some(common_dir)),
            Some(Workspace::Folder(_)) | None => (None, None),
        };
        let root = repo_root.as_deref().unwrap_or(cwd);
        let (config, config_source) = match config.unwrap_or_else(|| Config::load(root)) {
            Ok(Some(config)) => (config, ConfigSource::File),
            Ok(None) => (Config::default(), ConfigSource::Absent),
            Err(e) => (Config::default(), ConfigSource::Broken(e)),
        };
        let (given, given_by) = given.map_or((None, ""), |(value, by)| (Some(value), by));
        let (session, session_error) = match find_session(given, given_by) {
            Ok(session) => (session, None),
            Err(e) => (None, Some(e)),
        };
        Self {
            cwd: cwd.to_path_buf(),
            branch: git_line(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"]),
            head: git_line(cwd, &["rev-parse", "--verify", "--quiet", "HEAD"]),
            worktree: repo_root
                .as_ref()
                .and_then(|r| r.file_name())
                .map(|n| n.to_string_lossy().into_owned()),
            // A toplevel ID-06 cannot hash (not UTF-8) writes no worktree.
            worktree_id: repo_root
                .as_deref()
                .and_then(|r| WorktreeId::of_toplevel(r, Host::current()).ok()),
            repo_root,
            common_dir,
            session,
            session_error,
            config,
            config_source,
            store,
            notices,
            suite,
            wait: Wait::Command,
            flag,
            vars,
            git_owner: OnceCell::new(),
        }
    }

    /// Locate the store and read suite.toml again from this config, as the next command will:
    /// after `init` rewrote its `store` or wrote suite.toml.
    pub(crate) fn relocate(&mut self) {
        let key = self.config.store.as_deref();
        let found = location::locate_store(&self.cwd, self.flag.as_deref(), &self.vars, |_| {
            key.map(str::to_owned)
        });
        match found {
            Ok(found) => {
                self.store = Ok(found.store);
                self.notices = found.notices;
                self.suite = Some(found.suite);
            }
            // Whatever the last location found is not news about a store no longer located.
            Err(e) => {
                self.store = Err(e);
                self.notices.clear();
                self.suite = None;
            }
        }
    }

    /// An environment outside git with no git asked: no tree hash, `dir` is the root, the store
    /// beside it as if `--store` named it, and `session` set.
    #[cfg(test)]
    pub(crate) fn outside_git(dir: &Path, session: &str) -> Self {
        Self {
            cwd: dir.to_path_buf(),
            repo_root: None,
            common_dir: None,
            branch: None,
            head: None,
            worktree: None,
            worktree_id: None,
            session: Some(session.to_owned()),
            session_error: None,
            config: Config::default(),
            config_source: ConfigSource::Absent,
            store: Ok(StoreAt {
                path: dir.join("events.sqlite3"),
                source: redkiln_core::location::Source::Flag,
            }),
            notices: Vec::new(),
            suite: None,
            wait: Wait::Command,
            flag: Some(dir.join("events.sqlite3")),
            vars: Vars::default(),
            // Set, so the test never asks the machine's git for an owner.
            git_owner: Config::default().owner.into(),
        }
    }

    /// The same environment, writing under `wait` (a hook's is [`Wait::Hook`]).
    #[must_use]
    pub(crate) fn with_wait(mut self, wait: Wait) -> Self {
        self.wait = wait;
        self
    }

    /// Where `.kb` atoms and the config live: the current worktree's root.
    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.repo_root.clone().unwrap_or_else(|| self.cwd.clone())
    }

    /// The store's file, as [`location`] found it.
    ///
    /// # Errors
    /// Fails when the store could not be located.
    pub(crate) fn store_path(&self) -> Result<&Path> {
        match &self.store {
            Ok(at) => Ok(&at.path),
            Err(e) => Err(anyhow::anyhow!("locating the store: {}", Report(e))),
        }
    }

    /// weighin's own files (board exports, the hook error log): `<store's directory>/weighin/`
    /// (decision D4), so they follow the store as they did in 1.x.
    ///
    /// # Errors
    /// Fails when the store could not be located.
    pub(crate) fn state_dir(&self) -> Result<PathBuf> {
        let store = self.store_path()?;
        let dir = store.parent().unwrap_or_else(|| Path::new(""));
        Ok(dir.join("weighin"))
    }

    /// The state dir of the default store, `<git-common-dir>/redkiln/weighin/`: where a hook
    /// logs a store Core refused to open, since nothing is written beside that file (D4, review
    /// F2). `None` outside git, where it would be a `.redkiln/` made in the user's folder for a
    /// store kept elsewhere.
    #[must_use]
    pub(crate) fn default_state_dir(&self) -> Option<PathBuf> {
        Some(self.common_dir.as_deref()?.join("redkiln").join("weighin"))
    }

    /// Whether the store is a path the user gave (`--store`, a variable, a config's `store`)
    /// rather than the default in `<git-common-dir>/redkiln/` or `<folder>/.redkiln/`, which
    /// are redkiln's own directories.
    #[must_use]
    pub(crate) fn store_was_given(&self) -> bool {
        use redkiln_core::location::Source;
        matches!(&self.store, Ok(at) if matches!(at.source, Source::Flag | Source::Env | Source::Config))
    }

    #[must_use]
    pub(crate) fn envelope(&self, event: LedgerEvent, actor: &str) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: actor.to_owned(),
            session_id: self.session.clone(),
            worktree: self.worktree_id.as_ref().map(ToString::to_string),
            branch: self.branch.clone(),
            head: self.head.clone(),
            ts: now(),
        }
    }

    /// The folder's owner (ST-12): suite.toml's `owner`, else [`Env::config_owner`].
    #[must_use]
    pub(crate) fn owner(&self) -> &str {
        self.suite_owner()
            .map_or_else(|| self.config_owner(), Owner::as_str)
    }

    /// suite.toml's `owner`, when it sets one in its grammar.
    #[must_use]
    pub(crate) fn suite_owner(&self) -> Option<&Owner> {
        self.suite.as_ref().and_then(Suite::owner)
    }

    /// The owner below suite.toml's: the config's, or for a folder with no usable config the one
    /// git names (B-31), asked of git the first time it is needed.
    #[must_use]
    pub(crate) fn config_owner(&self) -> &str {
        match self.config_source {
            ConfigSource::File => &self.config.owner,
            ConfigSource::Absent | ConfigSource::Broken(_) => {
                self.git_owner.get_or_init(|| owner_from_git(&self.root()))
            }
        }
    }

    /// The repository class (ST-13): suite.toml's, else weigh-in 1.x's config.json's with a
    /// notice (M1-F), else `client`; `client` too when the store could not be located.
    #[must_use]
    pub(crate) fn repo_class(&self) -> RepoClass {
        self.suite
            .as_ref()
            .map_or(RepoClass::Client, |s| s.repo_class().into())
    }

    /// Where [`Env::repo_class`] came from.
    #[must_use]
    pub(crate) fn class_source(&self) -> ClassSource {
        self.suite
            .as_ref()
            .map_or(ClassSource::Default, Suite::class_source)
    }

    /// Where the class came from, in words: `from .redkiln/suite.toml`, and so on.
    #[must_use]
    pub(crate) fn class_origin(&self) -> String {
        match self.class_source() {
            ClassSource::Suite => format!("from {}", suite::SUITE_FILE),
            ClassSource::Legacy => format!(
                "from {}, as {} sets none",
                suite::LEGACY_CONFIG,
                suite::SUITE_FILE
            ),
            ClassSource::Default => format!(
                "the default, as {} sets no usable repo_class",
                suite::SUITE_FILE
            ),
        }
    }

    /// Whether this folder's config allows `board`: a config file whose board mode is one of
    /// `board.modes`. A folder never initialised has chosen no mode, so the defaults never count.
    ///
    /// # Errors
    /// Fails when the folder has no config, a broken one, or another board mode, each refusal
    /// worded by `board`.
    pub(crate) fn board_allows(&self, board: &BoardUse) -> Result<()> {
        match &self.config_source {
            ConfigSource::File => {}
            ConfigSource::Absent => bail!(
                "no {} here, so no board mode was chosen: run `weighin init --class own`, or \
                 `weighin init --class client` for a client's repository{}",
                Config::PATH,
                board.before
            ),
            // `run` normally rejects a broken config before any command; this keeps each use
            // safe alone.
            ConfigSource::Broken(e) => bail!("{}: {}", board.refusing, Report(e)),
        }
        let mode = self.config.board.mode;
        if !board.modes.contains(&mode) {
            bail!("this repo's board mode is `{mode}`: {}", board.other_mode);
        }
        Ok(())
    }

    #[must_use]
    pub(crate) fn human(&self) -> String {
        format!("human:{}", self.owner())
    }

    /// Repo-relative, forward-slash form of a path the agent or a hook gave us.
    #[must_use]
    pub(crate) fn relative(&self, p: &str) -> String {
        let path = Path::new(p);
        let root = self.root();
        let rel = if path.is_absolute() {
            // Compare resolved paths: the surface may spell the path differently from git's
            // toplevel (8.3 short names, symlinked temp dirs, `..`). Once both resolve, that answer
            // stands, and a path outside comes back absolute. Only when nothing resolves is the
            // path compared as given.
            match resolve(path).zip(std::fs::canonicalize(&root).ok()) {
                Some((resolved_path, resolved_root)) => resolved_path
                    .strip_prefix(&resolved_root)
                    .map_or_else(|_| path.to_path_buf(), Path::to_path_buf),
                None => path
                    .strip_prefix(&root)
                    .map_or_else(|_| path.to_path_buf(), Path::to_path_buf),
            }
        } else {
            path.to_path_buf()
        };
        rel.to_string_lossy().replace('\\', "/")
    }

    /// [`Env::relative`] for a `path`, `path:line` or `path:line:col` anchor, keeping the
    /// position as given.
    #[must_use]
    pub(crate) fn relative_anchor(&self, at: &str) -> String {
        let (path, position) = split_anchor(at);
        format!("{}{position}", self.relative(path))
    }
}

/// RFC 3339 UTC, seconds precision.
#[must_use]
pub(crate) fn now() -> String {
    let t = time::OffsetDateTime::now_utc()
        .replace_nanosecond(0)
        .unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    t.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[must_use]
pub(crate) fn today() -> String {
    now().chars().take(10).collect()
}

/// Seconds between two RFC 3339 stamps, when both parse and the second is later.
#[must_use]
pub(crate) fn seconds_between(a: &str, b: &str) -> Option<u64> {
    let fmt = &time::format_description::well_known::Rfc3339;
    let a = time::OffsetDateTime::parse(a, fmt).ok()?;
    let b = time::OffsetDateTime::parse(b, fmt).ok()?;
    u64::try_from((b - a).whole_seconds()).ok()
}

/// Human age of a stamp relative to now: `12m`, `3h`, `2d`.
#[must_use]
pub(crate) fn age(ts: &str) -> String {
    match seconds_between(ts, &now()) {
        Some(s) if s < 3600 => format!("{}m", s / 60),
        Some(s) if s < 172_800 => format!("{}h", s / 3600),
        Some(s) => format!("{}d", s / 86_400),
        None => "?".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_lower_case_ascii_words_joined_by_dashes() {
        let slugs = [
            "Ana Lima",
            "  O'Brien,  Seán ",
            "J.R.R. Tolkien",
            "日本",
            "--",
        ]
        .map(slug);

        assert_eq!(
            slugs,
            [
                Some("ana-lima".to_owned()),
                Some("o-brien-sen".to_owned()),
                Some("j-r-r-tolkien".to_owned()),
                None,
                None,
            ]
        );
    }

    #[test]
    fn the_owner_is_the_git_user_name_else_the_email_local_part() {
        let both = "user.email ana@example.com\nuser.name Ana Lima\n";
        let email = "user.email j.doe@example.com\n";
        let neither = "user.name 日本\nuser.email @example.com\n";

        let owners = [both, email, neither, ""].map(owner_in);

        assert_eq!(
            owners,
            [
                Some("ana-lima".to_owned()),
                Some("j-doe".to_owned()),
                None,
                None
            ]
        );
    }

    #[test]
    fn the_last_git_user_name_wins_as_git_reads_it() {
        let listed = "user.name Global Name\nuser.name Repo Name\n";

        assert_eq!(owner_in(listed), Some("repo-name".to_owned()));
    }

    /// A git repository at `<tmp>/repo` holding `docs/plan.md`. Hooks pass paths as the surface
    /// spells them (an 8.3 short name on Windows, a symlinked temp dir on macOS, `..`
    /// components); git reports the toplevel resolved. The tests spell paths through `..`.
    fn fixture() -> (tempfile::TempDir, PathBuf, Env) {
        let tmp = tempfile::tempdir().expect("temp dir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(repo.join("docs")).expect("docs dir");
        std::fs::write(repo.join("docs/plan.md"), "# Plan\n").expect("plan written");
        git::run(&repo, &["init", "-q"]).expect("git init");
        let env = Env::discover(&repo, None, None, Vars::default());
        (tmp, repo, env)
    }

    fn roundabout(repo: &Path, rest: &str) -> String {
        repo.join("..")
            .join("repo")
            .join(rest)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn an_existing_file_spelled_differently_is_in_the_repo() {
        let (_tmp, repo, env) = fixture();
        assert_eq!(
            env.relative(&roundabout(&repo, "docs/plan.md")),
            "docs/plan.md"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn a_file_not_written_yet_spelled_differently_is_in_the_repo() {
        let (_tmp, repo, env) = fixture();
        assert_eq!(
            env.relative(&roundabout(&repo, "docs/new.md")),
            "docs/new.md"
        );
        assert_eq!(
            env.relative(&roundabout(&repo, "drafts/new.md")),
            "drafts/new.md"
        );
        assert_eq!(
            env.relative(&env.root().join("new.md").to_string_lossy()),
            "new.md"
        );
    }

    #[test]
    fn an_unresolved_tail_is_folded_onto_its_base() {
        assert_eq!(
            fold(
                PathBuf::from("/a/repo"),
                Path::new("missing/../../other/./x.md")
            ),
            PathBuf::from("/a/other/x.md")
        );
        assert_eq!(
            fold(PathBuf::from("/a/repo"), Path::new("docs/new.md")),
            PathBuf::from("/a/repo/docs/new.md")
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn dotdot_after_a_missing_directory_stays_outside() {
        // On Unix, canonicalize does not fold `..` after a directory that does not exist.
        let (_tmp, repo, env) = fixture();
        let p = repo
            .join("missing")
            .join("..")
            .join("..")
            .join("other")
            .join("x.md");
        let rel = env.relative(&p.to_string_lossy());
        assert!(
            rel.starts_with("../") || Path::new(&rel).is_absolute(),
            "{rel}"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn a_sibling_reached_through_dotdot_stays_outside() {
        let (tmp, repo, env) = fixture();
        std::fs::create_dir_all(tmp.path().join("other")).expect("other dir");
        std::fs::write(tmp.path().join("other/x.md"), "x\n").expect("x written");
        for rest in ["x.md", "y.md"] {
            let p = repo.join("..").join("other").join(rest);
            let rel = env.relative(&p.to_string_lossy());
            assert!(
                rel.starts_with("../") || Path::new(&rel).is_absolute(),
                "{rel}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn an_in_repo_symlink_keeps_its_own_name() {
        let (_tmp, repo, env) = fixture();
        std::os::unix::fs::symlink("plan.md", repo.join("docs/current.md")).expect("symlink");
        assert_eq!(
            env.relative(&roundabout(&repo, "docs/current.md")),
            "docs/current.md"
        );
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn a_file_beyond_a_directory_symlink_out_of_the_repo_is_outside() {
        // Git treats such a file as beyond a symbolic link, not in the repository; it is not
        // scanned, and nothing is annotated through the link.
        let (tmp, repo, env) = fixture();
        std::fs::create_dir_all(tmp.path().join("elsewhere")).expect("elsewhere dir");
        std::fs::write(tmp.path().join("elsewhere/f.md"), "f\n").expect("f written");
        std::os::unix::fs::symlink(tmp.path().join("elsewhere"), repo.join("link"))
            .expect("symlink");
        let rel = env.relative(&repo.join("link").join("f.md").to_string_lossy());
        assert!(Path::new(&rel).is_absolute(), "{rel}");
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn an_absolute_anchor_keeps_its_line_and_column() {
        let (_tmp, repo, env) = fixture();
        assert_eq!(
            env.relative_anchor(&roundabout(&repo, "docs/plan.md:12:4")),
            "docs/plan.md:12:4"
        );
        assert_eq!(
            env.relative_anchor(&roundabout(&repo, "docs/new.md:3")),
            "docs/new.md:3"
        );
        assert_eq!(env.relative_anchor("src/a.rs:7"), "src/a.rs:7");
        assert_eq!(env.relative_anchor("docs/plan.md"), "docs/plan.md");
    }

    fn write_config(root: &Path, text: &str) {
        let p = root.join(Config::PATH);
        std::fs::create_dir_all(p.parent().expect("the config path has a parent"))
            .expect("config dir");
        std::fs::write(p, text).expect("config written");
    }

    #[test]
    fn a_folder_with_no_config_loads_none() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;

        let loaded = Config::load(tmp.path())?;

        assert!(loaded.is_none(), "{loaded:?}");
        Ok(())
    }

    #[test]
    fn a_config_that_parses_loads_its_values() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        write_config(
            tmp.path(),
            r#"{"repo_class":"client","board":{"mode":"local"},"store":"~/l.sqlite3"}"#,
        );

        let c = Config::load(tmp.path())?.ok_or("the file is there")?;

        assert_eq!(
            (
                c.repo_class.map(RepoClass::as_str),
                c.board.mode.as_str(),
                c.store.as_deref()
            ),
            (Some("client"), "local", Some("~/l.sqlite3"))
        );
        Ok(())
    }

    #[test]
    fn a_config_from_before_review_ages_reviews_after_90_days()
    -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        write_config(
            tmp.path(),
            r#"{"caps":{"chat_cards":3,"board_focus":5,"context_chars":2400}}"#,
        );

        let c = Config::load(tmp.path())?.ok_or("the file is there")?;

        assert_eq!((c.caps.chat_cards, c.caps.review_age_days), (3, 90));
        Ok(())
    }

    #[test]
    fn a_config_sets_its_own_review_age() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        write_config(tmp.path(), r#"{"caps":{"review_age_days":0}}"#);

        let c = Config::load(tmp.path())?.ok_or("the file is there")?;

        assert_eq!((c.caps.review_age_days, c.caps.context_chars), (0, 2400));
        Ok(())
    }

    #[test]
    fn a_config_with_a_byte_order_mark_loads() -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        // What PowerShell 5's `Set-Content -Encoding UTF8` and some Windows editors write.
        write_config(
            tmp.path(),
            "\u{feff}{\"repo_class\":\"client\",\"board\":{\"mode\":\"local\"}}",
        );

        let c = Config::load(tmp.path())?.ok_or("the file is there")?;

        assert_eq!(
            (c.repo_class.map(RepoClass::as_str), c.board.mode.as_str()),
            (Some("client"), "local")
        );
        Ok(())
    }

    #[test]
    fn a_config_that_does_not_parse_is_a_parse_error_naming_the_file() {
        let tmp = tempfile::tempdir().expect("temp dir");
        write_config(tmp.path(), r#"{"board":"local","store":"../l.sqlite3"}"#);

        let e = Config::load(tmp.path()).expect_err("a string is not a board");

        assert!(
            matches!(&e, ConfigError::Parse { path, .. } if *path == tmp.path().join(Config::PATH)),
            "{e:?}"
        );
        assert!(e.to_string().contains("config.json"), "{e}");
    }

    #[test]
    fn a_config_with_an_unknown_repo_class_is_a_parse_error_naming_the_file() {
        let tmp = tempfile::tempdir().expect("temp dir");
        // A typo for `client`: before #169 it loaded and took the `artifact` board.
        write_config(tmp.path(), r#"{"repo_class":"clinet"}"#);

        let e = Config::load(tmp.path()).expect_err("`clinet` is not a repo class");

        assert!(
            matches!(&e, ConfigError::Parse { path, .. } if *path == tmp.path().join(Config::PATH)),
            "{e:?}"
        );
    }

    #[test]
    fn a_config_with_an_unknown_board_mode_is_a_parse_error_naming_the_file() {
        let tmp = tempfile::tempdir().expect("temp dir");
        write_config(tmp.path(), r#"{"board":{"mode":"locl"}}"#);

        let e = Config::load(tmp.path()).expect_err("`locl` is not a board mode");

        assert!(
            matches!(&e, ConfigError::Parse { path, .. } if *path == tmp.path().join(Config::PATH)),
            "{e:?}"
        );
    }

    #[test]
    fn a_config_that_cannot_be_read_is_a_read_error_naming_the_file() {
        let tmp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(tmp.path().join(Config::PATH)).expect("a directory in its place");

        let e = Config::load(tmp.path()).expect_err("a directory is not a file");

        assert!(
            matches!(&e, ConfigError::Read { path, .. } if *path == tmp.path().join(Config::PATH)),
            "{e:?}"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn discovery_keeps_the_defaults_and_the_error_for_a_broken_config() {
        let (_tmp, repo, _) = fixture();
        write_config(&repo, "{ not json");

        let env = Env::discover(&repo, None, None, Vars::default());

        assert_eq!(env.config.board.mode, BoardMode::Artifact);
        assert!(
            matches!(
                env.config_source,
                ConfigSource::Broken(ConfigError::Parse { .. })
            ),
            "{:?}",
            env.config_source
        );
    }

    #[test]
    fn the_default_state_dir_is_in_the_git_common_dir_whatever_chose_the_store()
    -> Result<(), Box<dyn std::error::Error>> {
        let (_tmp, repo, _) = fixture();
        let expected = std::fs::canonicalize(repo.join(".git"))?
            .join("redkiln")
            .join("weighin");
        write_config(&repo, r#"{"store":"../elsewhere/ledger.sqlite3"}"#);
        let by_key = Env::discover(&repo, None, None, Vars::default());
        let by_flag = Env::discover(
            &repo,
            None,
            Some(repo.join("flagged/events.sqlite3")),
            Vars::default(),
        );

        for (env, chose) in [(&by_key, "config.json's store"), (&by_flag, "--store")] {
            let dir = env
                .default_state_dir()
                .ok_or_else(|| format!("{chose}: no default state dir"))?;
            let [redkiln, weighin] = ["redkiln", "weighin"].map(std::ffi::OsStr::new);
            assert_eq!(dir.file_name(), Some(weighin), "{chose}: {}", dir.display());
            let common = dir.parent().and_then(Path::parent).ok_or("two levels up")?;
            assert_eq!(
                dir.parent().and_then(Path::file_name),
                Some(redkiln),
                "{chose}"
            );
            assert_eq!(
                std::fs::canonicalize(common)?
                    .join("redkiln")
                    .join("weighin"),
                expected,
                "{chose}"
            );
        }
        Ok(())
    }

    #[test]
    fn a_relocation_that_fails_drops_the_notices_of_the_last_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let tmp = tempfile::tempdir()?;
        std::fs::create_dir_all(tmp.path().join(".weighin"))?;
        std::fs::write(tmp.path().join(".weighin/ledger.sqlite3"), b"1.x")?;
        let mut env = Env::discover(tmp.path(), None, None, Vars::default());
        assert_eq!(env.notices.len(), 1, "the old ledger: {:?}", env.notices);

        // A `~/` key with no home directory cannot be resolved.
        env.config.store = Some("~/proj.sqlite3".to_owned());
        env.relocate();

        assert!(env.store.is_err(), "{:?}", env.store);
        assert_eq!(env.notices, [], "notices of a store no longer located");
        Ok(())
    }

    #[test]
    fn anchors_split_into_path_and_position() {
        assert_eq!(split_anchor("src/a.rs:88"), ("src/a.rs", ":88"));
        assert_eq!(split_anchor("src/a.rs:88:4"), ("src/a.rs", ":88:4"));
        assert_eq!(split_anchor("C:/r/a.cs:12"), ("C:/r/a.cs", ":12"));
        assert_eq!(split_anchor("C:/r/a.cs"), ("C:/r/a.cs", ""));
        assert_eq!(split_anchor("docs/plan.md"), ("docs/plan.md", ""));
        assert_eq!(split_anchor("a:b:12"), ("a:b", ":12"));
    }
}
