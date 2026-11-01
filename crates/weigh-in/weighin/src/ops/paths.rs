//! Which paths are scanned, and how a path reads when a decision is written back into it.

use std::path::Path;

use crate::annotate::Syntax;
use crate::env::Env;

/// Whether `rel` is markdown: scanned for question bullets rather than TODO comments, and
/// annotated as prose.
#[must_use]
pub(crate) fn is_markdown(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "mdx" | "markdown"))
}

/// How `rel` reads when a decision is written back into it: markdown ([`is_markdown`]) and
/// `.txt` are prose; anything else is code, and is never annotated.
pub(super) fn syntax_of(rel: &str) -> Syntax {
    let txt = Path::new(rel)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("txt"));
    if txt || is_markdown(rel) {
        Syntax::Prose
    } else {
        Syntax::Code
    }
}

/// Whether a path should be scanned, per config.
#[must_use]
pub(crate) fn scannable(env: &Env, rel: &str) -> bool {
    !env.config.scan.exclude.iter().any(|p| excludes(p, rel))
        && !Path::new(rel).components().any(|c| c.as_os_str() == ".git")
}

/// Whether the `scan.exclude` entry `pattern` covers the repo-relative path `rel`. Every entry
/// matches as a prefix from the root. A single-segment entry also matches at any depth, as
/// `.gitignore` treats a bare name (wi-e038ef): `node_modules/` any folder of that name, `gen`
/// any folder or file.
fn excludes(pattern: &str, rel: &str) -> bool {
    if rel.starts_with(pattern) {
        return true;
    }
    let (name, folders_only) = pattern
        .strip_suffix('/')
        .map_or((pattern, false), |name| (name, true));
    if name.is_empty() || name.contains('/') {
        return false;
    }
    let path = Path::new(rel);
    let within = if folders_only {
        path.parent()
    } else {
        Some(path)
    };
    within.is_some_and(|p| p.components().any(|c| c.as_os_str() == name))
}

#[cfg(test)]
mod tests {
    use super::{scannable, syntax_of};
    use crate::annotate::Syntax;
    use crate::ops::test_support::env_in;

    /// wi-e038ef: a single-segment entry matches at any depth, as `.gitignore` treats a bare name;
    /// an entry with more segments stays anchored at the root.
    #[test]
    fn a_bare_exclude_name_matches_at_any_depth() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut env = env_in(dir.path(), "s1");
        env.config.scan.exclude = vec!["node_modules/".into(), "gen".into(), "docs/out/".into()];

        for skipped in [
            "node_modules/x/README.md",
            "packages/app/node_modules/x/README.md",
            "gen/a.md",
            "lib/gen/a.md",
            "lib/gen",
            "docs/out/a.md",
            "a/.git/x.md",
        ] {
            assert!(!scannable(&env, skipped), "{skipped} should be skipped");
        }
        for kept in [
            "src/node_modules.md",
            "lib/generated/a.md",
            "lib/docs/out/a.md",
            "README.md",
        ] {
            assert!(scannable(&env, kept), "{kept} should be scanned");
        }
    }

    #[test]
    fn only_markdown_and_txt_files_read_as_prose() {
        let read: Vec<(&str, Syntax)> = [
            "notes.TXT",
            "spec.md",
            "docs/a.mdx",
            "README.markdown",
            "src/c.js",
            "src/a.rs",
            "doc/guide.rst",
            "Makefile",
        ]
        .into_iter()
        .map(|p| (p, syntax_of(p)))
        .collect();

        assert_eq!(
            read,
            [
                ("notes.TXT", Syntax::Prose),
                ("spec.md", Syntax::Prose),
                ("docs/a.mdx", Syntax::Prose),
                ("README.markdown", Syntax::Prose),
                ("src/c.js", Syntax::Code),
                ("src/a.rs", Syntax::Code),
                ("doc/guide.rst", Syntax::Code),
                ("Makefile", Syntax::Code),
            ]
        );
    }
}
