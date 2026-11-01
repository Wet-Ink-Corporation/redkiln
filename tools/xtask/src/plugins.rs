//! The plugin rules (`AGENTS.md`, CLI first, harness second) and a check of `plugins/` against
//! them.
//!
//! A plugin is a thin integration of its binary into the harness: manifests, hooks that run
//! `<binary> hook ...`, skills, agents and commands that tell the model which command to run,
//! launcher copies, builds and, for a binary that runs in Cowork, one fetch script its skill
//! sends to the device shell (`libexec/<binary>-cowork-fetch.sh`, M1-D). Anything else in
//! `plugins/<plugin>/` is functionality that belongs in the CLI, and fails the check. A mod plugin (its `hooks.json` lists `modules`) carries its
//! module and types instead of skills, agents or commands.

mod read;

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

pub use read::{MAX_DEPTH, MAX_FILES, MAX_TEXT, ReadError, read_plugins};

/// The only form a hook command may take, followed by the binary's name.
const HOOK_PREFIX: &str = "${CLAUDE_PLUGIN_ROOT}/libexec/";

/// The first argument every hook passes to its binary.
const HOOK_SUBCOMMAND: &str = "hook";

/// Files a plugin may carry at its root.
const ROOT_FILES: [&str; 5] = [
    "README.md",
    "CHANGELOG.md",
    "LICENSE",
    "LICENSE-MIT",
    "LICENSE-APACHE",
];

/// Extensions of the files a skill folder may carry beside its `SKILL.md`.
const SKILL_EXTENSIONS: [&str; 4] = ["md", "json", "html", "txt"];

/// Extensions of a mod plugin's module and type files.
const MOD_EXTENSIONS: [&str; 3] = ["ts", "tsx", "json"];

/// What a path in a plugin is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// A regular file.
    Regular,
    /// A symlink or anything else that is not a regular file.
    Other,
}

/// One file in a plugin as the check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginFile {
    /// The path relative to the plugin's folder, with `/` separators.
    pub path: String,
    /// What the path is on disk.
    pub kind: FileKind,
    /// The contents, for `hooks/hooks.json` and the Markdown of skills, agents and commands.
    pub text: Option<String>,
}

/// One folder under `plugins/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plugin {
    /// The folder's name.
    pub name: String,
    /// Whether it is a real folder; a file or symlink directly under `plugins/` is not.
    pub folder: bool,
    /// Every file under it, in path order.
    pub files: Vec<PluginFile>,
}

/// One way a plugin breaks the plugin rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    /// An entry directly under `plugins/` that is not a folder (a file or a symlink).
    NotAFolder {
        /// The entry's name.
        plugin: String,
    },
    /// The plugin has no `.claude-plugin/plugin.json`.
    NoManifest {
        /// The plugin folder.
        plugin: String,
    },
    /// A file that is not on the plugin allowlist.
    Unlisted {
        /// The plugin folder.
        plugin: String,
        /// The file, relative to the plugin folder.
        path: String,
    },
    /// A symlink or other non-regular file.
    NotRegularFile {
        /// The plugin folder.
        plugin: String,
        /// The path, relative to the plugin folder.
        path: String,
    },
    /// A mod plugin that also carries a skill, agent or command.
    ModWithSkills {
        /// The plugin folder.
        plugin: String,
        /// The skill, agent or command file.
        path: String,
    },
    /// `hooks/hooks.json` could not be read as hooks.
    HooksUnreadable {
        /// The plugin folder.
        plugin: String,
        /// Why it could not be read.
        reason: String,
    },
    /// A hook that does not run the plugin's binary as `<binary> hook ...`.
    HookNotBinary {
        /// The plugin folder.
        plugin: String,
        /// The hook event it is registered on.
        event: String,
        /// What is wrong with it.
        problem: String,
    },
    /// A skill, agent or command that names a binary the plugin ships no launcher for.
    UnknownBinary {
        /// The plugin folder.
        plugin: String,
        /// The skill, agent or command file.
        path: String,
        /// The binary it names.
        binary: String,
    },
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAFolder { plugin } => {
                write!(
                    f,
                    "{plugin}: not a folder; plugins/ holds only plugin folders"
                )
            }
            Self::NoManifest { plugin } => {
                write!(f, "{plugin}: no .claude-plugin/plugin.json")
            }
            Self::Unlisted { plugin, path } => write!(
                f,
                "{plugin}/{path}: not a manifest, hook entry, skill, agent, command, launcher, \
                 build or Cowork bootstrap; functionality belongs in the CLI"
            ),
            Self::NotRegularFile { plugin, path } => {
                write!(f, "{plugin}/{path}: not a regular file")
            }
            Self::ModWithSkills { plugin, path } => write!(
                f,
                "{plugin}/{path}: a mod plugin carries no skills, agents or commands"
            ),
            Self::HooksUnreadable { plugin, reason } => {
                write!(f, "{plugin}/hooks/hooks.json: unreadable: {reason}")
            }
            Self::HookNotBinary {
                plugin,
                event,
                problem,
            } => write!(f, "{plugin}: a {event} hook: {problem}"),
            Self::UnknownBinary {
                plugin,
                path,
                binary,
            } => write!(
                f,
                "{plugin}/{path}: names libexec/{binary}, which the plugin does not ship"
            ),
        }
    }
}

