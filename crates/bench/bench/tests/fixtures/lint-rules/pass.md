# Rules: a passing rule document

Each rule names the hook, lint or test that enforces it (`bench lint rules`).

## Rules

- Never merge a pull request without a fresh review verdict. Enforcer: `hook merge-gate`
- Every MUST in a skill names its enforcer. Enforcer: `bench lint skills plugin/skills`
- A rule with no enforcer is reported, never passed.
  Enforcer: `test crates/bench/bench/tests/it/lint_rules.rs::a_rule_with_no_enforcer_is_an_error`
