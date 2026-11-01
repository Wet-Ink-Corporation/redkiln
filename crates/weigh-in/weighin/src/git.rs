//! Git through the command line. Reads and the one write-shaped operation (a tree hash on a
//! temporary index) both shell out; nothing here commits, moves refs or runs hooks.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::{Command, Stdio};

/// Run `git -C dir args…` and return stdout.
///
/// # Errors
/// Fails when git cannot start or exits non-zero.
pub(crate) fn run(dir: &Path, args: &[&str]) -> Result<String> {
    run_env(dir, args, &[])
}

/// # Errors
/// Fails when git cannot start or exits non-zero.
pub(crate) fn run_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let out = output_env(dir, args, env)?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}

/// Run `git -C dir args…` and return stdout as bytes, for content that need not be UTF-8 (a
/// file at a revision).
///
/// # Errors
/// Fails when git cannot start or exits non-zero.
pub(crate) fn output(dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    output_env(dir, args, &[])
}

fn output_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<Vec<u8>> {
    let mut cmd = Command::new("git");
    cmd.stdin(Stdio::null())
        .arg("-C")
        .arg(dir)
        .args(["-c", "core.quotepath=off"])
        .args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
}

/// The tree the human is looking at: the current index plus tracked modifications, untracked
/// files excluded, written on a temporary index in the temporary directory. Same derivation as
/// redkiln-rs ADR-0105 clause 4, so a hash recorded here compares with one recorded there.
#[must_use]
pub(crate) fn tree_hash(root: &Path) -> Option<String> {
    let index = run(
        root,
        &["rev-parse", "--path-format=absolute", "--git-path", "index"],
    )
    .ok()?;
    let index = index.trim();
    // A temporary index on the local temporary disk, not beside the ledger: git replaces it by
    // rename and it is removed afterwards, and Cowork's connected folders refuse deletes.
    let tmp_dir = tempfile::tempdir().ok()?;
    let tmp = tmp_dir.path().join("index");
    if Path::new(index).exists() {
        std::fs::copy(index, &tmp).ok()?;
    }
    let tmp_s = tmp.to_string_lossy().into_owned();
    let env = [("GIT_INDEX_FILE", tmp_s.as_str())];
    let result = (|| {
        if !Path::new(index).exists() {
            run_env(root, &["read-tree", "--empty"], &env).ok()?;
        }
        run_env(root, &["add", "-u"], &env).ok()?;
        run_env(root, &["write-tree"], &env)
            .ok()
            .map(|s| s.trim().to_owned())
    })();
    drop(tmp_dir);
    result
}

/// `origin`'s URL turned into a browsable permalink for `path` lines `a..=b` at `sha`.
/// GitHub and Azure DevOps are supported; anything else returns `None`.
#[must_use]
pub(crate) fn permalink(root: &Path, sha: &str, path: &str, span: LineSpan) -> Option<String> {
    let url = run(root, &["remote", "get-url", "origin"]).ok()?;
    web_url(url.trim(), sha, path, span)
}

/// The first and last line (1-based, inclusive) a permalink points at.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineSpan {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

#[must_use]
pub(crate) fn web_url(remote: &str, sha: &str, path: &str, span: LineSpan) -> Option<String> {
    let LineSpan { first: a, last: b } = span;
    let r = remote.trim_end_matches('/').trim_end_matches(".git");
    // git@github.com:owner/repo  |  https://github.com/owner/repo
    if let Some(rest) = r
        .strip_prefix("git@github.com:")
        .or_else(|| r.strip_prefix("https://github.com/"))
        .or_else(|| r.strip_prefix("ssh://git@github.com/"))
    {
        let rest = rest.split('@').next_back().unwrap_or(rest);
        return Some(format!(
            "https://github.com/{rest}/blob/{sha}/{path}#L{a}-L{b}"
        ));
    }
    // https://dev.azure.com/org/project/_git/repo  |  git@ssh.dev.azure.com:v3/org/project/repo
    let ado = if let Some(rest) = r.strip_prefix("git@ssh.dev.azure.com:v3/") {
        match rest.split('/').collect::<Vec<&str>>().as_slice() {
            [org, project, repo] => {
                Some(format!("https://dev.azure.com/{org}/{project}/_git/{repo}"))
            }
            _ => None,
        }
    } else if r.contains("dev.azure.com/") && r.contains("/_git/") {
        let (_, tail) = r.split_once("dev.azure.com/")?;
        Some(format!("https://dev.azure.com/{tail}"))
    } else {
        None
    };
    // Azure DevOps highlights a line only when the full parameter set is present.
    ado.map(|base| format!("{base}?path=/{path}&version=GC{sha}&line={a}&lineEnd={b}&lineStartColumn=1&lineEndColumn=1&lineStyle=plain"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_and_azure_permalinks() {
        assert_eq!(
            web_url(
                "git@github.com:Wet-Ink-Corporation/redkiln-rs.git",
                "abc",
                "crates/x.rs",
                LineSpan { first: 3, last: 9 }
            )
            .as_deref(),
            Some("https://github.com/Wet-Ink-Corporation/redkiln-rs/blob/abc/crates/x.rs#L3-L9")
        );
        let ado = web_url(
            "https://acme@dev.azure.com/acme/Contoso/_git/migrate",
            "abc",
            "src/a.cs",
            LineSpan { first: 5, last: 5 },
        )
        .unwrap();
        assert!(ado.starts_with("https://dev.azure.com/acme/Contoso/_git/migrate?path=/src/a.cs&version=GCabc&line=5&lineEnd=5"), "{ado}");
        assert!(ado.ends_with("lineStyle=plain"));
        assert!(
            web_url(
                "https://gitlab.com/x/y",
                "abc",
                "a",
                LineSpan { first: 1, last: 1 }
            )
            .is_none()
        );
    }
}
