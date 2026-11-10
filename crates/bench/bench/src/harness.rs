//! The harness adapters: one module per harness bench runs under, and the only place a
//! harness's protocol is read or written (SPEC CLI-13; root `AGENTS.md`, CLI first, harness
//! second). The hooks read their payloads through the adapter's types and write their outputs
//! through its writers; `cargo xtask plugins` fails on the protocol's tokens anywhere else in the
//! crate.
//!
//! Today there is one harness, Claude Code.

pub(crate) mod claude_code;
