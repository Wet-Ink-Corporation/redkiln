//! The Claude Code harness adapter: every environment variable Claude Code sets for the
//! plugin's hooks, commands and status line, by name, with what each one means, and the hook
//! payloads and outputs, read and written here only.
//!
//! This is the one seam between the CLI and the harness that runs it (CLI-13): the rest of
//! the crate asks this module for a variable's name and reads it through its injected
//! `lookup`, and never spells a `CLAUDE_*` string itself (a test in this module scans the
//! sources for one). anneal's own variables (`ANNEAL_*`, `WEIGHIN_BIN`) and third-party ones
//! (`TMPDIR`, `SSL_CERT_FILE`, the proxy variables) stay with the code that reads them.
//!
//! Plugin options: Claude Code (checked in 2.1.296) gives a hook each `userConfig` value,
//! sensitive ones included, as `CLAUDE_PLUGIN_OPTION_<KEY>`, the key upper-cased with every
//! character outside `[A-Za-z0-9_]` turned into `_`.
//!
//! Payloads: each hook reads the fields it uses through one of the types below, the rest
//! ignored, and its tool-specific part (`tool_input`) through a type of its own. Outputs: the
//! hooks write `hookSpecificOutput`, `systemMessage`, `decision` and `reason` only through the
//! writers here and [`StopOutput`].

mod stop_output;

use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) use stop_output::StopOutput;

/// `userConfig.cf_account_id`: the Cloudflare account that runs Clef; empty means Clef is off.
pub const ACCOUNT_VAR: &str = "CLAUDE_PLUGIN_OPTION_CF_ACCOUNT_ID";

/// `userConfig.cf_api_token` (sensitive): the Workers AI token.
pub const TOKEN_VAR: &str = "CLAUDE_PLUGIN_OPTION_CF_API_TOKEN";

/// The `userConfig` key holding the Clef mode, which [`MODE_VAR`] carries.
pub(crate) const MODE_KEY: &str = "clef_mode";

/// `userConfig.clef_mode`: `off`, `shadow` or `enforce`, with per-point overrides.
pub const MODE_VAR: &str = "CLAUDE_PLUGIN_OPTION_CLEF_MODE";

/// The auto-compact window in tokens, which Claude Code clamps to 100,000..=1,000,000.
pub(crate) const AUTO_COMPACT_WINDOW_VAR: &str = "CLAUDE_CODE_AUTO_COMPACT_WINDOW";

/// Given to a `SessionStart` hook: a shell script Claude Code sources before each later Bash
/// command.
pub(crate) const ENV_FILE_VAR: &str = "CLAUDE_ENV_FILE";

/// The plugin's root directory, the one holding `libexec/` and `skills/`.
pub(crate) const PLUGIN_ROOT_VAR: &str = "CLAUDE_PLUGIN_ROOT";

/// The project's root directory, the one Claude Code was started in.
pub(crate) const PROJECT_DIR_VAR: &str = "CLAUDE_PROJECT_DIR";

/// The payload key of a tool call's input, which denials name.
pub(crate) const TOOL_INPUT: &str = "tool_input";

/// A `PreToolUse` payload: the fields the guard, the merge gate and the preconditions hook use,
/// with the tool's input read as `T`; the rest are ignored.
#[derive(Debug, Deserialize)]
pub(crate) struct ToolUse<T> {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// Where the ledger (and the worktree) are found; the process's working directory when
    /// absent.
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    /// The tool called.
    pub(crate) tool_name: String,
    /// The call's input (`tool_input`).
    #[serde(default, rename = "tool_input")]
    pub(crate) tool: T,
}

/// A `PreToolUse` payload as the brief hook reads it: [`ToolUse`]'s fields and the subagent the
/// call runs in; the rest are ignored.
#[derive(Debug, Deserialize)]
pub(crate) struct SubagentToolUse<T> {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// The session's working directory.
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    /// The tool called.
    pub(crate) tool_name: String,
    /// The call's input (`tool_input`).
    #[serde(default, rename = "tool_input")]
    pub(crate) tool: T,
    /// Set when the call runs inside a subagent.
    #[serde(default)]
    pub(crate) agent_id: Option<String>,
    /// The subagent's type, when the call runs inside one.
    #[serde(default)]
    pub(crate) agent_type: Option<String>,
}

