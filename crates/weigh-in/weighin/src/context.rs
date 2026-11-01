//! Context packs: the evidence a card needs, computed by script so the model reads a short
//! summary instead of files. Bounded by `caps.context_chars` (about 600 tokens).
//!
//! For a `path:line` anchor: a 3–15 line excerpt, the symbol's reference count and top call
//! sites, tests that mention it, one line of history, working-tree churn, matching `.kb`
//! atoms, a SHA-pinned permalink and an editor link. Without an anchor, only atom matches.

use serde::Serialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use weighin_core::Item;

use crate::env::{self, Env};
use crate::git::{self, LineSpan};
use crate::text::clip;

#[derive(Debug, Default, Serialize)]
pub(crate) struct Pack {
    pub(crate) item: String,
    pub(crate) anchor: Option<String>,
    pub(crate) excerpt: Option<(usize, Vec<String>)>,
    pub(crate) symbol: Option<String>,
    pub(crate) references: Option<usize>,
    pub(crate) call_sites: Vec<String>,
    pub(crate) tests: Vec<String>,
    pub(crate) history: Option<String>,
    pub(crate) churn: Option<String>,
    pub(crate) atoms: Vec<String>,
    pub(crate) permalink: Option<String>,
    pub(crate) editor: Option<String>,
    pub(crate) notes: Vec<String>,
}

/// An anchor's path and its line, when it gives one.
pub(crate) fn parse_anchor(at: &str) -> (&str, Option<usize>) {
    // path:line or path:line:col; Windows drive letters keep their colon.
    let (path, position) = env::split_anchor(at);
    let line = position
        .strip_prefix(':')
        .and_then(|p| p.split(':').next())
        .and_then(|l| l.parse().ok());
    (path, line)
}

const DEF_WORDS: &[&str] = &[
    "fn",
    "func",
    "function",
    "def",
    "class",
    "struct",
    "enum",
    "trait",
    "interface",
    "type",
    "record",
    "impl",
    "module",
    "namespace",
    "const",
    "static",
    "sub",
    "procedure",
];

/// The identifier an anchor line defines or most prominently uses.
fn symbol_on(line: &str) -> Option<String> {
    let words: Vec<&str> = line
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .collect();
    for (i, w) in words.iter().enumerate() {
        if DEF_WORDS.contains(&w.to_ascii_lowercase().as_str())
            && let Some(n) = words.get(i + 1)
            && n.chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            return Some((*n).to_owned());
        }
    }
    words
        .iter()
        .filter(|w| {
            w.len() >= 4
                && w.chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
        })
        .max_by_key(|w| w.len())
        .map(|w| (*w).to_owned())
}

fn has(cmd: &str) -> bool {
    Command::new(cmd)
        .stdin(Stdio::null())
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Whether ripgrep runs here, checked once per process: `open --with-context` builds a pack per
/// item, and each would otherwise spawn its own `rg --version`.
fn has_rg() -> bool {
    static RG: OnceLock<bool> = OnceLock::new();
    *RG.get_or_init(|| has("rg"))
}

/// `path:line` hits for a whole-word symbol, excluding generated trees.
fn references(root: &Path, sym: &str) -> Vec<String> {
    // An explicit `.` and a null stdin: ripgrep searches stdin, not the tree, whenever stdin is
    // not a terminal, which is always the case inside hooks and tool calls.
    let out = if has_rg() {
        Command::new("rg")
            .current_dir(root)
            .stdin(Stdio::null())
            .args([
                "-n",
                "-w",
                "--no-heading",
                "--color=never",
                "-g",
                "!target",
                "-g",
                "!node_modules",
                "-g",
                "!.git",
                "-g",
                "!.kb/_intake",
                "-F",
                "--",
                sym,
                ".",
            ])
            .output()
            .ok()
    } else {
        Command::new("git")
            .current_dir(root)
            .stdin(Stdio::null())
            .args(["grep", "-n", "-w", "-F", "--", sym])
            .output()
            .ok()
    };
    let Some(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim_start_matches("./").replace('\\', "/"))
        .collect()
}

fn is_test_path(p: &str) -> bool {
    let l = p.to_ascii_lowercase();
    l.contains("/test")
        || l.contains("tests/")
        || l.contains("_test.")
        || l.contains(".test.")
        || l.contains("spec")
        || l.starts_with("test")
}

/// Atom ids and titles in `.kb` (outside `_intake` and `_ledger`) that mention any needle.
fn matching_atoms(root: &Path, needles: &[&str]) -> Vec<String> {
    let kb = root.join(".kb");
    if !kb.is_dir() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut stack = vec![kb];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            // The entry's own type: a symlinked directory is not descended into, so a link back
            // up the tree cannot loop (H-13). An entry whose type cannot be read is skipped, as
            // an unreadable directory is.
            let Ok(kind) = e.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if !name.starts_with('_') {
                    stack.push(p);
                }
                continue;
            }
            if Path::new(&name).extension().is_none_or(|x| x != "md")
                || name.eq_ignore_ascii_case("readme.md")
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                continue;
            };
            if needles.iter().any(|n| !n.is_empty() && text.contains(n)) {
                let head = field_lines(&text);
                let field = |k: &str| {
                    head.lines().find_map(|l| {
                        l.strip_prefix(k)
                            .map(|v| v.trim().trim_matches('"').to_owned())
                    })
                };
                let id = field("id:").unwrap_or_else(|| name.clone());
                let status = field("status:").unwrap_or_default();
                let title = field("title:").unwrap_or_default();
                hits.push(format!("{id} [{status}] {}", clip(&title, 70)));
            }
        }
    }
    hits.sort();
    hits.truncate(4);
    hits
}

