//! Integration tests of the guard kit through its public API only, compiled as one test
//! binary (TST-02): small rule packs of their own driven through `engine::check`, and a
//! protected-path table of their own.

mod named;
mod pack;
mod protect;
mod script_file;
mod seam;
