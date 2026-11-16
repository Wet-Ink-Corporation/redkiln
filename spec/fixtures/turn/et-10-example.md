The retention fix is in; one call below waits on the owner.

```text
Status: DONE_WITH_CONCERNS
Gate: fmt ok | clippy ok | test ok (42 passed) | doc ok
Review: approved
Changed: crates/retention/src/policy.rs (+40/-2)
Decisions needed: retention per series or per record (the block below)
Declined: none
Not verified: the Windows leg
```

```weigh-in
kind: assumption
door: one-way
authority: authority
q: Should retention overrides attach per record or per series?
did: per-series, matching RetentionPolicy
flip: a tenant needs per-record legal hold
at: crates/retention/src/policy.rs:88
blocks: none
```
