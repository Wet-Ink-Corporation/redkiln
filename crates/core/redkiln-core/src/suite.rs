//! `.redkiln/suite.toml` (SPEC ST-12, ST-13): the repository's class, its store and its owner.
//!
//! [`Suite::load`] never fails: a missing file, a missing key or a file that does not parse means
//! [`RepoClass::Client`], so a forgotten or broken config never publishes (ST-13). What it could
//! not use comes back as [`Warning`]s for the caller to report; nothing here prints.
//!
//! For the 2.x line, a suite.toml that does not set `repo_class` (no file, or no key) falls back
//! to the legacy `.kb/_ledger/config.json`'s `repo_class`, with [`Warning::LegacyRepoClass`]
//! (migration plan M1-F). A suite.toml that is there but cannot be used is `client`, never the
//! legacy value: it may have said `client`.

mod owner;

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;

/// The suite's config, relative to the worktree root (ST-12).
pub const SUITE_FILE: &str = ".redkiln/suite.toml";

/// weigh-in 1.x's config, whose `repo_class` is read below suite.toml for the 2.x line (M1-F).
pub const LEGACY_CONFIG: &str = ".kb/_ledger/config.json";

/// The largest config file read, in bytes (H-13); a larger one is [`Problem::TooLarge`].
pub const MAX_CONFIG_BYTES: u64 = 64 * 1024;

/// One UTF-8 byte order mark, which Windows editors write; passed over.
const BOM: char = '\u{feff}';

/// `text` past one leading [`BOM`], if it has one.
fn without_bom(text: &str) -> &str {
    text.strip_prefix(BOM).unwrap_or(text)
}

/// Whether the repository's events may leave the machine (ST-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoClass {
    /// The owner's own repository: journals, refs, boards and GitHub projections are allowed.
    Own,
    /// Anyone else's, and the default: nothing is exported, pushed, published or projected.
    Client,
}

impl RepoClass {
    /// The value as written in suite.toml: `own` or `client`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Client => "client",
        }
    }

    /// The class `value` names, exactly as written; `None` for anything else.
    fn parse(value: &str) -> Option<Self> {
        match value {
            "own" => Some(Self::Own),
            "client" => Some(Self::Client),
            _ => None,
        }
    }
}

/// Where [`Suite::repo_class`] came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassSource {
    /// suite.toml's `repo_class`.
    Suite,
    /// The legacy config's `repo_class` (M1-F), reported by [`Warning::LegacyRepoClass`].
    Legacy,
    /// Neither file set a usable value: `client` (ST-13).
    Default,
}

/// The owner's name: lower-case ASCII words (letters and digits) joined by single dashes (ST-12).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Owner(String);

impl Owner {
    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A name outside [`Owner`]'s grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an owner is lower-case ASCII words joined by single dashes")]
pub struct OwnerError;

impl FromStr for Owner {
    type Err = OwnerError;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        let word = |w: &str| {
            !w.is_empty()
                && w.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        };
        if name.split('-').all(word) {
            Ok(Self(name.to_owned()))
        } else {
            Err(OwnerError)
        }
    }
}

/// A key of suite.toml's `[suite]` table (ST-12), or the legacy config's `repo_class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// `repo_class`.
    RepoClass,
    /// `store`.
    Store,
    /// `owner`.
    Owner,
}

impl Key {
    /// The key as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepoClass => "repo_class",
            Self::Store => "store",
            Self::Owner => "owner",
        }
    }
}

