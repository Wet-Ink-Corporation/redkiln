//! What a decision or a deferral writes back beside the ledger: `.kb` atoms in `_intake`, the
//! open-question atom a decision supersedes, and the arrow on the line a question came from.

use anyhow::{Context as _, Result};
use std::fmt::Write as _;
use weighin_core::atom::{self, RenderContext};
use weighin_core::event::{LedgerEvent, RecordWritten};
use weighin_core::id::ItemId;

use super::paths::syntax_of;
use super::{Session, Trailer};
use crate::annotate::{Decision, Skip, annotate_source_line};

impl Session<'_> {
    /// Write the decision's atom and source annotation, and say what was written.
    pub(super) fn decision_receipt(
        &mut self,
        id: &ItemId,
        option: &str,
        matched: Option<bool>,
    ) -> Result<String> {
        let mut receipt = format!("decided {id} → {option}");
        if matched == Some(false) {
            receipt.push_str(" (override)");
        }
        if let Some(path) = self.write_decision_atom(id)? {
            write!(receipt, " · atom {path}")?;
            if let Some(tracking) = self.supersede_open_question(id, option)? {
                write!(receipt, " · superseded {tracking}")?;
            }
        }
        match self.annotate_source(id, option)? {
            Some(Annotation::Written(src)) => write!(receipt, " · annotated {src}")?,
            Some(Annotation::Skipped(src, skip)) => {
                write!(receipt, " · not annotated {src} ({})", skip.reason())?;
            }
            None => {}
        }
        write!(receipt, " · trailer `{}`", Trailer { id, option })?;
        Ok(receipt)
    }

    fn write_file(&self, rel: &str, text: &str) -> Result<()> {
        let path = self.env.root().join(rel);
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(&path, text.replace("\r\n", "\n"))?;
        Ok(())
    }

    fn write_decision_atom(&mut self, id: &ItemId) -> Result<Option<String>> {
        let Some(item) = self.ledger.get(id).cloned() else {
            return Ok(None);
        };
        let Some(text) = atom::render_decision(&item, &render_ctx(&self.today)) else {
            return Ok(None);
        };
        let rel = atom::decision_path(&item);
        self.write_file(&rel, &text)?;
        self.write(
            id,
            vec![LedgerEvent::RecordWritten(RecordWritten {
                item_id: id.clone(),
                path: rel.clone(),
                atom_id: atom::decision_atom_id(&item),
                atom_kind: "decision".into(),
                reason: "decision rendered to .kb/_intake for kb-ingest".into(),
            })],
        )?;
        Ok(Some(rel))
    }

    pub(super) fn write_open_question(&mut self, id: &ItemId) -> Result<Option<String>> {
        let Some(item) = self.ledger.get(id).cloned() else {
            return Ok(None);
        };
        let Some(text) = atom::render_open_question(&item, &render_ctx(&self.today)) else {
            return Ok(None);
        };
        let rel = atom::open_question_path(&item);
        self.write_file(&rel, &text)?;
        self.write(
            id,
            vec![LedgerEvent::RecordWritten(RecordWritten {
                item_id: id.clone(),
                path: rel.clone(),
                atom_id: atom::open_question_atom_id(&item),
                atom_kind: "open_question".into(),
                reason: "one-way item deferred past its session".into(),
            })],
        )?;
        Ok(Some(rel))
    }

    /// Mark the open-question atom a deferral wrote for `id`, if any, as superseded by the
    /// decision atom just written, so it no longer reads as open. The file is overwritten in place
    /// (B-01); one already closed, or no longer there, is left alone.
    fn supersede_open_question(&self, id: &ItemId, option: &str) -> Result<Option<String>> {
        let Some(item) = self.ledger.get(id) else {
            return Ok(None);
        };
        let Some(record) = item
            .records
            .iter()
            .rev()
            .find(|r| r.atom_kind == "open_question")
        else {
            return Ok(None);
        };
        let path = self.env.root().join(&record.path);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let settled = Settled {
            option,
            decision_atom: &atom::decision_atom_id(item),
            date: &self.today,
        };
        let Some(closed) = superseded_open_question(&text, &settled)? else {
            return Ok(None);
        };
        self.write_file(&record.path, &closed)?;
        Ok(Some(record.path.clone()))
    }

    /// Move the `last_reviewed` date of the decision atom written for `id` to today, after a
    /// review left the decision standing (D-0022), and return its path. The file is overwritten
    /// in place (B-01); one no longer there, or with no frontmatter date, is left alone.
    pub(super) fn mark_decision_atom_reviewed(&self, id: &ItemId) -> Result<Option<String>> {
        let Some(record) = self.ledger.get(id).and_then(|item| {
            item.records
                .iter()
                .rev()
                .find(|r| r.atom_kind == "decision")
        }) else {
            return Ok(None);
        };
        let path = self.env.root().join(&record.path);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        let Some(reviewed) = reviewed_atom(&text, &self.today)? else {
            return Ok(None);
        };
        self.write_file(&record.path, &reviewed)?;
        Ok(Some(record.path.clone()))
    }

    /// spec-kit's write-back: the line the question came from gets an arrow, so it is not raised
    /// again. Only prose is written into ([`syntax_of`], [`annotate_source_line`]); a code file,
    /// or a line the option cannot go on, is left as it was, and the receipt says so.
    fn annotate_source(&self, id: &ItemId, option: &str) -> Result<Option<Annotation>> {
        let Some(item) = self.ledger.get(id) else {
            return Ok(None);
        };
        let Some(src) = &item.source_path else {
            return Ok(None);
        };
        let path = self.env.root().join(src);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        let first = item.quote.lines().next().unwrap_or("").trim();
        if first.is_empty() {
            return Ok(None);
        }
        let syntax = syntax_of(src);
        let decision = Decision {
            id: id.as_str(),
            option,
            date: &self.today,
        };
        // Rebuild with each line's own terminator so a CRLF file stays CRLF. `found` is the
        // outcome on the question's line: `None` until it is found, then the skip, if any.
        let mut found = None;
        let mut joined = String::with_capacity(text.len() + 64);
        for chunk in text.split_inclusive('\n') {
            let (body, end) = match chunk.strip_suffix("\r\n") {
                Some(b) => (b, "\r\n"),
                None => match chunk.strip_suffix('\n') {
                    Some(b) => (b, "\n"),
                    None => (chunk, ""),
                },
            };
            if found.is_none() && body.trim() == first && !body.contains("→ Weigh-In") {
                let rewritten = annotate_source_line(body, &decision, syntax);
                joined.push_str(rewritten.as_deref().unwrap_or(body));
                found = Some(rewritten.err());
            } else {
                joined.push_str(body);
            }
            joined.push_str(end);
        }
        let annotation = match found {
            None => return Ok(None),
            Some(Some(skip)) => Annotation::Skipped(src.clone(), skip),
            Some(None) => {
                std::fs::write(&path, joined)
                    .with_context(|| format!("writing {}", path.display()))?;
                Annotation::Written(src.clone())
            }
        };
        Ok(Some(annotation))
    }
}

