//! Integration tests for `redkiln-core` on real git repositories: journals (SPEC JR-01 to JR-14)
//! and the store's location (ST-01 to ST-03).
//!
//! A test binary of its own: these tests start many git processes, and under `cargo test` the
//! tests of one binary share a process. A child forked while another test's thread holds a
//! store's open lock (`<store>.lock`, a whole-file lock) holds that lock too until it execs, so
//! a write there that never waits, as telemetry's, would drop. Core keeps its own spawns from
//! that window; a test's own `Command` is outside it. nextest runs each test alone.

#[cfg(test)]
mod journal;
#[cfg(test)]
mod location;
#[cfg(test)]
mod support;