/// The fields of Claude Code's `Stop` input the completeness hook uses; the rest are ignored.
#[derive(Debug, Deserialize)]
pub(crate) struct StopInput {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// The session's transcript (`transcript_path`).
    #[serde(default, rename = "transcript_path")]
    pub(crate) transcript: Option<PathBuf>,
    /// The session's working directory.
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    /// Set when the stop already follows a block (`stop_hook_active`).
    #[serde(default, rename = "stop_hook_active")]
    pub(crate) after_block: bool,
    /// The reply that ends the turn (`last_assistant_message`).
    #[serde(default, rename = "last_assistant_message")]
    pub(crate) last_message: Option<String>,
}

/// The fields of Claude Code's `SubagentStop` input the status hook uses; the rest are ignored.
#[derive(Debug, Deserialize)]
pub(crate) struct SubagentStopInput {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// The session's working directory.
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    /// The subagent's id.
    #[serde(default)]
    pub(crate) agent_id: Option<String>,
    /// The subagent's type.
    #[serde(default)]
    pub(crate) agent_type: Option<String>,
    /// The subagent's own transcript.
    #[serde(default)]
    pub(crate) agent_transcript_path: Option<PathBuf>,
    /// The subagent's last reply (`last_assistant_message`).
    #[serde(default, rename = "last_assistant_message")]
    pub(crate) last_message: Option<String>,
}

/// The fields of Claude Code's `UserPromptSubmit` input the hooks use; the rest are ignored.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct PromptInput {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// The session's transcript (`transcript_path`).
    #[serde(default, rename = "transcript_path")]
    pub(crate) transcript: Option<PathBuf>,
    /// The session's working directory.
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    /// The prompt as submitted.
    #[serde(default)]
    pub(crate) prompt: Option<String>,
}

/// The fields of the `PostToolBatch` (or `PostToolUse`) input the context monitor uses.
#[derive(Debug, Deserialize)]
pub(crate) struct MonitorInput {
    /// The session's id.
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    /// The event the input is for (`hook_event_name`).
    #[serde(default, rename = "hook_event_name")]
    pub(crate) event: Option<String>,
}

/// The fields of a `PostToolBatch` or `PostToolUse` input the injection scan uses; the rest
/// are ignored.
#[derive(Debug, Deserialize)]
pub(crate) struct ScanInput {
    /// The event the input is for (`hook_event_name`).
    #[serde(default, rename = "hook_event_name")]
    pub(crate) event: Option<String>,
    /// A `PostToolBatch` input's calls.
    #[serde(default)]
    pub(crate) tool_calls: Option<Vec<ToolCall>>,
    /// A `PostToolUse` input's one call, at the top level.
    #[serde(flatten)]
    pub(crate) call: ToolCall,
}

/// One tool call. `tool_input` and `tool_response` stay raw: their shapes depend on the tool.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct ToolCall {
    /// The tool called (`tool_name`).
    #[serde(default, rename = "tool_name")]
    pub(crate) name: String,
    /// The call's input (`tool_input`).
    #[serde(default, rename = "tool_input")]
    pub(crate) input: Value,
    /// The call's result (`tool_response`).
    #[serde(default, rename = "tool_response")]
    pub(crate) response: Value,
}

/// One call of a `PostToolBatch` batch as the fast check reads it. `tool_input` stays raw: its
/// shape depends on the tool.
#[derive(Debug, Deserialize)]
pub(crate) struct BatchCall {
    /// The tool called.
    #[serde(default)]
    pub(crate) tool_name: String,
    /// The call's input (`tool_input`).
    #[serde(default, rename = "tool_input")]
    pub(crate) input: Value,
}

/// `{"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": "deny", ...}}`
/// (or `"ask"`).
#[derive(Debug, Serialize)]
struct DenyOutput<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: PreToolUseOutput<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreToolUseOutput<'a> {
    hook_event_name: &'static str,
    permission_decision: PermissionDecision,
    permission_decision_reason: &'a str,
}

/// The `permissionDecision` of a `PreToolUse` output the hooks write.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PermissionDecision {
    /// Block the call.
    Deny,
    /// Have the human confirm the call.
    Ask,
}

/// Writes a `PreToolUse` permission `decision` with `line`, folded to one line, as its reason.
///
/// # Errors
/// Returns the I/O error when `out` cannot be written.
pub(crate) fn write_permission<W: Write>(
    mut out: W,
    decision: PermissionDecision,
    line: &str,
) -> io::Result<()> {
    let line = line.replace(['\r', '\n'], " ");
    let output = DenyOutput {
        specific: PreToolUseOutput {
            hook_event_name: "PreToolUse",
            permission_decision: decision,
            permission_decision_reason: &line,
        },
    };
    serde_json::to_writer(&mut out, &output)?;
    writeln!(out)
}

