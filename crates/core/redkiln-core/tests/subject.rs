//! Subjects an approval, verdict or gate result binds to (SPEC ID-09, ID-10), on real
//! repositories. The `patch-id` cases rebuild `spec/fixtures/patch-id/check.sh`'s repository and
//! its hostile settings; the fixture's controls, showing each setting moves a plain diff's id,
//! stay in the script, which CI runs.
//!
//! A test binary of its own: these tests start many git processes, and under `cargo test` the
//! tests of one binary share a process. A child forked while another test's thread holds a
//! store's open lock (`<store>.lock`, a whole-file lock) holds that lock too until it execs, so
//! a telemetry write there, which never waits, would drop. nextest runs each test alone.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::Duration;

    use redkiln_core::location::GitError;
    use redkiln_core::subject::{
        Method, NoPatchId, Subject, SubjectError, git_commit, git_tree, patch_id,
    };

    const TIMEOUT: Duration = Duration::from_secs(30);

    /// The id `spec/fixtures/patch-id/expected.txt` records.
    fn expected() -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../spec/fixtures/patch-id/expected.txt");
        std::fs::read_to_string(path)
            .expect("the fixture's id")
            .trim()
            .to_owned()
    }

    /// git for setting a repository up: no system or global configuration, fixed identities and
    /// dates. The library's own calls never see this environment.
    fn git_in(dir: &Path, args: &[&str]) -> String {
        let home = dir.parent().unwrap_or(dir);
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", home.join("no-global-config"))
            .env("HOME", home)
            .env("GIT_AUTHOR_NAME", "fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
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

    fn write(dir: &Path, name: &str, bytes: &[u8]) {
        std::fs::write(dir.join(name), bytes).expect("write");
    }

    fn lines<'a>(items: impl IntoIterator<Item = &'a str>) -> Vec<u8> {
        items
            .into_iter()
            .flat_map(|l| format!("{l}\n").into_bytes())
            .collect()
    }

    /// The fixture's repository: `(dir, repo, root, base, head)`.
    struct Fixture {
        _dir: tempfile::TempDir,
        repo: PathBuf,
        root: String,
        base: String,
        head: String,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).expect("repo dir");
        let git = |args: &[&str]| git_in(&repo, args);
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "advice.graftFileDeprecated", "false"]);
        git(&["commit", "-q", "--allow-empty", "-m", "root"]);
        let root = git(&["rev-parse", "HEAD"]);
        std::fs::create_dir(repo.join("sub")).expect("sub");
        let cafe = "caf\u{e9}.txt";
        write(&repo, "blocks.src", b"{\n  one\n}\n\n{\n  three\n}\n");
        let numbered: Vec<String> = (1..=16).map(|n| format!("line {n}")).collect();
        write(
            &repo,
            "notes.txt",
            &lines(numbered.iter().map(String::as_str)),
        );
        write(
            &repo,
            "algo.txt",
            &lines(["a", "b", "c", "a", "b", "c", "X", "}", "}"]),
        );
        write(&repo, "sub/inner.txt", b"inner\n");
        let bulk: Vec<String> = (0..400)
            .map(|n| format!("bulk line {n} of the change"))
            .collect();
        write(&repo, "bulk.txt", &lines(bulk.iter().map(String::as_str)));
        write(&repo, "old-name.txt", b"moved content\none\ntwo\nthree\n");
        write(&repo, cafe, b"bonjour\n");
        write(&repo, "blob.bin", b"\x00\x01\x02");
        git(&["add", "-A"]);
        git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,1111111111111111111111111111111111111111,module",
        ]);
        git(&["commit", "-q", "-m", "base"]);
        let base = git(&["rev-parse", "HEAD"]);
        let head = change(&repo);
        Fixture {
            _dir: dir,
            repo,
            root,
            base,
            head,
        }
    }

    /// check.sh's change branch, a change sensitive to every input the recipe pins.
    fn change(repo: &Path) -> String {
        let git = |args: &[&str]| git_in(repo, args);
        let cafe = "caf\u{e9}.txt";
        let mut numbered: Vec<String> = (1..=16).map(|n| format!("line {n}")).collect();
        git(&["checkout", "-q", "-b", "change"]);
        write(
            repo,
            "blocks.src",
            b"{\n  one\n}\n\n{\n  two\n}\n\n{\n  three\n}\n",
        );
        numbered[1] = "line TWO".to_owned();
        numbered[10] = "line ELEVEN".to_owned();
        write(
            repo,
            "notes.txt",
            &lines(numbered.iter().map(String::as_str)),
        );
        write(
            repo,
            "algo.txt",
            &lines(["X", "a", "b", "c", "}", "Y", "a", "b", "}", "c"]),
        );
        write(repo, "sub/inner.txt", b"inner changed\n");
        let bulk: Vec<String> = (0..400)
            .map(|n| format!("bulk LINE {n} of the change"))
            .collect();
        write(repo, "bulk.txt", &lines(bulk.iter().map(String::as_str)));
        git(&["mv", "old-name.txt", "new-name.txt"]);
        write(repo, cafe, b"bonjour\nau revoir\n");
        write(repo, "blob.bin", b"\x00\x01\x03");
        write(repo, "white.yml", b"  trailing   \n\tindented\n");
        git(&["add", "-A"]);
        git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,2222222222222222222222222222222222222222,module",
        ]);
        git(&["commit", "-q", "-m", "change"]);
        git(&["rev-parse", "HEAD"])
    }

    fn id_in(dir: &Path, base: &str, head: &str) -> Result<String, SubjectError> {
        patch_id(dir, base, head, TIMEOUT).map(|id| {
            assert_eq!(id.base(), base, "recorded with its base");
            id.subject().hash().to_owned()
        })
    }

    /// check.sh's `-c` cases, as repository configuration, with the files they name in `work`.
    fn hostile_configuration(work: &Path) -> Vec<Vec<(&'static str, String)>> {
        write(work, "attr-nodiff", b"* -diff\n");
        write(work, "attr-textconv", b"notes.txt diff=upper\n");
        write(work, "attr-bin", b"algo.txt diff=bin\n");
        write(work, "order", b"white.yml\n*\n");
        let path = |name: &str| work.join(name).to_string_lossy().replace('\\', "/");
        vec![
            vec![("color.ui", "always".into())],
            vec![("diff.renames", "false".into())],
            vec![("diff.external", "false".into())],
            vec![("diff.noprefix", "true".into())],
            vec![("diff.context", "10".into())],
            vec![("diff.interHunkContext", "10".into())],
            vec![("diff.algorithm", "histogram".into())],
            vec![("diff.indentHeuristic", "false".into())],
            vec![("diff.suppressBlankEmpty", "true".into())],
            vec![("core.quotePath", "false".into())],
            vec![("diff.submodule", "log".into())],
            vec![("diff.ignoreSubmodules", "all".into())],
            vec![("core.attributesFile", path("attr-nodiff"))],
            vec![
                ("core.attributesFile", path("attr-textconv")),
                ("diff.upper.textconv", "tr a-z A-Z <".into()),
            ],
            vec![
                ("core.attributesFile", path("attr-bin")),
                ("diff.bin.binary", "true".into()),
            ],
            vec![("diff.orderFile", path("order"))],
            vec![("diff.mnemonicPrefix", "true".into())],
            vec![
                ("diff.srcPrefix", "x/".into()),
                ("diff.dstPrefix", "y/".into()),
            ],
            vec![("core.abbrev", "4".into())],
            vec![
                ("patchid.stable", "false".into()),
                ("patchid.verbatim", "false".into()),
            ],
        ]
    }

    #[test]
    fn the_patch_id_is_the_fixtures_under_any_configuration() {
        let f = fixture();
        let expected = expected();
        let id = patch_id(&f.repo, &f.base, &f.head, TIMEOUT).expect("an id");
        assert_eq!(id.subject().method(), Method::PatchId);
        assert_eq!(id.subject().as_str(), format!("patch-id:{expected}"));
        let work = f.repo.parent().expect("parent").to_path_buf();
        for case in hostile_configuration(&work) {
            for (key, value) in &case {
                git_in(&f.repo, &["config", key, value]);
            }
            assert_eq!(
                id_in(&f.repo, &f.base, &f.head).ok(),
                Some(expected.clone()),
                "{case:?}"
            );
            for (key, _) in &case {
                git_in(&f.repo, &["config", "--unset", key]);
            }
        }
        git_in(&f.repo, &["config", "diff.relative", "true"]);
        assert_eq!(
            id_in(&f.repo.join("sub"), &f.base, &f.head).ok(),
            Some(expected),
            "diff.relative"
        );
        git_in(&f.repo, &["config", "--unset", "diff.relative"]);
    }

    #[test]
    fn the_patch_id_ignores_attribute_files_replace_refs_and_grafts() {
        let f = fixture();
        let expected = expected();
        let info = f.repo.join(".git/info");
        std::fs::create_dir_all(&info).expect("info");
        write(&info, "attributes", b"*.txt -diff\n");
        assert_eq!(
            id_in(&f.repo, &f.base, &f.head).ok(),
            Some(expected.clone()),
            "info/attributes"
        );
        std::fs::remove_file(info.join("attributes")).expect("remove");
        write(&f.repo, ".gitattributes", b"*.src binary\n");
        assert_eq!(
            id_in(&f.repo, &f.base, &f.head).ok(),
            Some(expected.clone()),
            ".gitattributes"
        );
        std::fs::remove_file(f.repo.join(".gitattributes")).expect("remove");

        let tree = format!("{}^{{tree}}", f.base);
        let other = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.base, "-m", "other", &tree],
        );
        git_in(&f.repo, &["replace", &f.head, &other]);
        assert_eq!(
            id_in(&f.repo, &f.base, &f.head).ok(),
            Some(expected.clone()),
            "replace ref"
        );
        git_in(&f.repo, &["replace", "-d", &f.head]);
        write(
            &info,
            "grafts",
            format!("{} {}\n", f.head, f.root).as_bytes(),
        );
        assert_eq!(
            id_in(&f.repo, &f.base, &f.head).ok(),
            Some(expected),
            "info/grafts"
        );
    }

    fn no_id(result: Result<String, SubjectError>) -> NoPatchId {
        match result {
            Err(SubjectError::NoPatchId(why)) => why,
            other => panic!("expected no id, got {other:?}"),
        }
    }

    #[test]
    fn there_is_no_patch_id_for_an_empty_change_a_missing_object_or_a_criss_cross() {
        let f = fixture();
        assert!(
            matches!(no_id(id_in(&f.repo, &f.base, &f.base)), NoPatchId::Empty),
            "empty change"
        );

        let blob = git_in(&f.repo, &["rev-parse", &format!("{}:white.yml", f.head)]);
        let (fan, rest) = blob.split_at_checked(2).expect("an object id");
        let object = f.repo.join(".git/objects").join(fan).join(rest);
        let away = f.repo.parent().expect("parent").join("blob");
        std::fs::rename(&object, &away).expect("hide the object");
        assert!(
            matches!(
                no_id(id_in(&f.repo, &f.base, &f.head)),
                NoPatchId::DiffFailed { .. }
            ),
            "missing object"
        );
        std::fs::rename(&away, &object).expect("restore the object");

        let tree_base = git_in(&f.repo, &["rev-parse", &format!("{}^{{tree}}", f.base)]);
        let tree_head = git_in(&f.repo, &["rev-parse", &format!("{}^{{tree}}", f.head)]);
        let xa = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.base, "-m", "xa", &tree_base],
        );
        let xb = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.base, "-m", "xb", &tree_base],
        );
        let x1 = git_in(
            &f.repo,
            &["commit-tree", "-p", &xa, "-p", &xb, "-m", "x1", &tree_base],
        );
        let x2 = git_in(
            &f.repo,
            &["commit-tree", "-p", &xb, "-p", &xa, "-m", "x2", &tree_head],
        );
        assert!(
            matches!(
                no_id(id_in(&f.repo, &x1, &x2)),
                NoPatchId::MergeBases { found: 2 }
            ),
            "criss-cross"
        );
        let linear = git_in(
            &f.repo,
            &["commit-tree", "-p", &xa, "-m", "linear", &tree_head],
        );
        git_in(&f.repo, &["replace", &x2, &linear]);
        assert!(
            matches!(
                no_id(id_in(&f.repo, &x1, &x2)),
                NoPatchId::MergeBases { found: 2 }
            ),
            "criss-cross behind a replace ref"
        );
    }

    #[test]
    fn a_shallow_repository_has_no_patch_id() {
        let f = fixture();
        let tree_base = git_in(&f.repo, &["rev-parse", &format!("{}^{{tree}}", f.base)]);
        let tree_head = git_in(&f.repo, &["rev-parse", &format!("{}^{{tree}}", f.head)]);
        let tree_root = git_in(&f.repo, &["rev-parse", &format!("{}^{{tree}}", f.root)]);
        let b1 = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.base, "-m", "b1", &tree_base],
        );
        let h1 = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.base, "-m", "h1", &tree_head],
        );
        let side = git_in(
            &f.repo,
            &["commit-tree", "-p", &f.root, "-m", "side", &tree_root],
        );
        let merged = git_in(
            &f.repo,
            &[
                "commit-tree",
                "-p",
                &h1,
                "-p",
                &side,
                "-m",
                "merged",
                &tree_head,
            ],
        );
        assert_eq!(
            id_in(&f.repo, &b1, &merged).ok(),
            Some(expected()),
            "moved base, not shallow"
        );
        write(
            &f.repo.join(".git"),
            "shallow",
            format!("{h1}\n").as_bytes(),
        );
        assert!(matches!(
            no_id(id_in(&f.repo, &b1, &merged)),
            NoPatchId::Shallow
        ));
    }

    #[test]
    fn a_commit_subject_is_its_id() {
        let f = fixture();
        let subject = git_commit(&f.repo, "change", TIMEOUT).expect("a commit");
        assert_eq!(subject.method(), Method::GitCommit);
        assert_eq!(subject.as_str(), format!("git-commit:{}", f.head));
        let missing = git_commit(&f.repo, "no-such-branch", TIMEOUT);
        assert!(
            matches!(missing, Err(SubjectError::Git(GitError::Failed { .. }))),
            "{missing:?}"
        );
    }

    #[test]
    fn a_tree_subject_is_the_index_plus_tracked_edits_and_leaves_the_index_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).expect("repo");
        git_in(&repo, &["init", "-q"]);
        write(&repo, "a.txt", b"one\n");
        write(&repo, "b.txt", b"two\n");
        git_in(&repo, &["add", "-A"]);
        git_in(&repo, &["commit", "-q", "-m", "first"]);
        write(&repo, "a.txt", b"one, edited\n");
        write(&repo, "staged.txt", b"staged\n");
        git_in(&repo, &["add", "staged.txt"]);
        write(&repo, "untracked.txt", b"not in the tree\n");
        let index = std::fs::read(repo.join(".git/index")).expect("the index");
        let before = files_beside_objects(&repo.join(".git"));

        let subject = git_tree(&repo, TIMEOUT).expect("a tree");
        assert_eq!(subject.method(), Method::GitTree);
        assert_eq!(
            std::fs::read(repo.join(".git/index")).expect("the index"),
            index,
            "index untouched"
        );
        assert_eq!(
            files_beside_objects(&repo.join(".git")),
            before,
            "nothing in .git but objects"
        );

        git_in(&repo, &["add", "-u"]);
        let expected = git_in(&repo, &["write-tree"]);
        assert_eq!(subject.as_str(), format!("git-tree:{expected}"));
        let names = git_in(&repo, &["ls-tree", "--name-only", &expected]);
        assert!(!names.contains("untracked"), "{names}");
    }

    /// Every file under `git_dir` outside `objects/`, by its path there, sorted.
    fn files_beside_objects(git_dir: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut dirs = vec![git_dir.to_path_buf()];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).expect("list") {
                let path = entry.expect("entry").path();
                let relative = path.strip_prefix(git_dir).expect("inside").to_path_buf();
                if relative.starts_with("objects") {
                    continue;
                }
                if path.is_dir() {
                    dirs.push(path);
                } else {
                    found.push(relative);
                }
            }
        }
        found.sort();
        found
    }

    #[test]
    fn a_repository_with_no_index_yet_has_the_empty_tree_and_gains_no_index() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).expect("repo");
        git_in(&repo, &["init", "-q"]);
        write(&repo, "untracked.txt", b"not in the tree\n");
        assert!(!repo.join(".git/index").exists(), "no index yet");
        let before = files_beside_objects(&repo.join(".git"));

        let subject = git_tree(&repo, TIMEOUT).expect("a tree");

        let nothing = dir.path().join("nothing");
        std::fs::write(&nothing, b"").expect("an empty file");
        let nothing = nothing.to_str().expect("utf-8");
        let empty = git_in(&repo, &["hash-object", "-t", "tree", nothing]);
        assert_eq!(subject.as_str(), format!("git-tree:{empty}"));
        assert_eq!(
            files_beside_objects(&repo.join(".git")),
            before,
            "no index written, nothing else either"
        );
    }

    #[test]
    fn a_subject_reads_back_in_its_form() {
        let sha = Subject::sha256(b"approved plan\n");
        assert_eq!(sha.method(), Method::Sha256);
        assert_eq!(Subject::parse(sha.as_str()).expect("parses"), sha);
        for good in [
            "git-commit:bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382",
            "patch-id:93594b0161a663ec597ecfa4138132233329f99f",
            "git-tree:4b825dc642cb6eb9a060e54bf8d69288fbee4904a5a5a5a5a5a5a5a5a5a5a5a5",
            "rust-fingerprint:4be1",
        ] {
            assert_eq!(Subject::parse(good).expect(good).as_str(), good);
        }
        for bad in [
            "sha256:abc",
            "git-commit:BD234009C1E5A7D4F1C3B2A0E9F8D7C6B5A49382",
            "git-tree:4b825dc6",
            "patch-id:",
            "md5:d41d8cd98f00b204e9800998ecf8427e",
            "rust-fingerprint:",
            "no colon",
        ] {
            assert!(
                matches!(Subject::parse(bad), Err(SubjectError::Form)),
                "{bad}"
            );
        }
    }
}
