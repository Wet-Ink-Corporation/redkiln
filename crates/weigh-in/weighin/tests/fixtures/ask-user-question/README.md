# AskUserQuestion hook payloads (#125 PR 0)

These are live `PreToolUse` and `PostToolUse` payloads for `AskUserQuestion`. Ryan captured them on 2026-11-21 with Claude Code 2.1.325 on Windows, using a scratch folder whose `.claude/settings.local.json` logged every payload on `PreToolUse`, `PostToolUse` and `PostToolUseFailure` for that tool. They are the evidence decision D1 asked for before the witnessed write (#125 PR 1b) merges.

Redaction: `session_id`, `prompt_id`, `transcript_path`, `cwd` and `scratchpad_dir` are replaced with placeholders. Everything else is byte-for-byte what the hook received, re-indented. That includes `tool_use_id`, `permission_mode`, `effort` and `duration_ms`.

| File | Case | What the person did |
|---|---|---|
| `answered.{pre,post}.json` | Answered | Picked one option of a single-select question |
| `other-text.{pre,post}.json` | Other | Chose Other and typed text |
| `multi-select.{pre,post}.json` | Multi-select | Picked two of four options |
| `esc-cancelled.pre.json` | Idle timeout (attempted) | Left the dialog for 16 minutes (it never timed out), then pressed Esc |
| `esc-instead-of-follow-up.pre.json` | Follow-up (attempted) | Pressed Esc instead of choosing |
| `prefill-matching.{pre,post}.json` | Model-prefilled `answers` | The model prefilled option 2, and the person also picked option 2 |
| `prefill-overridden.{pre,post}.json` | Model-prefilled `answers` | The model prefilled option 2 ("Rotate at 10 MB"), and the person picked option 1 ("No limit") |

## What they show

- **The answer.** `tool_response` is `{questions, answers, annotations}`. `answers` maps each question's text to the chosen label.
- **Multi-select.** The answer is one string with the labels joined by `", "`.
- **Other.** The typed text is the answer itself, with no marker saying it came from Other. The text only stands out because it matches no option label.
- **Esc.** Esc ends the dialog with no `PostToolUse` and no `PostToolUseFailure` payload. The dialog never timed out in this build, and no `afkTimeoutMs`, `followUp` or free-text `response` field appeared in any payload.
- **Prefill.** On `PostToolUse`, `tool_input` already carries the harness's `answers` and equals `tool_response`. Only the `PreToolUse` payload holds the model's own prefill.
- **D1 passes.** A model-prefilled `tool_input.answers` does not answer the dialog. The dialog still appears with nothing selected, and the person's pick replaces the prefill (`prefill-overridden`).
