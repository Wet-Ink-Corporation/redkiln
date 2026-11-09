//! Writing packaged plugins into the release repository (wi-f041fb, M1-G).

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use super::{ReleaseError, checked_name, copy_plugin, make_executable, read_manifest};
use crate::plugins;

/// Writes the packaged plugins into a checkout of the release repository.
///
/// For each plugin [`package`](super::package) staged under `packages`, `plugins/<name>/` of
/// `release_repository` is replaced whole, and
/// `.claude-plugin/marketplace.json` gets an entry `{name, source: "./plugins/<name>",
/// description}` for it, replacing those three fields of an entry with its name (its other fields
/// kept) or appended. Every other folder and entry is left as it is, so 1.x's plugins stay until
/// they retire. Returns the names written, in order.
///
/// # Errors
///
/// [`ReleaseError::Marketplace`] when `marketplace.json` has no `plugins` list of objects,
/// [`ReleaseError::Plugin`] when a packaged folder's name is not its manifest's or a
/// `plugins/<name>` in the release repository is not a folder, and the read and write errors.
pub fn assemble(packages: &Path, release_repository: &Path) -> Result<Vec<String>, ReleaseError> {
    let path = release_repository.join(".claude-plugin/marketplace.json");
    let text = fs::read_to_string(&path).map_err(|source| ReleaseError::Io {
        path: path.clone(),
        source,
    })?;
    let mut marketplace: Value =
        serde_json::from_str(&text).map_err(|source| ReleaseError::Json {
            path: path.clone(),
            source,
        })?;
    let shape = |problem: &str| ReleaseError::Marketplace {
        path: path.clone(),
        problem: problem.to_owned(),
    };
    let entries = marketplace
        .get_mut("plugins")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| shape("no `plugins` list"))?;
    let mut written = Vec::new();
    for plugin in plugins::read_plugins(packages)?.iter().filter(|p| p.folder) {
        let from = packages.join(&plugin.name);
        let manifest = read_manifest(&from)?;
        if manifest.name != plugin.name {
            return Err(ReleaseError::Plugin {
                plugin: plugin.name.clone(),
                problem: format!("its manifest names it {}", manifest.name),
            });
        }
        let name = checked_name(&plugin.name)?;
        let to = release_repository.join("plugins").join(name);
        remove_folder(name, &to)?;
        copy_plugin(plugin, &from, &to, |_| false)?;
        // An artifact download drops the executable bit, and hooks start the launcher in exec
        // form, which execs the build: both are executable whatever mode they arrived in.
        for file in plugin.files.iter().filter(|f| runs(&f.path)) {
            make_executable(&to.join(&file.path))?;
        }
        upsert(entries, name, manifest.description.as_deref())
            .ok_or_else(|| shape("an entry of `plugins` is not an object"))?;
        written.push(name.to_owned());
    }
    let mut out =
        serde_json::to_string_pretty(&marketplace).map_err(|source| ReleaseError::Json {
            path: path.clone(),
            source,
        })?;
    out.push('\n');
    fs::write(&path, out).map_err(|source| ReleaseError::Io { path, source })?;
    Ok(written)
}

/// A launcher copy or a build: a file under `libexec/` that is not a `.cmd` launcher or a `.sh`
/// script read by a skill.
fn runs(path: &str) -> bool {
    let script = Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("sh"));
    path.starts_with("libexec/") && !script
}

/// Removes the folder `to` when it exists; a symlink or file there is refused, never followed.
fn remove_folder(name: &str, to: &Path) -> Result<(), ReleaseError> {
    match fs::symlink_metadata(to) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ReleaseError::Io {
            path: to.to_owned(),
            source,
        }),
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(to).map_err(|source| ReleaseError::Io {
                path: to.to_owned(),
                source,
            })
        }
        Ok(_) => Err(ReleaseError::Plugin {
            plugin: name.to_owned(),
            problem: format!("{} is not a folder", to.display()),
        }),
    }
}

/// Sets the entry named `name` in `entries`, or appends one. `None` when an entry is not an
/// object.
fn upsert(entries: &mut Vec<Value>, name: &str, description: Option<&str>) -> Option<()> {
    let mut found = None;
    for entry in entries.iter_mut() {
        let object = entry.as_object_mut()?;
        if object.get("name").and_then(Value::as_str) == Some(name) {
            found = Some(object);
        }
    }
    let mut fresh = Map::new();
    let object = match found {
        Some(object) => object,
        None => &mut fresh,
    };
    object.insert("name".to_owned(), Value::from(name));
    object.insert(
        "source".to_owned(),
        Value::from(format!("./plugins/{name}")),
    );
    match description {
        Some(text) => object.insert("description".to_owned(), Value::from(text)),
        None => object.remove("description"),
    };
    if !fresh.is_empty() {
        entries.push(Value::Object(fresh));
    }
    Some(())
}