/// The lines an atom's `id:`, `status:` and `title:` are read from: its frontmatter block, between
/// the opening `---` and the closing one, so a body line such as `status: open` is never taken
/// for a field. A file that does not open with `---` has no frontmatter and is read whole; a
/// block that is never closed is read to the end of the file.
fn field_lines(text: &str) -> &str {
    let Some((first, rest)) = text.split_once('\n') else {
        return text;
    };
    if first.trim_end() != "---" {
        return text;
    }
    let mut end = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return rest.get(..end).unwrap_or(rest);
        }
        end = end.saturating_add(line.len());
    }
    rest
}

/// Build the pack for an item.
#[must_use]
pub(crate) fn build(env: &Env, item: &Item) -> Pack {
    let root = env.root();
    let mut p = Pack {
        item: item.id.to_string(),
        ..Default::default()
    };
    let Some(at) = item.at.clone().or_else(|| {
        item.source_path
            .as_ref()
            .map(|s| format!("{s}:{}", item.source_line.unwrap_or(1)))
    }) else {
        p.atoms = matching_atoms(&root, &[&item.question]);
        p.notes
            .push("no path:line anchor; evidence limited to .kb matches".into());
        return p;
    };
    let (path, line) = parse_anchor(&at);
    let rel = env.relative(path);
    p.anchor = Some(format!("{rel}:{}", line.unwrap_or(1)));
    let abs = root.join(&rel);
    let text = read_source(&mut p, &rel, &abs);
    let line = line.unwrap_or(1).max(1);
    let mut anchor_line = String::new();
    if let Some(t) = &text {
        let (start, body, at_line) = excerpt(t, line);
        anchor_line = at_line;
        p.excerpt = Some((start, body));
    }
    p.symbol = item.sym.clone().or_else(|| symbol_on(&anchor_line));
    add_references(&mut p, &root, &rel, line);
    add_history(&mut p, &root, &rel, line);
    add_churn(&mut p, &root, &rel);
    let sym = p.symbol.clone().unwrap_or_default();
    p.atoms = matching_atoms(&root, &[&rel, &sym]);
    if let Some(sha) = &env.head {
        let span = p.excerpt.as_ref().map_or(
            LineSpan {
                first: line,
                last: line,
            },
            |(s, l)| LineSpan {
                first: *s,
                last: s.saturating_add(l.len().saturating_sub(1)),
            },
        );
        p.permalink = git::permalink(&root, sha, &rel, span);
    }
    let abs_s = abs.to_string_lossy().replace('\\', "/");
    // vscode://file/<absolute path>: `/home/x` becomes `vscode://file/home/x`, `C:/r/a` becomes
    // `vscode://file/C:/r/a`.
    p.editor = Some(format!(
        "vscode://file/{}:{line}:1",
        abs_s.trim_start_matches('/')
    ));
    p
}

/// The anchored file's text; a file that cannot be read becomes a note on the pack.
fn read_source(p: &mut Pack, rel: &str, abs: &Path) -> Option<String> {
    match std::fs::read_to_string(abs) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            p.notes.push(format!("{rel} not found in this worktree"));
            None
        }
        Err(e) => {
            p.notes.push(format!("{rel} unreadable: {e}"));
            None
        }
    }
}

