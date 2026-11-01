# Plugin API types for the mod's type check

`claude-code.d.ts` is Claude Code's declaration of the plugin API (`'claude-code'` and
`'claude-code/testing'`), copied unedited from the `plugin-authoring` skill of Claude Code
2.1.303 (its first line names the version). `tests/mod.tsconfig.json` type-checks
`plugin-mod/hooks/mod/` and `plugin-mod/types/` against it, as temper's lens does (wi-78d987).
Refresh it by hand when the pinned Claude Code moves.
