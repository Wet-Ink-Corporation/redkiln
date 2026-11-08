//! Fixtures the op families' tests share: a ledger outside git, a batch runner and a card.

use anyhow::Result;
use std::path::Path;
use weighin_core::id::ItemId;

use super::{Session, parse_ops};
use crate::env::Env;

pub(super) const ONE_WAY: &str = "Should retention attach per record or per series?";
pub(super) const TWO_WAY: &str = "Name the helper apply or run?";

/// An environment outside git: no tree hash, the folder is the root, the store beside it.
pub(super) fn env_in(dir: &Path, session: &str) -> Env {
    Env::outside_git(dir, session)
}

/// Apply a JSON batch and return one line per op, as `weighin batch` prints them.
pub(super) fn run(s: &mut Session<'_>, batch: &serde_json::Value) -> Result<Vec<String>> {
    Ok(parse_ops(&batch.to_string())?
        .into_iter()
        .map(|op| match s.apply_parsed(op) {
            Ok(r) => format!("✓ {r}"),
            Err(e) => format!("✗ {e:#}"),
        })
        .collect())
}

pub(super) fn card() -> serde_json::Value {
    serde_json::json!({
        "why": "It fixes what a tenant can hold.",
        "timing": "Decide now.",
        "options": [
            {"id": "A", "title": "Per series"},
            {"id": "B", "title": "Per record"},
            {"id": "C", "title": "Both", "fails_must": "one schema"}
        ],
        "recommendation": {"option": "A"}
    })
}

pub(super) fn id_of(q: &str) -> ItemId {
    ItemId::mint("s1", q)
}

/// The key of the hashed atom id of `kind` for `id`, a 10-hex item (Knowledge call C): what
/// follows `kb-<kind>-` in the id and starts the atom's file name.
pub(super) fn atom_key(kind: knowledge_contract::Kind, id: &ItemId) -> String {
    knowledge_contract::AtomId::hashed(kind, id.as_str(), knowledge_contract::HashLen::Ten)
        .key()
        .to_owned()
}

/// Run `setup`, then `ops`, in one batch; return only the lines `ops` printed, failing when a
/// setup op was refused.
pub(super) fn after(setup: &[serde_json::Value], ops: &[serde_json::Value]) -> Result<Vec<String>> {
    let dir = tempfile::tempdir()?;
    let env = env_in(dir.path(), "s1");
    let batch: Vec<_> = setup.iter().chain(ops).cloned().collect();
    let mut out = run(
        &mut Session::open(&env, "cli")?,
        &serde_json::Value::Array(batch),
    )?;
    let tail = out.split_off(setup.len().min(out.len()));
    if let Some(refused) = out.iter().find(|line| !line.starts_with('✓')) {
        anyhow::bail!("setup refused: {refused}");
    }
    Ok(tail)
}

/// [`ONE_WAY`], raised as a one-way door bound to authority.
pub(super) fn raised() -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({"op": "raise", "q": ONE_WAY, "door": "one-way", "authority": "authority"}),
    ]
}

/// [`ONE_WAY`], raised and grounded on [`card`].
pub(super) fn grounded() -> Vec<serde_json::Value> {
    let mut ops = raised();
    ops.push(serde_json::json!({"op": "ground", "id": "@1", "card": card()}));
    ops
}
