//! `board export` and `board apply`: the exchange with the Artifact board and the local pane.

use anyhow::{Context as _, Result, anyhow};
use std::io::{self, Write};
use weighin_core::Ledger;
use weighin_core::board::{self, Answer, Scope, ValidAnswer};

use super::Receipts;
use crate::env::{self, BoardMode, BoardUse, Env};
use crate::ops::{self, DecideOp, DeferOp, Op, Session};
use crate::{AnswerSurface, ApplyArgs, ExportArgs, pane};

pub(crate) fn board_export(env: &Env, a: ExportArgs, out: &mut impl Write) -> Result<()> {
    if a.stdout {
        return pane::export(env, board_scope(env), &env::now(), out);
    }
    // The default state file sits beside the ledger; writing it would create the ledger's
    // directory where no item was ever raised. Only a stored ledger has items to keep off
    // claude.ai, so only its export asks whether this folder may publish.
    let ledger = match (ops::stored_ledger(env)?, &a.out) {
        (Some(ledger), _) => {
            publishable(env)?;
            ledger
        }
        (None, Some(_)) => Ledger::default(),
        (None, None) => {
            writeln!(
                out,
                "no ledger at {} yet: nothing to export",
                env.store_path().display()
            )?;
            return Ok(());
        }
    };
    let state = board::export(
        &ledger,
        board_scope(env),
        env.config.caps.board_focus,
        &env::now(),
    );
    let path = a
        .out
        .unwrap_or_else(|| env.state_dir().join("board-state.json"));
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&state)?)?;
    let waiting = ledger
        .open()
        .iter()
        .filter(|i| i.needs_attention() && i.card.is_none())
        .count();
    writeln!(
        out,
        "wrote {} · {} card(s) · {} decided · {} default(s){}",
        path.display(),
        state.items.len(),
        state.decided.len(),
        state.defaults.len(),
        if waiting > 0 {
            format!(" · {waiting} open item(s) need grounding before they can be shown")
        } else {
            String::new()
        }
    )?;
    Ok(())
}

/// Whether this folder's items may go to the claude.ai board: only when its config chose the
/// `artifact` board (decision wi-7c21e4). A folder never initialised may be a client's, so the
/// defaults do not count as a choice.
fn publishable(env: &Env) -> Result<()> {
    env.board_allows(&BoardUse {
        modes: &[BoardMode::Artifact],
        before: " (its board stays local), before exporting the board",
        refusing: "not exporting the board",
        other_mode: "its items are not published to claude.ai, so there is no board to export \
                     (`weighin init --board artifact` changes the mode)",
    })
}

/// Where the board's items come from, as the board shows it.
fn board_scope(env: &Env) -> Scope {
    Scope {
        repo: env.worktree.clone().unwrap_or_else(|| {
            env.cwd
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        }),
        session: env
            .session
            .as_deref()
            .map_or_else(|| "all sessions".into(), |s| format!("session {s}")),
        source: env.branch.clone().unwrap_or_default(),
        generated_at: String::new(),
        // The board exchange format carries the mode as text, and `Scope` owns it.
        board_mode: env.config.board.mode.as_str().to_owned(),
    }
}

/// Apply board answers. From the board, a refused answer is a receipt and the command succeeds;
/// from the pane, which applies one answer at a time and must know, it fails the command after
/// the others are applied, and the board mode must allow a board at all (`pane::allowed`).
pub(crate) fn board_apply(env: &Env, a: &ApplyArgs, out: &mut impl Write) -> Result<()> {
    let pane = a.surface == AnswerSurface::Pane;
    if pane {
        pane::allowed(env)?;
    }
    let answers = match a.path.as_deref() {
        Some(path) if path != std::path::Path::new("-") => read_answers(path)?,
        Some(_) | None => answers_in(&pane::read_capped(io::stdin().lock())?, "")
            .context("reading answers from stdin")?,
    };
    let mut s = Session::open(env, "cli")?;
    let mut receipts = Receipts::new(out);
    let mut applied = Vec::new();
    let mut refused = 0_usize;
    for (doc_id, answer) in answers {
        let op = match answer_op(&s.ledger, &answer, a.surface) {
            Ok(Some(op)) => op,
            Ok(None) => continue,
            Err(e) => {
                refused += 1;
                receipts.line(format_args!("✗ {doc_id}: {e:#}"));
                continue;
            }
        };
        match s.apply(&op) {
            Ok(r) => {
                receipts.line(format_args!("✓ {r}"));
                applied.push(doc_id);
            }
            Err(e) => {
                refused += 1;
                receipts.line(format_args!("✗ {doc_id}: {e:#}"));
            }
        }
    }
    receipts.line(format_args!(
        "applied: {}",
        if applied.is_empty() {
            "none".into()
        } else {
            applied.join(" ")
        }
    ));
    let outcome = if pane && refused > 0 {
        Err(anyhow!("{refused} answer(s) from the pane refused"))
    } else {
        Ok(())
    };
    receipts.finish(outcome)
}

