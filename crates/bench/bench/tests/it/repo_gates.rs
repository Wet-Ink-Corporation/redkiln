//! Repository gates on Bench's own paths: the packaged plugin holds no evals, and no debug
//! leftover is committed. The Tier 0, 1 and 3 skill gates moved to `tools/skillcheck`'s
//! `tests/it/repo_gates.rs` with `lint skills` and `eval` (Move 2 step 5, PR 5.1b).

use std::error::Error;
use std::path::{Path, PathBuf};

use bench::repro::debug_leftovers;

type TestResult = Result<(), Box<dyn Error>>;

/// Where the monorepo keeps what was anneal's repository (Move 2 placement).
const BENCH_PATHS: [&str; 6] = [
    "crates/bench",
    "plugins/bench",
    "plugins/bench-cockpit",
    "docs/bench",
    "evals/bench",
    "tools/bench",
];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Every directory under `dir`, recursively, without following symlinks.
fn directories(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let path = entry.path();
            found.extend(directories(&path)?);
            found.push(path);
        }
    }
    Ok(found)
}

#[test]
fn the_packaged_plugin_holds_no_evals_directory() -> TestResult {
    let evals: Vec<PathBuf> = directories(&repo().join("plugins/bench"))?
        .into_iter()
        .filter(|dir| dir.ends_with("evals"))
        .collect();

    assert_eq!(
        evals,
        Vec::<PathBuf>::new(),
        "evals/ stays outside the packaged plugin"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "runs git in a subprocess")]
fn the_repository_holds_no_debug_leftover() -> TestResult {
    // Bench's own paths in the monorepo, which together hold what anneal's repository held.
    for dir in BENCH_PATHS {
        let scan = debug_leftovers(&repo().join(dir))?;

        let found: Vec<String> = scan
            .leftovers()
            .iter()
            .map(|leftover| format!("{dir}/{}:{}", leftover.file(), leftover.line()))
            .collect();
        assert!(
            scan.is_clean(),
            "the Stop hook would block every session in this repository; build a test's tag \
             at run time, as `format!(\"[{{}}-x1]\", \"DEBUG\")`: {found:?}"
        );
        assert!(
            scan.skipped().is_empty() && !scan.over_budget(),
            "the leftover scan skipped files it could not read whole, so a tag there goes unseen: {:?}",
            scan.skipped()
        );
    }
    Ok(())
}