/// Why a config file that is there could not be used.
#[derive(Debug, thiserror::Error)]
pub enum Problem {
    /// It could not be read (permissions, a directory in its place).
    #[error("it cannot be read")]
    Read(#[source] io::Error),
    /// It is larger than [`MAX_CONFIG_BYTES`].
    #[error("it is over {MAX_CONFIG_BYTES} bytes")]
    TooLarge,
    /// It is not UTF-8.
    #[error("it is not UTF-8")]
    NotUtf8(#[source] std::string::FromUtf8Error),
    /// suite.toml is not TOML of the expected shape (a typo, a value of the wrong type).
    #[error("it does not parse")]
    Toml(#[source] toml::de::Error),
    /// The legacy config is not JSON of the expected shape.
    #[error("it does not parse")]
    Json(#[source] serde_json::Error),
}

/// Something [`Suite::load`] could not use, or a fallback it took. Returned, never printed.
#[derive(Debug)]
pub enum Warning {
    /// The file is there but cannot be used. For suite.toml the repository is `client` and no
    /// `store` or `owner` is taken from it; for the legacy config, it is `client`.
    Unusable {
        /// The file.
        path: PathBuf,
        /// Why.
        problem: Problem,
    },
    /// A key holds a value outside its grammar, and is ignored; a `repo_class` is then `client`.
    Value {
        /// The file.
        path: PathBuf,
        /// The key.
        key: Key,
    },
    /// suite.toml sets no `repo_class`, so the legacy config's was used (M1-F, the 2.x line).
    LegacyRepoClass {
        /// The legacy config.
        path: PathBuf,
        /// The class it gave.
        class: RepoClass,
    },
}

impl Warning {
    /// The file the warning is about.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Unusable { path, .. }
            | Self::Value { path, .. }
            | Self::LegacyRepoClass { path, .. } => path,
        }
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = self.path().display();
        match self {
            Self::Unusable { problem, .. } => {
                write!(f, "{path} is not used: {problem}; the repository is client")
            }
            Self::Value { key, .. } => {
                write!(
                    f,
                    "{path}: `{}` has a value outside its grammar and is ignored",
                    key.as_str()
                )
            }
            Self::LegacyRepoClass { class, .. } => write!(
                f,
                "repo_class `{}` comes from {path}; set it in {SUITE_FILE}, as 3.0 reads only that",
                class.as_str()
            ),
        }
    }
}

/// The repository's suite config, as read from one worktree (ST-12, ST-13).
#[derive(Debug)]
pub struct Suite {
    class: RepoClass,
    class_source: ClassSource,
    store: Option<PathBuf>,
    owner: Option<Owner>,
    warnings: Vec<Warning>,
}

/// suite.toml's shape: only the `[suite]` table is read, and only its three keys; the rest of
/// the file arrives with later SPEC versions and is left alone.
#[derive(Debug, Default, Deserialize)]
struct SuiteFile {
    suite: Option<SuiteTable>,
}

#[derive(Debug, Default, Deserialize)]
struct SuiteTable {
    repo_class: Option<String>,
    store: Option<String>,
    owner: Option<String>,
}

/// The legacy config's one key read here.
#[derive(Debug, Deserialize)]
struct LegacyFile {
    repo_class: Option<String>,
}

impl Suite {
    /// The config of the worktree at `root`: `<root>/.redkiln/suite.toml`, and below it for the
    /// repository class the legacy `<root>/.kb/_ledger/config.json`.
    #[must_use]
    pub fn load(root: &Path) -> Self {
        let path = root.join(SUITE_FILE);
        let mut warnings = Vec::new();
        let table = match read_capped(&path) {
            Ok(None) => Ok(SuiteTable::default()),
            // A file with no `[suite]` table sets none of its keys: the defaults are what ST-13
            // means by a missing key, not a hidden failure.
            Ok(Some(text)) => toml::from_str::<SuiteFile>(without_bom(&text))
                .map(|file| file.suite.unwrap_or_default())
                .map_err(Problem::Toml),
            Err(problem) => Err(problem),
        };
        let table = match table {
            Ok(table) => table,
            Err(problem) => {
                warnings.push(Warning::Unusable { path, problem });
                return Self::client(warnings);
            }
        };
        let (class, class_source) = match table.repo_class.as_deref() {
            Some(value) => RepoClass::parse(value).map_or_else(
                || {
                    warnings.push(Warning::Value {
                        path: path.clone(),
                        key: Key::RepoClass,
                    });
                    (RepoClass::Client, ClassSource::Default)
                },
                |class| (class, ClassSource::Suite),
            ),
            None => legacy_class(root, &mut warnings),
        };
        let owner = table.owner.and_then(|name| {
            if let Ok(owner) = name.parse::<Owner>() {
                Some(owner)
            } else {
                // `OwnerError` says only that the name is outside the grammar; the warning names
                // the key and the file.
                warnings.push(Warning::Value {
                    path: path.clone(),
                    key: Key::Owner,
                });
                None
            }
        });
        let store = table
            .store
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        Self {
            class,
            class_source,
            store,
            owner,
            warnings,
        }
    }

    /// No usable config: `client`, nothing else set.
    const fn client(warnings: Vec<Warning>) -> Self {
        Self {
            class: RepoClass::Client,
            class_source: ClassSource::Default,
            store: None,
            owner: None,
            warnings,
        }
    }

    /// The repository class: `client` unless a file says `own` (ST-13).
    #[must_use]
    pub const fn repo_class(&self) -> RepoClass {
        self.class
    }

    /// Which file the class came from.
    #[must_use]
    pub const fn class_source(&self) -> ClassSource {
        self.class_source
    }

    /// suite.toml's `store` as written (relative to the worktree root, or starting `~/`); `None`
    /// when absent or blank.
    #[must_use]
    pub fn store(&self) -> Option<&Path> {
        self.store.as_deref()
    }

    /// suite.toml's `owner`, when it is set and in its grammar. The default ST-12 gives from
    /// git's identity is the caller's to apply.
    #[must_use]
    pub const fn owner(&self) -> Option<&Owner> {
        self.owner.as_ref()
    }

    /// What could not be used, and the fallbacks taken, in the order they were met.
    #[must_use]
    pub fn warnings(&self) -> &[Warning] {
        &self.warnings
    }
}

/// The legacy config's `repo_class` (M1-F): `client` from [`ClassSource::Default`] when the file
/// or the key is absent, or when either cannot be used (with a warning).
fn legacy_class(root: &Path, warnings: &mut Vec<Warning>) -> (RepoClass, ClassSource) {
    let path = root.join(LEGACY_CONFIG);
    let parsed = match read_capped(&path) {
        Ok(None) => return (RepoClass::Client, ClassSource::Default),
        Ok(Some(text)) => {
            serde_json::from_str::<LegacyFile>(without_bom(&text)).map_err(Problem::Json)
        }
        Err(problem) => Err(problem),
    };
    let value = match parsed {
        Ok(LegacyFile { repo_class: None }) => return (RepoClass::Client, ClassSource::Default),
        Ok(LegacyFile {
            repo_class: Some(value),
        }) => value,
        Err(problem) => {
            warnings.push(Warning::Unusable { path, problem });
            return (RepoClass::Client, ClassSource::Default);
        }
    };
    if let Some(class) = RepoClass::parse(&value) {
        warnings.push(Warning::LegacyRepoClass { path, class });
        (class, ClassSource::Legacy)
    } else {
        warnings.push(Warning::Value {
            path,
            key: Key::RepoClass,
        });
        (RepoClass::Client, ClassSource::Default)
    }
}

/// The file's text, at most [`MAX_CONFIG_BYTES`] of it read; `None` when there is no file.
fn read_capped(path: &Path) -> Result<Option<String>, Problem> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Problem::Read(e)),
    };
    let mut bytes = Vec::new();
    // One byte past the cap tells a file at the cap from a larger one.
    file.take(MAX_CONFIG_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(Problem::Read)?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_CONFIG_BYTES) {
        return Err(Problem::TooLarge);
    }
    String::from_utf8(bytes).map(Some).map_err(Problem::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder holding `suite` as its `.redkiln/suite.toml` and `legacy` as its
    /// `.kb/_ledger/config.json`, each only when given.
    fn folder(suite: Option<&str>, legacy: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        for (rel, text) in [(SUITE_FILE, suite), (LEGACY_CONFIG, legacy)] {
            if let Some(text) = text {
                let path = dir.path().join(rel);
                std::fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
                std::fs::write(&path, text).expect("write");
            }
        }
        dir
    }

    fn load(suite: Option<&str>, legacy: Option<&str>) -> Suite {
        Suite::load(folder(suite, legacy).path())
    }

    #[test]
    fn a_missing_file_means_client_with_nothing_else_set() {
        let suite = load(None, None);
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert_eq!(suite.class_source(), ClassSource::Default);
        assert_eq!(suite.store(), None);
        assert_eq!(suite.owner(), None);
        assert!(suite.warnings().is_empty(), "{:?}", suite.warnings());
    }

    #[test]
    fn the_three_keys_are_read() {
        let suite = load(
            Some(
                "[suite]\nrepo_class = \"own\"\nstore = \"../shared/events.sqlite3\"\nowner = \"ryan-britton\"\n",
            ),
            None,
        );
        assert_eq!(suite.repo_class(), RepoClass::Own);
        assert_eq!(suite.class_source(), ClassSource::Suite);
        assert_eq!(suite.store(), Some(Path::new("../shared/events.sqlite3")));
        assert_eq!(suite.owner().map(Owner::as_str), Some("ryan-britton"));
        assert!(suite.warnings().is_empty(), "{:?}", suite.warnings());
    }

    #[test]
    fn client_is_read_as_client() {
        let suite = load(
            Some("[suite]\nrepo_class = \"client\"\n"),
            Some(r#"{"repo_class":"own"}"#),
        );
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert_eq!(
            suite.class_source(),
            ClassSource::Suite,
            "the legacy file is not read"
        );
        assert!(suite.warnings().is_empty());
    }

    #[test]
    fn other_keys_and_tables_are_left_for_later_specs() {
        let suite = load(
            Some("[suite]\nrepo_class = \"own\"\nlater = 1\n\n[weigh-in]\nboard = \"local\"\n"),
            None,
        );
        assert_eq!(suite.repo_class(), RepoClass::Own);
        assert!(suite.warnings().is_empty());
    }

    #[test]
    fn a_byte_order_mark_is_passed_over() {
        let suite = load(Some("\u{feff}[suite]\nrepo_class = \"own\"\n"), None);
        assert_eq!(suite.repo_class(), RepoClass::Own);
    }

    #[test]
    fn a_file_that_does_not_parse_means_client_and_sets_nothing() {
        for text in [
            "[suite\nrepo_class = \"own\"",
            "[suite]\nrepo_class = \"own\"\nstore = 5\n",
            "suite = \"own\"\n",
            "[suite]\nrepo_class = \"own\"\nrepo_class = \"own\"\n",
        ] {
            let suite = load(Some(text), Some(r#"{"repo_class":"own"}"#));
            assert_eq!(suite.repo_class(), RepoClass::Client, "{text:?}");
            assert_eq!(
                suite.class_source(),
                ClassSource::Default,
                "never the legacy file"
            );
            assert_eq!(suite.store(), None);
            assert!(
                matches!(
                    suite.warnings(),
                    [Warning::Unusable {
                        problem: Problem::Toml(_),
                        ..
                    }]
                ),
                "{text:?}: {:?}",
                suite.warnings()
            );
        }
    }

    #[test]
    fn a_file_that_is_not_utf8_is_unusable() {
        let dir = folder(None, None);
        let path = dir.path().join(SUITE_FILE);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(&path, b"[suite]\nrepo_class = \"own\xff\"\n").expect("write");
        let suite = Suite::load(dir.path());
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert!(
            matches!(
                suite.warnings(),
                [Warning::Unusable {
                    problem: Problem::NotUtf8(_),
                    ..
                }]
            ),
            "{:?}",
            suite.warnings()
        );
    }

    #[test]
    fn a_file_over_the_cap_is_not_parsed() {
        let mut text = String::from("[suite]\nrepo_class = \"own\"\n");
        let pad = usize::try_from(MAX_CONFIG_BYTES).expect("fits");
        text.push_str(&"#".repeat(pad));
        let suite = load(Some(&text), None);
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert!(
            matches!(
                suite.warnings(),
                [Warning::Unusable {
                    problem: Problem::TooLarge,
                    ..
                }]
            ),
            "{:?}",
            suite.warnings()
        );
    }

    #[test]
    fn a_file_at_the_cap_is_parsed() {
        let head = "[suite]\nrepo_class = \"own\"\n";
        let pad = usize::try_from(MAX_CONFIG_BYTES).expect("fits") - head.len();
        let text = format!("{head}{}", "#".repeat(pad));
        assert_eq!(load(Some(&text), None).repo_class(), RepoClass::Own);
    }

    #[test]
    fn a_directory_in_the_files_place_is_unusable() {
        let dir = folder(None, None);
        std::fs::create_dir_all(dir.path().join(SUITE_FILE)).expect("a directory");
        let suite = Suite::load(dir.path());
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert!(
            matches!(
                suite.warnings(),
                [Warning::Unusable {
                    problem: Problem::Read(_),
                    ..
                }]
            ),
            "{:?}",
            suite.warnings()
        );
    }

    #[test]
    fn a_repo_class_outside_its_values_means_client_not_the_legacy_one() {
        let suite = load(
            Some("[suite]\nrepo_class = \"Own\"\n"),
            Some(r#"{"repo_class":"own"}"#),
        );
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert_eq!(suite.class_source(), ClassSource::Default);
        assert!(
            matches!(
                suite.warnings(),
                [Warning::Value {
                    key: Key::RepoClass,
                    ..
                }]
            ),
            "{:?}",
            suite.warnings()
        );
    }

    #[test]
    fn an_owner_outside_the_grammar_is_ignored_with_a_warning() {
        for owner in [
            "Ryan",
            "ryan britton",
            "-ryan",
            "ryan-",
            "ryan--b",
            "",
            "rÿan",
        ] {
            let text = format!("[suite]\nrepo_class = \"own\"\nowner = \"{owner}\"\n");
            let suite = load(Some(&text), None);
            assert_eq!(suite.owner(), None, "{owner:?}");
            assert_eq!(suite.repo_class(), RepoClass::Own, "the other keys stand");
            assert!(
                matches!(
                    suite.warnings(),
                    [Warning::Value {
                        key: Key::Owner,
                        ..
                    }]
                ),
                "{owner:?}: {:?}",
                suite.warnings()
            );
        }
    }

    #[test]
    fn owners_are_lower_case_ascii_words_joined_by_dashes() {
        for owner in ["ryan", "ryan-britton", "r2-d2", "a"] {
            assert_eq!(
                owner
                    .parse::<Owner>()
                    .map(|o| o.as_str().to_owned())
                    .ok()
                    .as_deref(),
                Some(owner)
            );
        }
        assert!(matches!("Ryan".parse::<Owner>(), Err(OwnerError)));
    }

    #[test]
    fn a_blank_store_is_no_store() {
        for store in ["", "  "] {
            let text = format!("[suite]\nstore = \"{store}\"\n");
            let suite = load(Some(&text), None);
            assert_eq!(suite.store(), None, "{store:?}");
            assert!(suite.warnings().is_empty());
        }
    }

    #[test]
    fn with_no_repo_class_in_suite_toml_the_legacy_one_is_used_with_a_warning() {
        for suite_text in [None, Some("[suite]\nowner = \"ryan\"\n"), Some("")] {
            let suite = load(
                suite_text,
                Some("\u{feff}{\"repo_class\":\"own\",\"board\":{}}"),
            );
            assert_eq!(suite.repo_class(), RepoClass::Own, "{suite_text:?}");
            assert_eq!(suite.class_source(), ClassSource::Legacy);
            assert!(
                matches!(
                    suite.warnings(),
                    [Warning::LegacyRepoClass {
                        class: RepoClass::Own,
                        ..
                    }]
                ),
                "{:?}",
                suite.warnings()
            );
        }
        let client = load(None, Some(r#"{"repo_class":"client"}"#));
        assert_eq!(client.repo_class(), RepoClass::Client);
        assert_eq!(client.class_source(), ClassSource::Legacy);
        assert_eq!(
            client.warnings().len(),
            1,
            "a legacy client is reported too"
        );
    }

    #[test]
    fn a_legacy_file_without_a_repo_class_is_no_fallback() {
        let suite = load(None, Some(r#"{"board":{"mode":"local"}}"#));
        assert_eq!(suite.repo_class(), RepoClass::Client);
        assert_eq!(suite.class_source(), ClassSource::Default);
        assert!(suite.warnings().is_empty());
    }

    #[test]
    fn an_unusable_legacy_file_means_client_with_a_warning() {
        let broken = load(None, Some("{not json"));
        assert_eq!(broken.repo_class(), RepoClass::Client);
        assert!(
            matches!(
                broken.warnings(),
                [Warning::Unusable {
                    problem: Problem::Json(_),
                    ..
                }]
            ),
            "{:?}",
            broken.warnings()
        );
        let odd = load(None, Some(r#"{"repo_class":"public"}"#));
        assert_eq!(odd.repo_class(), RepoClass::Client);
        assert!(
            matches!(
                odd.warnings(),
                [Warning::Value {
                    key: Key::RepoClass,
                    ..
                }]
            ),
            "{:?}",
            odd.warnings()
        );
    }

    #[test]
    fn warnings_name_their_file() {
        let dir = folder(Some("[suite"), None);
        let suite = Suite::load(dir.path());
        let [warning] = suite.warnings() else {
            panic!("one warning: {:?}", suite.warnings());
        };
        assert_eq!(warning.path(), dir.path().join(SUITE_FILE));
        assert!(warning.to_string().contains("suite.toml"), "{warning}");
    }
}