/// The excerpt window around `line` (first line number, clipped lines) and the anchored line.
fn excerpt(text: &str, line: usize) -> (usize, Vec<String>, String) {
    let lines: Vec<&str> = text.lines().collect();
    let start = line.saturating_sub(3).max(1);
    let end = line.saturating_add(7).min(lines.len()).max(start);
    let body = lines
        .get(start.saturating_sub(1)..end)
        .unwrap_or_default()
        .iter()
        .map(|l| clip(l, 140))
        .collect();
    let anchor = lines
        .get(line.saturating_sub(1))
        .copied()
        .unwrap_or_default()
        .to_owned();
    (start, body, anchor)
}

/// Reference count, test mentions and call sites of the pack's symbol.
fn add_references(p: &mut Pack, root: &Path, rel: &str, line: usize) {
    let Some(sym) = &p.symbol else {
        return;
    };
    let refs: Vec<String> = references(root, sym)
        .into_iter()
        .filter(|r| !r.starts_with(&format!("{rel}:{line}:")))
        .collect();
    p.references = Some(refs.len());
    p.tests = refs
        .iter()
        .filter(|r| is_test_path(r))
        .take(3)
        .map(|r| clip(r, 120))
        .collect();
    p.call_sites = refs
        .iter()
        .filter(|r| !is_test_path(r))
        .take(5)
        .map(|r| clip(r, 120))
        .collect();
}

/// One line of history for the lines around the anchor.
fn add_history(p: &mut Pack, root: &Path, rel: &str, line: usize) {
    let range = format!("-L{line},{}:{rel}", line.saturating_add(5));
    let args = [
        "log",
        "--no-patch",
        "--format=%h %ad %s",
        "--date=short",
        &range,
    ];
    if let Ok(log) = git::run(root, &args) {
        let commits: Vec<&str> = log.lines().filter(|l| !l.trim().is_empty()).collect();
        if let Some(last) = commits.first() {
            p.history = Some(format!(
                "{} commit(s) touch these lines; last {}",
                commits.len(),
                clip(last, 90)
            ));
        }
    }
}

/// Uncommitted added/removed line counts for the file.
fn add_churn(p: &mut Pack, root: &Path, rel: &str) {
    if let Ok(ns) = git::run(root, &["diff", "--numstat", "HEAD", "--", rel])
        && let Some(l) = ns.lines().next()
        && let [added, removed, ..] = l.split_whitespace().collect::<Vec<_>>().as_slice()
    {
        p.churn = Some(format!("uncommitted +{added} -{removed}"));
    }
}

/// A pack as compact text, uncapped; [`render`] caps it.
struct Text<'p>(&'p Pack);

impl std::fmt::Display for Text<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let p = self.0;
        write!(f, "## {} context", p.item)?;
        if let Some(a) = &p.anchor {
            write!(f, " · {a}")?;
        }
        writeln!(f)?;
        if let Some((start, lines)) = &p.excerpt {
            writeln!(f, "```")?;
            for (n, l) in (*start..).zip(lines) {
                writeln!(f, "{n:>4} {l}")?;
            }
            writeln!(f, "```")?;
        }
        if let Some(sym) = &p.symbol {
            writeln!(
                f,
                "symbol `{sym}`: {} other reference(s)",
                p.references.unwrap_or(0)
            )?;
        }
        for (k, v) in [("call sites", &p.call_sites), ("tests", &p.tests)] {
            if !v.is_empty() {
                writeln!(f, "{k}: {}", v.join(" | "))?;
            }
        }
        for (k, v) in [
            ("history", &p.history),
            ("churn", &p.churn),
            ("permalink", &p.permalink),
            ("editor", &p.editor),
        ] {
            if let Some(v) = v {
                writeln!(f, "{k}: {v}")?;
            }
        }
        if !p.atoms.is_empty() {
            writeln!(f, ".kb: {}", p.atoms.join(" | "))?;
        }
        for n in &p.notes {
            writeln!(f, "note: {n}")?;
        }
        Ok(())
    }
}