/// `{"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": "..."}}`.
#[derive(Debug, Serialize)]
struct SessionStartJson<'a> {
    #[serde(rename = "hookSpecificOutput")]
    specific: SessionStartOutput<'a>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionStartOutput<'a> {
    hook_event_name: &'static str,
    additional_context: &'a str,
}

/// Writes a `SessionStart` output carrying `context` for the model, then a newline.
///
/// # Errors
/// Returns the I/O error when `out` cannot be written.
pub(crate) fn write_session_start<W: Write>(mut out: W, context: &str) -> io::Result<()> {
    let output = SessionStartJson {
        specific: SessionStartOutput {
            hook_event_name: "SessionStart",
            additional_context: context,
        },
    };
    serde_json::to_writer(&mut out, &output)?;
    writeln!(out)
}

/// `{"decision": "block", "reason": ..., "hookSpecificOutput": {...}}`, without the parts that
/// are empty.
#[derive(Debug, Serialize)]
struct BatchJson<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    decision: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    #[serde(rename = "hookSpecificOutput", skip_serializing_if = "Option::is_none")]
    specific: Option<BatchSpecific>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchSpecific {
    hook_event_name: &'static str,
    additional_context: String,
}

/// Writes a `PostToolBatch` output: a block with `reason` when there is one, and `context` for
/// the model when there is some, then a newline.
///
/// # Errors
/// Returns the I/O error when `out` cannot be written.
pub(crate) fn write_batch<W: Write>(
    mut out: W,
    reason: Option<&str>,
    context: Option<String>,
) -> io::Result<()> {
    let specific = context.map(|context| BatchSpecific {
        hook_event_name: "PostToolBatch",
        additional_context: context,
    });
    let output = BatchJson {
        decision: reason.map(|_| "block"),
        reason,
        specific,
    };
    serde_json::to_writer(&mut out, &output)?;
    writeln!(out)
}

#[derive(Debug, Serialize)]
struct PromptJson<'a> {
    #[serde(rename = "systemMessage", skip_serializing_if = "Option::is_none")]
    system_message: Option<&'a str>,
    #[serde(rename = "hookSpecificOutput", skip_serializing_if = "Option::is_none")]
    specific: Option<PromptSpecific<'a>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PromptSpecific<'a> {
    hook_event_name: &'static str,
    additional_context: &'a str,
}

/// Writes a `UserPromptSubmit` output: `error` shown to the person and `context` for the model,
/// each when present, as one JSON object and a newline.
///
/// # Errors
/// Returns the I/O error when `out` cannot be written.
pub(crate) fn write_prompt<W: Write>(
    mut out: W,
    error: Option<&str>,
    context: Option<&str>,
) -> io::Result<()> {
    let json = PromptJson {
        system_message: error,
        specific: context.map(|line| PromptSpecific {
            hook_event_name: "UserPromptSubmit",
            additional_context: line,
        }),
    };
    serde_json::to_writer(&mut out, &json)?;
    writeln!(out)
}

/// The protocol's keys, for tests that write a payload or read an output.
#[cfg(test)]
pub(crate) mod keys {
    pub(crate) use super::TOOL_INPUT;

    /// Payload: the hook event's name.
    pub(crate) const HOOK_EVENT_NAME: &str = "hook_event_name";
    /// Payload: a tool call's result.
    pub(crate) const TOOL_RESPONSE: &str = "tool_response";
    /// Payload: the session's transcript.
    pub(crate) const TRANSCRIPT_PATH: &str = "transcript_path";
    /// Payload: a stop that follows a block.
    pub(crate) const STOP_HOOK_ACTIVE: &str = "stop_hook_active";
    /// Payload: the reply that ends the turn.
    pub(crate) const LAST_ASSISTANT_MESSAGE: &str = "last_assistant_message";
    /// Output: the event's own part.
    pub(crate) const HOOK_SPECIFIC_OUTPUT: &str = "hookSpecificOutput";
    /// Output: the event the specific part answers.
    pub(crate) const HOOK_EVENT_NAME_OUT: &str = "hookEventName";
    /// Output: context for the model.
    pub(crate) const ADDITIONAL_CONTEXT: &str = "additionalContext";
    /// Output: the line shown to the person.
    pub(crate) const SYSTEM_MESSAGE: &str = "systemMessage";
    /// Output: a `PreToolUse` decision.
    pub(crate) const PERMISSION_DECISION: &str = "permissionDecision";
}

#[cfg(test)]
mod tests;
