//! Integration tests for the `skillcheck` binary and library, compiled as one test binary
//! (TST-02).

mod bootstrap_index;
mod eval_routing;
mod eval_routing_real;
mod eval_uplift;
mod lint_skills;
mod lint_skills_inputs;
mod repo_gates;
#[cfg(test)]
mod support;