/// The operation an answer asks for, or `None` for one applied already. The pane may answer only
/// items a sitting grounded.
///
/// # Errors
/// Fails when the answer cannot be applied to the ledger as it stands.
fn answer_op(ledger: &Ledger, answer: &Answer, surface: AnswerSurface) -> Result<Option<Op>> {
    if surface == AnswerSurface::Pane {
        pane::grounded(ledger, answer)?;
    }
    match board::validate_answer(ledger, answer) {
        Ok(v) => Ok(Some(to_op(&answer.item_id, v, surface))),
        Err(board::AnswerError::AlreadyApplied) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn to_op(id: &str, v: ValidAnswer, surface: AnswerSurface) -> Op {
    let place = surface.place();
    match v {
        ValidAnswer::Choose {
            option,
            note,
            seconds,
        } => Op::Decide(DecideOp {
            id: id.into(),
            option,
            note: Some(note),
            provenance: None,
            via: Some(surface.surface().as_str().into()),
            seconds,
        }),
        ValidAnswer::Defer {
            trigger,
            resolve_by,
            default,
            note,
        } => Op::Defer(DeferOp {
            id: id.into(),
            flip: trigger.clone(),
            trigger,
            resolve_by,
            default,
            owner: None,
            unqualified: false,
            reason: if note.is_empty() {
                format!("deferred from the {}", surface.surface().as_str())
            } else {
                note
            },
        }),
        ValidAnswer::Dismiss { reason, note } => {
            let why = match reason.as_str() {
                "already-decided" => format!("already-decided:{}", surface.surface().as_str()),
                "duplicate" => format!("duplicate-of:{}", surface.surface().as_str()),
                r => r.to_owned(),
            };
            Op::Dismiss {
                id: id.into(),
                why,
                note: Some(note),
                reason: format!("dismissed {place}"),
            }
        }
        ValidAnswer::NotMyCall { who, note } => Op::Dismiss {
            id: id.into(),
            why: format!("not-my-call:{who}"),
            note: Some(note),
            reason: format!("handed off {place}"),
        },
        ValidAnswer::Reopen { note } => Op::Reopen {
            id: id.into(),
            note: Some(note),
            reason: format!("reopened {place}"),
        },
    }
}

/// Answers as `(document id, answer)`. Accepts one document, an array, `{ "id", "data" }` wrappers
/// as `ArtifactData` prints them, or a directory of such files.
fn read_answers(path: &std::path::Path) -> Result<Vec<(String, Answer)>> {
    let mut files = Vec::new();
    if path.is_dir() {
        let mut stack = vec![path.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d)?.flatten() {
                let p = e.path();
                // The entry's own type: a symlinked directory is not descended into, so a link
                // back up the tree cannot loop (H-13).
                let is_dir = e
                    .file_type()
                    .with_context(|| format!("reading {}", p.display()))?
                    .is_dir();
                if is_dir {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "json") {
                    files.push(p);
                }
            }
        }
    } else {
        files.push(path.to_path_buf());
    }
    let mut out = Vec::new();
    for f in files {
        let stem = f
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let answers = answers_in(&std::fs::read_to_string(&f)?, &stem)
            .with_context(|| format!("parsing {}", f.display()))?;
        out.extend(answers);
    }
    Ok(out)
}

/// The answers in one JSON text, as [`read_answers`] reads a file: a bare answer's document id is
/// `stem`, or its `item_id` when `stem` is empty (stdin).
fn answers_in(text: &str, stem: &str) -> Result<Vec<(String, Answer)>> {
    let docs = match serde_json::from_str(text)? {
        serde_json::Value::Array(a) => a,
        other => vec![other],
    };
    let mut out = Vec::with_capacity(docs.len());
    for d in docs {
        let (id, data) = match (d.get("id").and_then(|x| x.as_str()), d.get("data")) {
            (Some(id), Some(data)) => (id.to_owned(), data.clone()),
            _ => (stem.to_owned(), d),
        };
        let mut a: Answer = serde_json::from_value(data).with_context(|| format!("answer {id}"))?;
        if a.item_id.is_empty() {
            a.item_id.clone_from(&id);
        }
        let id = if id.is_empty() { a.item_id.clone() } else { id };
        out.push((id, a));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(unix)]
    #[cfg_attr(miri, ignore = "makes directories and symlinks on the file system")]
    fn answers_under_a_symlinked_directory_loop_are_read_once() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let answer = r#"{"item_id": "wi-000001", "action": "reopen"}"#;
        std::fs::write(dir.path().join("a.json"), answer)?;
        // One link, so a walk that follows it fails fast (on duplicates, at the kernel's 40-link
        // limit) rather than hanging as two links make it.
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop"))?;

        let answers = super::read_answers(dir.path())?;

        let ids: Vec<&str> = answers.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["a"]);
        Ok(())
    }
}
