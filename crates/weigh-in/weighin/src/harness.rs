//! The harness adapters: one module per harness weighin's hooks run under, and the only place a
//! harness's protocol is read or written (SPEC CLI-13; root `AGENTS.md`, CLI first, harness
//! second). An adapter reads the harness's payload into `hooks::HookInput`, runs the
//! harness-neutral hook, and writes its `hooks::HookOutput` back in the harness's terms; the hook
//! logic and every command take plain values. `cargo xtask plugins` fails on the protocol's
//! tokens anywhere else in the crate.
//!
//! Today there is one harness, Claude Code (Cowork's cloud workspace included).

pub(crate) mod claude_code;