/// `hooks/hooks.json` as far as the check reads it.
#[derive(Deserialize)]
struct HooksFile {
    #[serde(default)]
    hooks: BTreeMap<String, Vec<Matcher>>,
    #[serde(default)]
    modules: Option<serde_json::Value>,
}

/// One matcher group of an event.
#[derive(Deserialize)]
struct Matcher {
    #[serde(default)]
    hooks: Vec<Hook>,
}

/// One hook entry.
#[derive(Deserialize)]
struct Hook {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
}

/// Checks every plugin against the plugin rules.
///
/// Pure: violations come in plugin order. Within a plugin they come by kind: a missing manifest,
/// an unreadable `hooks.json`, then placements in file order, then hooks in event order, then
/// unknown binaries in file order. An entry that is not a folder gives only `NotAFolder`.
#[must_use]
pub fn check(plugins: &[Plugin]) -> Vec<Violation> {
    plugins.iter().flat_map(check_plugin).collect()
}

fn check_plugin(plugin: &Plugin) -> Vec<Violation> {
    let name = || plugin.name.clone();
    if !plugin.folder {
        return vec![Violation::NotAFolder { plugin: name() }];
    }
    let mut found = Vec::new();
    if !plugin
        .files
        .iter()
        .any(|f| f.path == ".claude-plugin/plugin.json")
    {
        found.push(Violation::NoManifest { plugin: name() });
    }
    let hooks = match plugin.files.iter().find(|f| f.path == "hooks/hooks.json") {
        None => None,
        Some(file) => match parse_hooks(file) {
            Ok(hooks) => Some(hooks),
            Err(reason) => {
                found.push(Violation::HooksUnreadable {
                    plugin: name(),
                    reason,
                });
                None
            }
        },
    };
    let is_mod = hooks
        .as_ref()
        .is_some_and(|h| h.modules.as_ref().is_some_and(|m| !m.is_null()));
    let launchers = launchers(plugin);
    found.extend(
        plugin
            .files
            .iter()
            .filter_map(|file| placement(plugin, file, is_mod, &launchers)),
    );
    if let Some(hooks) = &hooks {
        found.extend(check_hooks(&plugin.name, hooks, &launchers));
    }
    found.extend(unknown_binaries(plugin, &launchers));
    found
}

/// The violation, if any, of where `file` sits in `plugin`.
fn placement(
    plugin: &Plugin,
    file: &PluginFile,
    is_mod: bool,
    launchers: &[&str],
) -> Option<Violation> {
    let (name, path) = (|| plugin.name.clone(), || file.path.clone());
    if file.kind == FileKind::Other {
        return Some(Violation::NotRegularFile {
            plugin: name(),
            path: path(),
        });
    }
    match place(&file.path, is_mod, launchers) {
        Place::Allowed => None,
        Place::Unlisted => Some(Violation::Unlisted {
            plugin: name(),
            path: path(),
        }),
        Place::SkillInMod => Some(Violation::ModWithSkills {
            plugin: name(),
            path: path(),
        }),
    }
}

/// Each binary a skill, agent or command names that the plugin ships no launcher for.
fn unknown_binaries(plugin: &Plugin, launchers: &[&str]) -> Vec<Violation> {
    let mut found = Vec::new();
    for file in plugin.files.iter().filter(|f| is_prompt(&f.path)) {
        let Some(text) = &file.text else { continue };
        for binary in named_binaries(text) {
            if !launchers.contains(&binary) {
                found.push(Violation::UnknownBinary {
                    plugin: plugin.name.clone(),
                    path: file.path.clone(),
                    binary: binary.to_owned(),
                });
            }
        }
    }
    found
}

fn parse_hooks(file: &PluginFile) -> Result<HooksFile, String> {
    let text = file.text.as_deref().ok_or("its contents were not read")?;
    serde_json::from_str(text).map_err(|error| error.to_string())
}

/// Where a path sits against the allowlist.
enum Place {
    Allowed,
    Unlisted,
    SkillInMod,
}

