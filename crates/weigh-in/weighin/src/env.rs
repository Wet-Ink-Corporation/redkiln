//! Where the CLI is running: repository, common dir, worktree, branch, session and config.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::path::{Component, Path, PathBuf};
use weighin_core::event::{EventEnvelope, LedgerEvent};

use crate::git;
use crate::text::Report;

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
    /// Client repos default to a local board (decision wi-7c21e4).
    pub(crate) repo_class: RepoClass,
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
            repo_class: RepoClass::Own,
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
    pub(crate) worktree: Option<String>,
    pub(crate) session: Option<String>,
    pub(crate) config: Config,
    pub(crate) config_source: ConfigSource,
    /// `--store`, which wins over everything else.
    pub(crate) store_override: Option<PathBuf>,
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

/// `~/x` expanded with `HOME` (or `USERPROFILE` on Windows).
fn expand_home(p: &str) -> PathBuf {
    let rest = p.strip_prefix("~/").or_else(|| p.strip_prefix("~\\"));
    match rest {
        Some(rest) => std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map_or_else(|| PathBuf::from(p), |h| PathBuf::from(h).join(rest)),
        None => PathBuf::from(p),
    }
}

impl Env {
    /// Discover the environment from `cwd`. Outside a git repository the ledger lives in
    /// `<cwd>/.weighin/` (Cowork folders, scratch directories).
    #[must_use]
    pub(crate) fn discover(cwd: &Path, session: Option<String>) -> Self {
        let repo_root = git::run(cwd, &["rev-parse", "--show-toplevel"])
            .ok()
            .map(|s| PathBuf::from(s.trim()));
        let common_dir = git::run(
            cwd,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )
        .ok()
        .map(|s| PathBuf::from(s.trim()));
        let branch = git::run(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let head = git::run(cwd, &["rev-parse", "--verify", "--quiet", "HEAD"])
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty());
        let worktree = repo_root
            .as_ref()
            .and_then(|r| r.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        // An empty value, or a `${…}` a surface did not substitute, is no session.
        let usable = |s: &str| !s.trim().is_empty() && !s.contains("${");
        let var = |k: &str| std::env::var(k).ok().filter(|s| usable(s));
        let session = session
            .filter(|s| usable(s))
            .or_else(|| var("WEIGHIN_SESSION"))
            .or_else(|| var("CLAUDE_SESSION_ID"))
            .or_else(|| var("CLAUDE_CODE_SESSION_ID"));
        let root = repo_root.clone().unwrap_or_else(|| cwd.to_path_buf());
        let (config, config_source) = match Config::load(&root) {
            Ok(Some(config)) => (config, ConfigSource::File),
            Ok(None) => (Config::default(), ConfigSource::Absent),
            Err(e) => (Config::default(), ConfigSource::Broken(e)),
        };
        Self {
            cwd: cwd.to_path_buf(),
            repo_root,
            common_dir,
            branch,
            head,
            worktree,
            session,
            config,
            config_source,
            store_override: None,
            git_owner: OnceCell::new(),
        }
    }

    /// Use `store` as the ledger, whatever the config and environment say.
    #[must_use]
    pub(crate) fn with_store(mut self, store: Option<PathBuf>) -> Self {
        self.store_override = store;
        self
    }

    /// Where `.kb` atoms and the config live: the current worktree's root.
    #[must_use]
    pub(crate) fn root(&self) -> PathBuf {
        self.repo_root.clone().unwrap_or_else(|| self.cwd.clone())
    }

    /// The live ledger, first match wins: `--store`; `WEIGHIN_STORE`; the config's `store`;
    /// `<git-common-dir>/weighin/ledger.sqlite3`, shared by every worktree and never committed
    /// (decision wi-6e2a47); outside git, `<folder>/.weighin/ledger.sqlite3`.
    #[must_use]
    pub(crate) fn store_path(&self) -> PathBuf {
        if let Some(p) = &self.store_override {
            return p.clone();
        }
        if let Ok(p) = std::env::var("WEIGHIN_STORE")
            && !p.trim().is_empty()
        {
            return expand_home(p.trim());
        }
        if let Some(p) = self.config.store.as_deref().map(str::trim)
            && !p.is_empty()
        {
            let p = expand_home(p);
            return if p.is_absolute() {
                p
            } else {
                self.root().join(p)
            };
        }
        match &self.common_dir {
            Some(c) => c.join("weighin").join("ledger.sqlite3"),
            None => self.cwd.join(".weighin").join("ledger.sqlite3"),
        }
    }

    /// The directory holding the ledger: board exports and the hook error log go beside it.
    #[must_use]
    pub(crate) fn state_dir(&self) -> PathBuf {
        self.store_path()
            .parent()
            .map_or_else(|| self.root().join(".weighin"), Path::to_path_buf)
    }

    #[must_use]
    pub(crate) fn envelope(&self, event: LedgerEvent, actor: &str) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: actor.to_owned(),
            session_id: self.session.clone(),
            worktree: self.worktree.clone(),
            branch: self.branch.clone(),
            head: self.head.clone(),
            ts: now(),
        }
    }

    /// The folder's owner: its config's, or for a folder with no usable config the one git names
    /// (B-31), asked of git the first time it is needed.
    #[must_use]
    pub(crate) fn owner(&self) -> &str {
        match self.config_source {
            ConfigSource::File => &self.config.owner,
            ConfigSource::Absent | ConfigSource::Broken(_) => {
                self.git_owner.get_or_init(|| owner_from_git(&self.root()))
            }
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

    /// # Errors
    /// Fails when the state directory cannot be created.
    pub(crate) fn ensure_state_dir(&self) -> Result<PathBuf> {
        let d = self.state_dir();
        std::fs::create_dir_all(&d).with_context(|| format!("creating {}", d.display()))?;
        Ok(d)
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
        let env = Env::discover(&repo, None);
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
                c.repo_class.as_str(),
                c.board.mode.as_str(),
                c.store.as_deref()
            ),
            ("client", "local", Some("~/l.sqlite3"))
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
            (c.repo_class.as_str(), c.board.mode.as_str()),
            ("client", "local")
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

        let env = Env::discover(&repo, None);

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
    fn anchors_split_into_path_and_position() {
        assert_eq!(split_anchor("src/a.rs:88"), ("src/a.rs", ":88"));
        assert_eq!(split_anchor("src/a.rs:88:4"), ("src/a.rs", ":88:4"));
        assert_eq!(split_anchor("C:/r/a.cs:12"), ("C:/r/a.cs", ":12"));
        assert_eq!(split_anchor("C:/r/a.cs"), ("C:/r/a.cs", ""));
        assert_eq!(split_anchor("docs/plan.md"), ("docs/plan.md", ""));
        assert_eq!(split_anchor("a:b:12"), ("a:b", ":12"));
    }
}