/// What settled a deferred item, as its open-question atom records it.
struct Settled<'a> {
    option: &'a str,
    decision_atom: &'a str,
    date: &'a str,
}

/// `text`, an open-question atom, rewritten as superseded by the decision `s`: `.kb`'s
/// `status: superseded` with `superseded_by` and `resolution_ref`, a settled summary and a
/// Resolution section. `None` when `text` is not an open one (no `status: proposed` in its
/// frontmatter).
fn superseded_open_question(text: &str, s: &Settled<'_>) -> Result<Option<String>> {
    let Some((front, body)) = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return Ok(None);
    };
    if !front.lines().any(|l| l == "status: proposed") {
        return Ok(None);
    }
    let summary = format!(
        "Settled on {}: decided {} ({}).",
        s.date, s.option, s.decision_atom
    );
    // YAML strings are written as JSON strings, as the atoms write them.
    let yaml = serde_json::to_string;
    let mut out = String::with_capacity(text.len().saturating_add(256));
    out.push_str("---\n");
    for line in front.lines() {
        if line == "status: proposed" {
            writeln!(out, "status: superseded")?;
            writeln!(out, "superseded_by: {}", yaml(s.decision_atom)?)?;
            // The key the design and redkiln's `.kb` schema name for an open question's answer.
            writeln!(out, "resolution_ref: {}", yaml(s.decision_atom)?)?;
            continue;
        }
        match line.split_once(':').map(|(key, _)| key) {
            Some("summary") => writeln!(out, "summary: {}", yaml(&summary)?)?,
            Some("last_reviewed") => writeln!(out, "last_reviewed: {}", yaml(s.date)?)?,
            _ => writeln!(out, "{line}")?,
        }
    }
    out.push_str("---\n");
    out.push_str(body.trim_end());
    write!(
        out,
        "\n\n## Resolution\n\nSettled on {}: decided **{}**, recorded in `{}`. The deferral above no longer applies.\n",
        s.date, s.option, s.decision_atom
    )?;
    Ok(Some(out))
}

