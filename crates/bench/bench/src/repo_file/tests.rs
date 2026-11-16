use std::fs;
use std::path::Path;

use super::{CONFIG, Chosen, Notice, RATCHETS, STANDING_ORDERS, notices, warn};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn write(root: &Path, relative: &str) -> TestResult {
    let path = super::under(root, relative);
    fs::create_dir_all(path.parent().ok_or("a parent")?)?;
    fs::write(path, "{}")?;
    Ok(())
}

#[test]
fn each_file_is_in_bench_s_folder_with_anneal_s_beside_it() {
    let paths = [CONFIG, RATCHETS, STANDING_ORDERS].map(|file| (file.path, file.legacy));

    assert_eq!(
        paths,
        [
            (".redkiln/bench/config.json", ".anneal/config.json"),
            (".redkiln/bench/ratchets.json", ".anneal/ratchets.json"),
            (
                ".redkiln/bench/standing-orders.md",
                ".anneal/standing-orders.md"
            ),
        ]
    );
}

#[test]
fn with_neither_file_the_new_one_is_chosen_with_no_notice() -> TestResult {
    let root = tempfile::tempdir()?;

    let chosen = CONFIG.choose(root.path());

    assert_eq!(
        chosen,
        Chosen {
            path: CONFIG.path,
            notice: None
        }
    );
    Ok(())
}

#[test]
fn the_old_file_alone_is_read_as_a_fallback_with_a_notice() -> TestResult {
    let root = tempfile::tempdir()?;
    write(root.path(), CONFIG.legacy)?;

    let chosen = CONFIG.choose(root.path());

    assert_eq!(
        chosen,
        Chosen {
            path: CONFIG.legacy,
            notice: Some(Notice::Fallback(CONFIG))
        }
    );
    Ok(())
}

#[test]
fn with_both_the_new_file_is_read_and_the_old_one_is_named_as_ignored() -> TestResult {
    let root = tempfile::tempdir()?;
    write(root.path(), CONFIG.legacy)?;
    write(root.path(), CONFIG.path)?;

    let chosen = CONFIG.choose(root.path());

    assert_eq!(
        chosen,
        Chosen {
            path: CONFIG.path,
            notice: Some(Notice::Ignored(CONFIG))
        }
    );
    Ok(())
}

#[test]
fn a_directory_in_the_new_place_is_chosen_so_its_reader_refuses_it() -> TestResult {
    let root = tempfile::tempdir()?;
    fs::create_dir_all(super::under(root.path(), CONFIG.path))?;
    write(root.path(), CONFIG.legacy)?;

    assert_eq!(CONFIG.choose(root.path()).path, CONFIG.path);
    Ok(())
}

#[test]
fn the_notices_say_which_file_is_read_and_where_to_move_it() {
    let fallback = Notice::Fallback(RATCHETS).to_string();
    let ignored = Notice::Ignored(RATCHETS).to_string();

    assert_eq!(
        (fallback.as_str(), ignored.as_str()),
        (
            ".anneal/ratchets.json is read because .redkiln/bench/ratchets.json does not exist; \
             bench reads anneal's .anneal/ only as a 0.x fallback, so move the file there",
            ".anneal/ratchets.json is ignored: bench reads .redkiln/bench/ratchets.json"
        )
    );
}

#[test]
fn warn_writes_one_line_per_old_file_and_nothing_without_one() -> TestResult {
    let root = tempfile::tempdir()?;
    let mut quiet = Vec::new();
    warn(root.path(), &[CONFIG, RATCHETS], &mut quiet)?;
    write(root.path(), CONFIG.legacy)?;
    write(root.path(), RATCHETS.legacy)?;
    write(root.path(), RATCHETS.path)?;

    let mut told = Vec::new();
    warn(root.path(), &[CONFIG, RATCHETS], &mut told)?;

    assert_eq!(
        (String::from_utf8(quiet)?, String::from_utf8(told)?),
        (
            String::new(),
            format!(
                "bench: warning: {}\nbench: warning: {}\n",
                Notice::Fallback(CONFIG),
                Notice::Ignored(RATCHETS)
            )
        )
    );
    assert_eq!(
        notices(root.path(), &[STANDING_ORDERS]),
        Vec::<Notice>::new()
    );
    Ok(())
}

#[test]
fn choosing_never_writes_or_removes_either_folder() -> TestResult {
    let root = tempfile::tempdir()?;
    write(root.path(), CONFIG.legacy)?;
    let old = super::under(root.path(), CONFIG.legacy);

    let chosen = CONFIG.choose(root.path());
    let notices = notices(root.path(), &[CONFIG, RATCHETS, STANDING_ORDERS]);

    assert_eq!(
        (
            chosen.path,
            notices.len(),
            fs::read_to_string(&old)?,
            root.path().join(".redkiln").exists()
        ),
        (CONFIG.legacy, 1, "{}".to_owned(), false)
    );
    Ok(())
}
