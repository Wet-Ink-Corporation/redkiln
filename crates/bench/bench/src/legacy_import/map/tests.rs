use std::path::Path;

use redkiln_core::envelope::Timestamp;
use redkiln_core::ids::{Host, WorktreeId, parse_store_id};

use super::{approval_store, timestamp, worktree};

#[test]
fn a_timestamp_core_takes_is_kept_as_written() {
    for text in ["2026-11-02T10:05:00Z", "2026-11-02T10:05:00.120Z"] {
        assert_eq!(
            timestamp(text).map(|ts| ts.as_str().to_owned()),
            Some(text.to_owned()),
            "{text}"
        );
    }
}

#[test]
fn a_dropped_trailing_zero_is_normalised_to_three_digits_at_the_same_instant() {
    let cases = [
        ("2026-11-02T10:04:00.12Z", "2026-11-02T10:04:00.120Z"),
        ("2026-11-02T10:12:00.5Z", "2026-11-02T10:12:00.500Z"),
        ("2026-11-02T10:12:00.123456Z", "2026-11-02T10:12:00.123Z"),
        ("2026-11-02t10:12:00.5z", "2026-11-02T10:12:00.500Z"),
    ];
    for (old, normalised) in cases {
        assert_eq!(
            timestamp(old).map(|ts| ts.as_str().to_owned()),
            Some(normalised.to_owned()),
            "{old}"
        );
    }
}

#[test]
fn an_offset_is_normalised_to_utc_through_its_unix_milliseconds() {
    let utc = Timestamp::parse("2026-11-02T08:04:00.000Z").expect("a timestamp");

    let cases = [
        "2026-11-02T10:04:00+02:00",
        "2026-11-02T05:34:00-02:30",
        "2026-11-02T10:04:00.0+02:00",
    ];
    for old in cases {
        assert_eq!(
            timestamp(old).map(|ts| ts.unix_millis()),
            Some(utc.unix_millis()),
            "{old}"
        );
    }
}

#[test]
fn text_that_is_no_rfc_3339_time_is_refused() {
    for text in [
        "",
        "yesterday",
        "2026-11-02",
        "2026-11-02T10:04:00",
        "2026-11-02T10:04:00.Z",
        "2026-11-02T10:04:00+2:00",
        "2026-11-02T10:04:00+24:00",
        "2026-13-02T10:04:00Z",
    ] {
        assert_eq!(timestamp(text), None, "{text:?}");
    }
}

#[test]
fn an_old_worktree_path_becomes_the_id_this_build_computes_and_an_id_stays() {
    let path = "/home/user/anneal-fixture";
    let id = WorktreeId::of_toplevel(Path::new(path), Host::current()).expect("an id");

    // Compared twice: from the path, then as itself.
    assert_eq!(worktree(path), Some(id.clone()), "a top-level path");
    assert_eq!(worktree(id.as_str()), Some(id), "an id already");
    assert_eq!(
        worktree("relative/path"),
        None,
        "Core refuses a relative path"
    );
}

#[test]
fn an_imported_approval_s_store_is_the_first_sixteen_bytes_of_its_derived_digest() {
    let old = parse_store_id("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f").expect("a store id");

    let derived = approval_store(old);

    // sha256("bench-approval:0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f"), its first 32 hex characters.
    assert_eq!(derived.to_string(), "5f39fb5b5061bf1a12b8fa159cb887d0");
    assert_ne!(derived, old);
}
