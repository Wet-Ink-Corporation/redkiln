use std::fs;

use super::{BenchBinaryError, LauncherTarget, bench_file, install, locate_bench};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn without_a_flag_the_bench_beside_skillcheck_is_used() -> TestResult {
    let dir = tempfile::tempdir()?;
    let skillcheck = dir.path().join("skillcheck");
    let bench = dir.path().join(bench_file());
    fs::write(&bench, "bench")?;

    let found = locate_bench(None, &skillcheck)?;

    assert_eq!(found, bench);
    Ok(())
}

#[test]
fn the_flag_overrides_the_bench_beside_skillcheck() -> TestResult {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join(bench_file()), "beside")?;
    let given = dir.path().join("elsewhere-bench");
    fs::write(&given, "given")?;

    let found = locate_bench(Some(&given), &dir.path().join("skillcheck"))?;

    assert_eq!(found, given);
    Ok(())
}

#[test]
fn no_bench_beside_skillcheck_and_no_flag_is_refused_naming_where_it_looked() -> TestResult {
    let dir = tempfile::tempdir()?;

    let refused = locate_bench(None, &dir.path().join("skillcheck"));

    let looked = dir.path().join(bench_file());
    assert!(
        matches!(&refused, Err(BenchBinaryError::NotBeside { looked: at }) if *at == looked),
        "{refused:?}"
    );
    Ok(())
}

#[test]
fn a_flag_naming_no_file_is_refused_even_with_a_bench_beside_skillcheck() -> TestResult {
    let dir = tempfile::tempdir()?;
    fs::write(dir.path().join(bench_file()), "beside")?;
    let given = dir.path().join("no-such-bench");

    let refused = locate_bench(Some(&given), &dir.path().join("skillcheck"));

    assert!(
        matches!(&refused, Err(BenchBinaryError::NotAFile { given: at }) if *at == given),
        "{refused:?}"
    );
    Ok(())
}

#[test]
fn a_directory_beside_skillcheck_named_bench_is_not_a_binary() -> TestResult {
    let dir = tempfile::tempdir()?;
    fs::create_dir(dir.path().join(bench_file()))?;

    let refused = locate_bench(None, &dir.path().join("skillcheck"));

    assert!(
        matches!(refused, Err(BenchBinaryError::NotBeside { .. })),
        "{refused:?}"
    );
    Ok(())
}

#[test]
fn each_host_the_launcher_knows_maps_to_the_target_it_tries_first() {
    let hosts = [
        ("linux", "x86_64"),
        ("linux", "aarch64"),
        ("macos", "aarch64"),
        ("macos", "x86_64"),
        ("windows", "x86_64"),
    ];

    let placed: Vec<Option<String>> = hosts
        .iter()
        .map(|(os, arch)| {
            LauncherTarget::of(os, arch).map(|target| {
                crate::path_text::slashed(
                    &target.relative().to_string_lossy(),
                    std::path::MAIN_SEPARATOR,
                )
                .into_owned()
            })
        })
        .collect();

    assert_eq!(
        placed,
        [
            "libexec/x86_64-unknown-linux-musl/bench",
            "libexec/aarch64-unknown-linux-musl/bench",
            "libexec/aarch64-apple-darwin/bench",
            "libexec/x86_64-apple-darwin/bench",
            "libexec/x86_64-pc-windows-msvc/bench.exe",
        ]
        .map(|path| Some(path.to_owned()))
    );
}

#[test]
fn a_host_the_launcher_does_not_know_has_no_target() {
    let unknown = [
        ("freebsd", "x86_64"),
        ("linux", "riscv64"),
        ("windows", "aarch64"),
        ("macos", "powerpc"),
    ];

    assert_eq!(
        unknown.map(|(os, arch)| LauncherTarget::of(os, arch)),
        [None; 4]
    );
}

#[test]
fn install_copies_the_binary_where_the_launcher_looks() -> TestResult {
    let source = tempfile::tempdir()?;
    let binary = source.path().join("anneal");
    fs::write(&binary, "binary")?;
    let copy = tempfile::tempdir()?;
    let target = LauncherTarget::of("linux", "x86_64").ok_or("a target")?;

    install(&binary, copy.path(), target)?;

    assert_eq!(
        fs::read_to_string(copy.path().join("libexec/x86_64-unknown-linux-musl/bench"))?,
        "binary"
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn the_installed_binary_is_executable_and_replaces_a_read_only_one() -> TestResult {
    use std::os::unix::fs::PermissionsExt;

    let source = tempfile::tempdir()?;
    let binary = source.path().join("anneal");
    fs::write(&binary, "new")?;
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o600))?;
    let copy = tempfile::tempdir()?;
    let placed = copy.path().join("libexec/aarch64-apple-darwin/bench");
    fs::create_dir_all(placed.parent().ok_or("a parent")?)?;
    fs::write(&placed, "old")?;
    fs::set_permissions(&placed, fs::Permissions::from_mode(0o444))?;
    let target = LauncherTarget::of("macos", "aarch64").ok_or("a target")?;

    install(&binary, copy.path(), target)?;

    assert_eq!(
        (
            fs::read_to_string(&placed)?,
            fs::metadata(&placed)?.permissions().mode() & 0o777
        ),
        ("new".to_owned(), 0o755)
    );
    Ok(())
}
