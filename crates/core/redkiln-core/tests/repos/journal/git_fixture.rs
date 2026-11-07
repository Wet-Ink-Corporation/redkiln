//! Repositories for the journal ref tests: a bare remote with `main`, and clones of it.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Runs git in `dir`, which must succeed; its stdout, trimmed.
pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// Whether `git <args>` succeeds in `dir`.
pub(crate) fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs")
        .status
        .success()
}

/// A bare remote whose `main` has one commit, under `dir/remote.git`.
pub(crate) fn remote(dir: &Path) -> PathBuf {
    let remote = dir.join("remote.git");
    std::fs::create_dir_all(&remote).expect("dir");
    git(&remote, &["init", "-q", "--bare", "-b", "main"]);
    let seed = dir.join("seed");
    std::fs::create_dir_all(&seed).expect("dir");
    git(&seed, &["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("README.md"), "seed\n").expect("write");
    git(&seed, &["add", "README.md"]);
    git(&seed, &["commit", "-q", "-m", "seed"]);
    let url = remote.to_string_lossy().into_owned();
    git(&seed, &["push", "-q", &url, "main"]);
    remote
}

/// A clone of `remote` at `dir/<name>`, its remote `origin`.
pub(crate) fn clone(dir: &Path, remote: &Path, name: &str) -> PathBuf {
    let url = remote.to_string_lossy().into_owned();
    git(dir, &["clone", "-q", &url, name]);
    dir.join(name)
}

/// The file `path` at `rev` in the bare `remote`, if there is one.
pub(crate) fn file_at(remote: &Path, rev: &str, path: &str) -> Option<String> {
    let spec = format!("{rev}:{path}");
    git_ok(remote, &["cat-file", "-e", &spec]).then(|| git(remote, &["show", &spec]) + "\n")
}

/// The refs of `remote` under `prefix`, sorted.
pub(crate) fn refs(remote: &Path, prefix: &str) -> Vec<String> {
    let listed = git(remote, &["for-each-ref", "--format=%(refname)", prefix]);
    let mut names: Vec<String> = listed.lines().map(str::to_owned).collect();
    names.sort();
    names
}

/// Makes `remote` refuse any push outside `refs/heads/*`, as Claude Code cloud's git proxy does.
#[cfg(unix)]
pub(crate) fn refuse_custom_refs(remote: &Path) {
    use std::os::unix::fs::PermissionsExt as _;
    let hook = remote.join("hooks/pre-receive");
    let script = "#!/bin/sh\nwhile read old new ref; do\n  case \"$ref\" in\n    refs/heads/*) ;;\n    *) echo \"push contains a ref outside refs/heads/*\" >&2; exit 1 ;;\n  esac\ndone\n";
    std::fs::write(&hook, script).expect("hook");
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Lets `remote` take any push again.
#[cfg(unix)]
pub(crate) fn accept_custom_refs(remote: &Path) {
    std::fs::remove_file(remote.join("hooks/pre-receive")).expect("hook removed");
}
