//! The hook ids that count as enforcers (design rule 10): those of a `--hooks` metadata file,
//! else the defaults the caller gives. `lint rules` reads it; `tools/skillcheck` has its own
//! copy for `lint skills`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::lint::checker::Checker;
use crate::lint::finding::{LintFinding, Location};
use crate::lint::read::{LintError, read_text};

/// A `--hooks` file: only the hook ids matter here.
#[derive(Debug, Deserialize)]
struct HooksMetadata {
    hooks: Vec<HookEntry>,
}

#[derive(Debug, Deserialize)]
struct HookEntry {
    id: String,
}

/// The hook ids that count as enforcers: those of `hooks` when given (plus a finding when it
/// does not parse), else `defaults`.
///
/// # Errors
/// [`LintError`] when `hooks` is missing, a directory or cannot be read.
pub(crate) fn hook_ids(
    hooks: Option<&Path>,
    defaults: &[&str],
) -> Result<(BTreeSet<String>, Vec<LintFinding>), LintError> {
    match hooks {
        Some(path) => read_hook_ids(path),
        // Owned so the set has one type whether it comes from a file or from the table.
        None => Ok((
            defaults.iter().map(|&id| id.to_owned()).collect(),
            Vec::new(),
        )),
    }
}

/// The hook ids in the hooks metadata file at `path`, or no ids and one `enforcer` error
/// finding when it does not parse.
fn read_hook_ids(path: &Path) -> Result<(BTreeSet<String>, Vec<LintFinding>), LintError> {
    let parsed = read_text(path)?
        .map_err(|size| format!("the file is {size} bytes"))
        .and_then(|text| {
            serde_json::from_str::<HooksMetadata>(&text).map_err(|error| error.to_string())
        });
    match parsed {
        Ok(metadata) => Ok((
            metadata.hooks.into_iter().map(|h| h.id).collect(),
            Vec::new(),
        )),
        Err(reason) => Ok((
            BTreeSet::new(),
            vec![LintFinding::error(
                Checker::Enforcer,
                Location::new(path.display().to_string(), 1),
                format!("hooks metadata does not parse: {reason}."),
                "fix the JSON: `{\"version\": 1, \"hooks\": [{\"id\": \"<hook-id>\", ...}]}`.",
            )],
        )),
    }
}
