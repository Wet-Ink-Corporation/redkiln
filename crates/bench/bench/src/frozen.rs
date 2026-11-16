//! The freeze markers on `rollout`, `perf` and `migrate` (Move 2 step 5; blueprint, Bench
//! "Freezes"): each module's source opens with [`MARKER`], so removing a freeze shows in CI.

/// The first doc line of each frozen module.
const MARKER: &str = "//! Frozen (Move 2 step 5; blueprint, Bench \"Freezes\"): security and correctness fixes only until Ship exists.";

/// Each frozen module's name and source.
const FROZEN: [(&str, &str); 3] = [
    ("rollout", include_str!("rollout.rs")),
    ("perf", include_str!("perf.rs")),
    ("migrate", include_str!("migrate.rs")),
];

#[test]
fn each_frozen_module_opens_with_the_freeze_marker() {
    for (name, source) in FROZEN {
        assert_eq!(source.lines().next(), Some(MARKER), "src/{name}.rs");
    }
}