fn place(path: &str, is_mod: bool, launchers: &[&str]) -> Place {
    let segments: Vec<&str> = path.split('/').collect();
    let prompt = |allowed: bool| match (allowed, is_mod) {
        (false, _) => Place::Unlisted,
        (true, true) => Place::SkillInMod,
        (true, false) => Place::Allowed,
    };
    let allowed = match segments.as_slice() {
        [".claude-plugin", "plugin.json"] | ["hooks", "hooks.json"] => true,
        [file] => ROOT_FILES.contains(file),
        ["libexec", name] => {
            is_launcher(name) || bootstrap_binary(name).is_some_and(|b| launchers.contains(&b))
        }
        ["libexec", target, build] => is_build(target, build, launchers),
        ["skills", _, rest @ ..] => {
            return prompt(!rest.is_empty() && has_extension(path, &SKILL_EXTENSIONS));
        }
        ["agents" | "commands", _] => return prompt(has_extension(path, &["md"])),
        ["hooks", "mod", _, ..] | ["types", _, ..] => {
            is_mod && has_extension(path, &MOD_EXTENSIONS)
        }
        _ => false,
    };
    if allowed {
        Place::Allowed
    } else {
        Place::Unlisted
    }
}

fn has_extension(path: &str, extensions: &[&str]) -> bool {
    path.rsplit_once('/')
        .map_or(path, |(_, file)| file)
        .rsplit_once('.')
        .is_some_and(|(stem, extension)| !stem.is_empty() && extensions.contains(&extension))
}

/// A launcher copy is `libexec/<binary>` or `libexec/<binary>.cmd`.
fn is_launcher(name: &str) -> bool {
    let stem = name.strip_suffix(".cmd").unwrap_or(name);
    !stem.is_empty() && stem.chars().all(is_binary_char)
}

/// The suffix of a Cowork bootstrap, `libexec/<binary>-cowork-fetch.sh` (M1-D): the one script a
/// plugin may carry, which a skill reads and sends to Cowork's device shell to fetch its binary.
const BOOTSTRAP_SUFFIX: &str = "-cowork-fetch.sh";

/// The binary a Cowork bootstrap file name serves, or `None` when the name is not a bootstrap.
fn bootstrap_binary(name: &str) -> Option<&str> {
    name.strip_suffix(BOOTSTRAP_SUFFIX)
        .filter(|binary| !binary.is_empty() && binary.chars().all(is_binary_char))
}

/// A build is `libexec/<target>/<binary>[.exe]`: a target triple (a name with a `-`) and a binary
/// the plugin ships a launcher copy for.
fn is_build(target: &str, build: &str, launchers: &[&str]) -> bool {
    let binary = build.strip_suffix(".exe").unwrap_or(build);
    target.contains('-') && target.chars().all(is_binary_char) && launchers.contains(&binary)
}

fn is_binary_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// The binaries the plugin ships a launcher copy for.
fn launchers(plugin: &Plugin) -> Vec<&str> {
    plugin
        .files
        .iter()
        .filter_map(|f| f.path.strip_prefix("libexec/"))
        .filter(|name| !name.contains('/') && !name.contains('.') && is_launcher(name))
        .collect()
}

fn is_prompt(path: &str) -> bool {
    ["skills/", "agents/", "commands/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// Each binary a text names as `libexec/<binary>`, once, in order of first mention. A name
/// followed by `/` is a build folder, and a placeholder such as `<binary>` is not a name.
fn named_binaries(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    for (_, after) in text
        .match_indices("libexec/")
        .map(|(at, m)| text.split_at(at + m.len()))
    {
        let end = after
            .find(|c: char| !is_binary_char(c))
            .unwrap_or(after.len());
        let (name, rest) = after.split_at(end);
        // A bootstrap, `libexec/<binary>-cowork-fetch.sh` with nothing of a file name after it,
        // names the binary it serves.
        let whole_bootstrap = rest
            .strip_prefix(".sh")
            .is_some_and(|tail| !tail.starts_with(|c: char| is_binary_char(c) || c == '.'));
        let name = after
            .get(..end + ".sh".len())
            .filter(|_| whole_bootstrap)
            .and_then(bootstrap_binary)
            .unwrap_or(name);
        if !name.is_empty() && !rest.starts_with('/') && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn check_hooks(plugin: &str, hooks: &HooksFile, launchers: &[&str]) -> Vec<Violation> {
    let mut found = Vec::new();
    for (event, matchers) in &hooks.hooks {
        for hook in matchers.iter().flat_map(|m| &m.hooks) {
            if let Some(problem) = hook_problem(hook, launchers) {
                found.push(Violation::HookNotBinary {
                    plugin: plugin.to_owned(),
                    event: event.clone(),
                    problem,
                });
            }
        }
    }
    found
}

fn hook_problem(hook: &Hook, launchers: &[&str]) -> Option<String> {
    if hook.kind != "command" {
        return Some(format!("a `{}` hook, not a command", hook.kind));
    }
    let command = hook.command.as_deref().unwrap_or_default();
    let calls_binary = command
        .strip_prefix(HOOK_PREFIX)
        .is_some_and(|binary| launchers.contains(&binary));
    if !calls_binary {
        return Some(format!(
            "`{command}` is not `{HOOK_PREFIX}<binary>` with a launcher copy"
        ));
    }
    if hook.args.first().map(String::as_str) != Some(HOOK_SUBCOMMAND) {
        return Some(format!(
            "its arguments do not start with `{HOOK_SUBCOMMAND}`"
        ));
    }
    None
}
