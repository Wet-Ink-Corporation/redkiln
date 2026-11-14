//! What another crate may name of the `rk-bench` plugin: its name, entry skills and hook ids.
//!
//! The name is the install id Claude Code namespaces the plugin's skills and slash commands
//! under (`rk-bench:build`, `/rk-bench:handoff`).
//!
//! bench's `plugin_name` tests pin [`NAME`] against `plugins/bench/.claude-plugin/plugin.json`
//! and [`HOOK_IDS`] against the hooks `bench hook` runs; its bootstrap index and route playbook
//! tests pin [`ENTRY_SKILLS`]. So each changes with Bench or a test fails.

/// The plugin's name, as its manifest declares it.
pub const NAME: &str = "rk-bench";

/// The plugin's model-invoked entry skills, in the bootstrap index's order (blueprint
/// "Packaging against the listing budget"). Every other skill of the plugin sets
/// `disable-model-invocation: true`.
pub const ENTRY_SKILLS: [&str; 7] = [
    "grill", "spec", "plan", "build", "diagnose", "verify", "ship",
];

/// The id of every hook the plugin runs (`bench hook <id>`), in Bench's own order.
pub const HOOK_IDS: &[&str] = &[
    "bootstrap",
    "guard",
    "preconditions",
    "fast-check",
    "completeness",
    "status",
    "route",
    "ruled-before",
    "merge-gate",
    "injection-scan",
    "context-monitor",
    "handoff",
    "brief",
    "standing-orders",
];

/// The name a skill of this plugin carries in Claude Code (`rk-bench:build`), stripped to the
/// skill's own (`build`); `None` for a bare name or another plugin's skill.
#[must_use]
pub fn skill_of(qualified: &str) -> Option<&str> {
    qualified
        .strip_prefix(NAME)
        .and_then(|rest| rest.strip_prefix(':'))
}

#[cfg(test)]
mod tests {
    use super::skill_of;

    #[test]
    fn only_this_plugins_qualified_skill_names_are_stripped() {
        assert_eq!(skill_of("rk-bench:build"), Some("build"));
        assert_eq!(skill_of("rk-bench:"), Some(""));
        assert_eq!(skill_of("rk-benchbuild"), None);
        assert_eq!(skill_of("build"), None);
        assert_eq!(skill_of("anneal:build"), None);
        assert_eq!(skill_of("rk-weigh-in:build"), None);
    }
}