/// `text`, an atom, with its frontmatter `last_reviewed` set to `date` and every other byte as
/// it was. `None` when `text` has no frontmatter or no `last_reviewed` line in it.
fn reviewed_atom(text: &str, date: &str) -> Result<Option<String>> {
    let Some((front, body)) = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
    else {
        return Ok(None);
    };
    if !front.lines().any(|l| l.starts_with("last_reviewed:")) {
        return Ok(None);
    }
    let mut out = String::with_capacity(text.len());
    out.push_str("---\n");
    for line in front.lines() {
        if line.starts_with("last_reviewed:") {
            // YAML strings are written as JSON strings, as the atoms write them.
            writeln!(out, "last_reviewed: {}", serde_json::to_string(date)?)?;
        } else {
            writeln!(out, "{line}")?;
        }
    }
    out.push_str("---\n");
    out.push_str(body);
    Ok(Some(out))
}

/// What a decision did to the line its question came from.
enum Annotation {
    /// The arrow was written into this file.
    Written(String),
    /// This file was left as it was, for this reason.
    Skipped(String, Skip),
}

fn render_ctx(today: &str) -> RenderContext {
    RenderContext {
        date: today.to_owned(),
        depends_on: Vec::new(),
        source_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{Settled, reviewed_atom, superseded_open_question};
    use crate::ops::Session;
    use crate::ops::test_support::{ONE_WAY, TWO_WAY, card, env_in, id_of, run};

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn deferring_a_one_way_door_writes_a_tracking_atom() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let batch = serde_json::json!([
            {"op": "raise", "q": ONE_WAY, "door": "one-way"},
            {"op": "triage", "id": "@1", "door": "one-way", "blast": "module", "route": "present", "reason": "r"},
            {"op": "defer", "id": "@1", "flip": "a tenant asks", "trigger": "the tenant survey", "resolve_by": "2026-12-01", "default": "A"}
        ]);

        let out = run(&mut Session::open(&env, "t")?, &batch)?;

        let id = id_of(ONE_WAY);
        assert_eq!(
            out.get(2),
            Some(&format!(
                "✓ deferred {id} until the tenant survey (default A by 2026-12-01) · tracking atom .kb/_intake/open_questions/{id}-should-retention-attach-per-record-or-per-series.md"
            ))
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn deciding_a_deferred_one_way_door_supersedes_its_tracking_atom() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let batch = serde_json::json!([
            {"op": "raise", "q": ONE_WAY, "door": "one-way"},
            {"op": "triage", "id": "@1", "door": "one-way", "blast": "module", "route": "present", "reason": "r"},
            {"op": "defer", "id": "@1", "flip": "a tenant asks", "trigger": "the tenant survey", "resolve_by": "2026-12-01", "default": "A"},
            {"op": "decide", "id": "@1", "option": "B"}
        ]);

        let out = run(
            &mut Session::open(&env, "t")?.with_today("2026-10-07"),
            &batch,
        )?;

        let id = id_of(ONE_WAY);
        let rel = format!(
            ".kb/_intake/open_questions/{id}-should-retention-attach-per-record-or-per-series.md"
        );
        assert!(
            out.get(3)
                .is_some_and(|r| r.contains(&format!(" · superseded {rel} · trailer"))),
            "{out:?}"
        );
        let atom = std::fs::read_to_string(dir.path().join(&rel))?;
        assert_eq!(
            atom,
            format!(
                "---\nid: \"kb-open-question-{id}\"\ntitle: \"{ONE_WAY}\"\nkind: open_question\nstatus: superseded\nsuperseded_by: \"kb-decision-{id}\"\nresolution_ref: \"kb-decision-{id}\"\nauthority_tier: note\nsummary: \"Settled on 2026-10-07: decided B (kb-decision-{id}).\"\ndepends_on: []\nrelated: []\nsource_paths: []\nlast_reviewed: \"2026-10-07\"\ntracks: \"{id}\"\nresolve_by: \"2026-12-01\"\ntrigger: \"the tenant survey\"\ndefault_if_unresolved: \"A\"\nowner: \"ryan\"\nflip_condition: \"a tenant asks\"\n---\n\n# {ONE_WAY}\n\nDeferred on purpose. Waiting for: the tenant survey. Resolve by: 2026-12-01. If nobody acts, **A** applies.\n\n## Resolution\n\nSettled on 2026-10-07: decided **B**, recorded in `kb-decision-{id}`. The deferral above no longer applies.\n"
            )
        );
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn reviewing_a_decision_moves_its_atom_last_reviewed_and_nothing_else() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let env = env_in(dir.path(), "s1");
        let decided = run(
            &mut Session::open(&env, "t")?.with_today("2026-01-02"),
            &serde_json::json!([
                {"op": "raise", "q": TWO_WAY, "door": "two-way"},
                {"op": "ground", "id": "@1", "card": card()},
                {"op": "decide", "id": "@1", "option": "A"}
            ]),
        )?;
        let id = id_of(TWO_WAY);
        let rel = format!(".kb/_intake/decisions/{id}-name-the-helper-apply-or-run.md");
        assert!(
            decided.get(2).is_some_and(|r| r.contains(&rel)),
            "{decided:?}"
        );
        let before = std::fs::read_to_string(dir.path().join(&rel))?;

        let out = run(
            &mut Session::open(&env, "t")?.with_today("2026-04-10"),
            &serde_json::json!([{"op": "ack", "id": id.as_str()}]),
        )?;

        assert_eq!(
            out,
            [format!(
                "✓ reviewed decision on {id}: still holds · atom {rel}"
            )]
        );
        let after = std::fs::read_to_string(dir.path().join(&rel))?;
        assert_eq!(
            after,
            before.replace(
                "last_reviewed: \"2026-01-02\"",
                "last_reviewed: \"2026-04-10\""
            )
        );
        assert_ne!(after, before, "the date moved");
        Ok(())
    }

    #[test]
    fn a_reviewed_atom_rewrites_only_its_frontmatter_date() -> Result<()> {
        let atom = "---\nid: \"kb-decision-wi-1\"\nlast_reviewed: \"2026-01-02\"\n---\n\n# Q\n\nlast_reviewed: \"2026-01-02\"\n";

        let out = reviewed_atom(atom, "2026-04-10")?;

        assert_eq!(
            out.as_deref(),
            Some(
                "---\nid: \"kb-decision-wi-1\"\nlast_reviewed: \"2026-04-10\"\n---\n\n# Q\n\nlast_reviewed: \"2026-01-02\"\n"
            )
        );
        assert_eq!(reviewed_atom("# no frontmatter\n", "2026-04-10")?, None);
        Ok(())
    }

    #[test]
    fn a_tracking_atom_already_superseded_is_left_alone() -> Result<()> {
        let atom = "---\nid: \"kb-open-question-wi-1\"\nstatus: superseded\nsuperseded_by: \"kb-decision-wi-1\"\n---\n\n# Q\n";
        let settled = Settled {
            option: "A",
            decision_atom: "kb-decision-wi-1",
            date: "2026-10-08",
        };

        let out = superseded_open_question(atom, &settled)?;

        assert_eq!(out, None);
        Ok(())
    }
}
