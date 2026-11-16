use std::ffi::OsString;
use std::path::{Path, PathBuf};

use redkiln_calibrate::scope::ScopeError;

use super::{BenchHome, StateHome};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A lookup over `vars` only: no real environment (TST-10).
fn lookup(vars: &[(&'static str, PathBuf)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let vars: Vec<(&'static str, OsString)> = vars
        .iter()
        .map(|(name, value)| (*name, value.clone().into_os_string()))
        .collect();
    move |var: &str| {
        vars.iter()
            .find(|(name, _)| *name == var)
            .map(|(_, value)| value.clone())
    }
}

#[test]
fn bench_s_state_is_the_user_scope_s_bench_folder_with_anneal_s_home_read_below_it() -> TestResult {
    let dir = tempfile::tempdir()?;
    let (root, anneal) = (dir.path().join("root"), dir.path().join("anneal"));

    let home = BenchHome::from_lookup(lookup(&[
        ("REDKILN_HOME", root.clone()),
        ("ANNEAL_HOME", anneal.clone()),
    ]))?;

    assert_eq!(
        (home.current(), home.legacy()),
        (root.join("bench").as_path(), Some(anneal.as_path()))
    );
    Ok(())
}

#[test]
fn without_any_home_variable_bench_s_state_has_no_place() {
    let result = BenchHome::from_lookup(|_: &str| None);

    assert!(matches!(result, Err(ScopeError::NoHome)), "{result:?}");
}

#[test]
fn an_anneal_home_that_is_bench_s_folder_is_not_read_twice() -> TestResult {
    let dir = tempfile::tempdir()?;
    let root = dir.path().join("root");

    let home = BenchHome::from_lookup(lookup(&[
        ("REDKILN_HOME", root.clone()),
        ("ANNEAL_HOME", root.join("bench")),
    ]))?;

    assert_eq!(home.legacy(), None, "{home:?}");
    Ok(())
}

/// A home with both folders under a fresh temporary directory.
fn both(dir: &Path) -> Result<BenchHome, ScopeError> {
    BenchHome::from_lookup(lookup(&[
        ("REDKILN_HOME", dir.join("root")),
        ("ANNEAL_HOME", dir.join("anneal")),
    ]))
}

#[test]
fn a_file_is_read_from_bench_s_folder_else_anneal_s_and_always_written_to_bench_s() -> TestResult {
    let dir = tempfile::tempdir()?;
    let home = both(dir.path())?;
    let (current, legacy) = (dir.path().join("root/bench"), dir.path().join("anneal"));
    std::fs::create_dir_all(&current)?;
    std::fs::create_dir_all(&legacy)?;
    std::fs::write(current.join("both.json"), "{}")?;
    std::fs::write(legacy.join("both.json"), "{}")?;
    std::fs::write(legacy.join("old.json"), "{}")?;

    let read = ["both.json", "old.json", "none.json"].map(|name| home.read(name));
    let written = ["both.json", "old.json", "none.json"].map(|name| home.written(name));

    assert_eq!(
        (read, written),
        (
            [
                current.join("both.json"),
                legacy.join("old.json"),
                current.join("none.json")
            ],
            [
                current.join("both.json"),
                current.join("old.json"),
                current.join("none.json")
            ]
        )
    );
    Ok(())
}

#[test]
fn an_old_file_that_cannot_be_read_is_read_as_absent() -> TestResult {
    let dir = tempfile::tempdir()?;
    let home = both(dir.path())?;
    std::fs::create_dir_all(dir.path().join("anneal/usage.json"))?;

    let read = home.read("usage.json");

    assert_eq!(read, dir.path().join("root/bench/usage.json"));
    Ok(())
}

#[test]
fn a_plain_folder_reads_and_writes_in_itself() {
    let folder = Path::new("state");

    assert_eq!(
        (folder.read("a.json"), folder.written("a.json")),
        (folder.join("a.json"), folder.join("a.json"))
    );
}
