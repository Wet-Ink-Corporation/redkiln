//! Whether a test run regenerates the files it checks: `BENCH_BLESS` set, or anneal's
//! `ANNEAL_BLESS`, still read for the 0.x line (M2-B). Test-only.

/// Whether `BENCH_BLESS` or, as a fallback, `ANNEAL_BLESS` is set.
pub(crate) fn blessing() -> bool {
    std::env::var_os("BENCH_BLESS").is_some() || std::env::var_os("ANNEAL_BLESS").is_some()
}