/// Compact text within `cap` characters.
#[must_use]
pub(crate) fn render(p: &Pack, cap: usize) -> String {
    let s = Text(p).to_string();
    if s.chars().count() > cap {
        s.chars().take(cap.saturating_sub(12)).collect::<String>() + "\n[truncated]"
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchors_parse() {
        assert_eq!(parse_anchor("src/a.rs:88"), ("src/a.rs", Some(88)));
        assert_eq!(parse_anchor("src/a.rs:88:4"), ("src/a.rs", Some(88)));
        assert_eq!(parse_anchor("C:/r/a.cs:12"), ("C:/r/a.cs", Some(12)));
        assert_eq!(parse_anchor("docs/plan.md"), ("docs/plan.md", None));
    }

    #[test]
    #[cfg(unix)]
    #[cfg_attr(miri, ignore = "makes directories and symlinks on the file system")]
    fn atoms_under_a_symlinked_directory_loop_are_matched_once() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let kb = root.path().join(".kb");
        std::fs::create_dir_all(&kb)?;
        std::fs::write(
            kb.join("a.md"),
            "id: kb-a\nstatus: accepted\ntitle: Ledger home\nThe ledger lives in git.\n",
        )?;
        // One link, so a walk that follows it fails fast (on duplicates, at the kernel's 40-link
        // limit) rather than hanging as two links make it.
        std::os::unix::fs::symlink(&kb, kb.join("loop"))?;

        let hits = matching_atoms(root.path(), &["ledger lives"]);

        assert_eq!(hits, ["kb-a [accepted] Ledger home"]);
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "makes directories on the file system")]
    fn atom_fields_are_read_from_the_frontmatter_not_the_body() -> std::io::Result<()> {
        let root = tempfile::tempdir()?;
        let kb = root.path().join(".kb/decisions");
        std::fs::create_dir_all(&kb)?;
        // No `status:` in the frontmatter; the body has lines that look like fields.
        std::fs::write(
            kb.join("retention.md"),
            "---\nid: \"kb-retention\"\ntitle: \"Retention per series\"\nkind: decision\n---\n\n\
             # Retention per series\n\nstatus: something\ntitle: Not the title\n\
             Anchor: `src/retention.rs:6`.\n",
        )?;

        let hits = matching_atoms(root.path(), &["src/retention.rs"]);

        assert_eq!(hits, ["kb-retention [] Retention per series"]);
        Ok(())
    }

    #[test]
    fn a_file_without_frontmatter_is_read_whole() {
        let text = "id: kb-a\nstatus: accepted\nBody.\n";

        assert_eq!(field_lines(text), text);
    }

    #[test]
    fn an_unclosed_frontmatter_block_is_read_to_the_end() {
        let text = "---\nid: kb-a\nstatus: accepted\nBody.\n";

        assert_eq!(field_lines(text), "id: kb-a\nstatus: accepted\nBody.\n");
    }

    #[test]
    fn an_anchor_past_any_line_gives_an_empty_excerpt() {
        let out = excerpt("a\nb\n", usize::MAX);

        assert_eq!(out, (usize::MAX - 3, Vec::new(), String::new()));
    }

    #[test]
    fn symbols() {
        assert_eq!(
            symbol_on("pub fn store_dir(common_dir: &Path) -> PathBuf {").as_deref(),
            Some("store_dir")
        );
        assert_eq!(
            symbol_on("public sealed class RetentionPolicy : IPolicy").as_deref(),
            Some("RetentionPolicy")
        );
        assert_eq!(
            symbol_on("    let overrides = policy.series_overrides();").as_deref(),
            Some("series_overrides")
        );
    }

    #[test]
    fn a_pack_renders_as_compact_text_within_its_cap() {
        let pack = Pack {
            item: "wi-000001".into(),
            anchor: Some("src/a.rs:9".into()),
            excerpt: Some((9, vec!["fn a() {}".into(), "fn b() {}".into()])),
            symbol: Some("a".into()),
            references: Some(2),
            call_sites: vec!["src/b.rs:3".into(), "src/c.rs:4".into()],
            history: Some("abc1234 add a".into()),
            atoms: vec!["kb-0001".into()],
            notes: vec!["one".into()],
            ..Pack::default()
        };
        let text = "## wi-000001 context · src/a.rs:9\n```\n   9 fn a() {}\n  10 fn b() {}\n```\nsymbol `a`: 2 other reference(s)\ncall sites: src/b.rs:3 | src/c.rs:4\nhistory: abc1234 add a\n.kb: kb-0001\nnote: one\n";

        assert_eq!(render(&pack, 1000), text);
        assert_eq!(
            render(&pack, 20),
            "## wi-00\n[truncated]",
            "cut to cap - 12 characters, then marked"
        );
    }
}
