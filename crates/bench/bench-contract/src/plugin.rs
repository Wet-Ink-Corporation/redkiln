//! What another crate may name of the `rk-bench` plugin: its name, the install id Claude Code
//! namespaces its skills and slash commands under (`rk-bench:build`, `/rk-bench:handoff`).
//!
//! bench's `plugin_name` test pins [`NAME`] against `plugins/bench/.claude-plugin/plugin.json`,
//! so it changes with the manifest or that test fails.

/// The plugin's name, as its manifest declares it.
pub const NAME: &str = "rk-bench";

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
