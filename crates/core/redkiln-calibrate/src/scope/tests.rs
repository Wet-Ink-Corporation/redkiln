use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{DecisionDirs, ScopeError, UserScope};

/// A lookup over `vars` only: no real environment (TST-10).
fn lookup(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let vars: Vec<(String, OsString)> = vars
        .iter()
        .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
        .collect();
    // Each call hands out its own value, as `std::env::var_os` does.
    move |var: &str| {
        vars.iter()
            .find(|(name, _)| name == var)
            .map(|(_, value)| value.clone())
    }
}

/// An absolute path on this host (`C:\..` on Windows), so the scope accepts it.
fn absolute(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn text(path: &Path) -> &str {
    path.to_str().expect("the temporary directory is UTF-8")
}

#[test]
fn redkiln_home_wins_over_home() {
    let (over, home) = (absolute("over"), absolute("me"));

    let scope = UserScope::from_lookup(lookup(&[
        ("REDKILN_HOME", text(&over)),
        ("HOME", text(&home)),
    ]))
    .expect("resolves");

    assert_eq!(scope.root(), over);
}

#[test]
fn without_redkiln_home_the_root_is_home_then_userprofile_dot_redkiln() {
    let (home, profile) = (absolute("me"), absolute("profile"));

    let unix = UserScope::from_lookup(lookup(&[
        ("REDKILN_HOME", ""),
        ("HOME", text(&home)),
        ("USERPROFILE", text(&profile)),
    ]))
    .expect("resolves");
    let windows =
        UserScope::from_lookup(lookup(&[("USERPROFILE", text(&profile))])).expect("resolves");

    assert_eq!(
        (unix.root(), windows.root()),
        (
            home.join(".redkiln").as_path(),
            profile.join(".redkiln").as_path()
        )
    );
}

#[test]
fn no_home_at_all_is_refused_even_with_anneal_home_set() {
    let anneal = absolute("anneal");

    let result = UserScope::from_lookup(lookup(&[("ANNEAL_HOME", text(&anneal))]));

    assert!(matches!(result, Err(ScopeError::NoHome)), "{result:?}");
}

#[test]
fn a_relative_home_is_refused_naming_its_variable() {
    let home = absolute("me");

    let redkiln = UserScope::from_lookup(lookup(&[("REDKILN_HOME", "rel/dir")]));
    let home_var = UserScope::from_lookup(lookup(&[("HOME", "rel")]));
    let anneal = UserScope::from_lookup(lookup(&[
        ("HOME", text(&home)),
        ("ANNEAL_HOME", "rel/anneal"),
    ]));

    assert!(
        matches!(
            &redkiln,
            Err(ScopeError::RelativeHome { var: "REDKILN_HOME", value }) if value == Path::new("rel/dir")
        ),
        "{redkiln:?}"
    );
    assert!(
        matches!(
            &home_var,
            Err(ScopeError::RelativeHome { var: "HOME", value })
                if value == &Path::new("rel").join(".redkiln")
        ),
        "{home_var:?}"
    );
    assert!(
        matches!(
            &anneal,
            Err(ScopeError::RelativeHome {
                var: "ANNEAL_HOME",
                ..
            })
        ),
        "{anneal:?}"
    );
}

#[test]
fn a_producers_rows_live_under_calibrate_and_the_anneal_log_is_read_alongside() {
    let (over, anneal) = (absolute("over"), absolute("anneal"));

    let dirs = UserScope::from_lookup(lookup(&[
        ("REDKILN_HOME", text(&over)),
        ("ANNEAL_HOME", text(&anneal)),
    ]))
    .expect("resolves")
    .decisions("bench")
    .expect("a plain producer name");

    assert_eq!(
        dirs,
        DecisionDirs {
            current: over.join("calibrate").join("bench"),
            legacy: Some(anneal.join("decisions")),
        }
    );
}

#[test]
fn without_anneal_home_the_old_log_is_home_dot_anneal_decisions() {
    let home = absolute("me");

    let dirs = UserScope::from_lookup(lookup(&[("HOME", text(&home))]))
        .expect("resolves")
        .decisions("bench")
        .expect("a plain producer name");

    assert_eq!(
        dirs,
        DecisionDirs {
            current: home.join(".redkiln").join("calibrate").join("bench"),
            legacy: Some(home.join(".anneal").join("decisions")),
        }
    );
}

#[test]
fn an_old_log_that_is_the_new_folder_is_not_read_twice() {
    let root = absolute("same");
    let anneal = root.join("calibrate");
    let dirs = UserScope::from_lookup(lookup(&[
        ("REDKILN_HOME", text(&root)),
        ("ANNEAL_HOME", text(&anneal)),
    ]))
    .expect("resolves")
    .decisions("decisions")
    .expect("a plain producer name");

    assert_eq!(dirs.legacy, None, "{dirs:?}");
}

#[test]
fn a_producer_that_is_not_one_plain_name_is_refused() {
    let scope =
        UserScope::from_lookup(lookup(&[("HOME", text(&absolute("me")))])).expect("resolves");

    for name in ["", "..", ".", "a/b", "a\\b", "Bench", "bench ", "-x"] {
        assert!(
            matches!(scope.decisions(name), Err(ScopeError::Producer { .. })),
            "{name:?}"
        );
    }
    assert_eq!(
        scope.decisions("weigh-in2").map(|dirs| dirs.current),
        Ok(absolute("me")
            .join(".redkiln")
            .join("calibrate")
            .join("weigh-in2"))
    );
}
