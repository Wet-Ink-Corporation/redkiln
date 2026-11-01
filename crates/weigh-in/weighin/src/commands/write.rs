//! The commands that go through the write path one way or another: `raise`, `batch`, `scan`,
//! and `decide`, `defer`, `dismiss` and `reopen` as one operation each.

use anyhow::{Context as _, Result, anyhow, bail};
use std::io::{self, Read as _, Write};
use weighin_core::marker::parse_markers;

use super::Receipts;
use crate::env::Env;
use crate::ops::{self, Op, RaiseInput, Session};
use crate::{BatchArgs, RaiseArgs, ScanArgs, hooks};

fn stdin() -> Result<String> {
    let mut s = String::new();
    io::stdin().read_to_string(&mut s)?;
    Ok(s)
}

pub(crate) fn raise(env: &Env, a: RaiseArgs, out: &mut impl Write) -> Result<()> {
    // Parsed before the session opens, so input with no blocks leaves no ledger behind (B-01).
    let inputs = raise_inputs(a)?;
    if inputs.is_empty() {
        bail!("no weigh-in blocks found on stdin");
    }
    let mut s = Session::open(env, "cli")?;
    let mut receipts = Receipts::new(out);
    let outcome = inputs.into_iter().try_for_each(|r| {
        receipts.line(format_args!("{}", s.apply(&Op::Raise(r))?));
        Ok(())
    });
    receipts.finish(outcome)
}

/// What `raise` raises: every `weigh-in` block on stdin with `--markers`, else the one question.
fn raise_inputs(a: RaiseArgs) -> Result<Vec<RaiseInput>> {
    if a.markers {
        let text = stdin()?;
        let ch = if a.channel == "manual" {
            "sweep".to_owned()
        } else {
            a.channel
        };
        return Ok(parse_markers(&text)
            .iter()
            .map(|m| {
                let mut r = RaiseInput::from_marker(m, weighin_core::event::Channel::Sweep);
                // Each input owns its channel.
                r.channel = Some(ch.clone());
                r
            })
            .collect());
    }
    let q =
        a.q.ok_or_else(|| anyhow!("--q is required (or --markers with blocks on stdin)"))?;
    Ok(vec![RaiseInput {
        q,
        kind: a.kind,
        door: a.door,
        authority: a.authority,
        did: a.did,
        flip: a.flip,
        at: a.at,
        channel: Some(a.channel),
        ..Default::default()
    }])
}

pub(crate) fn batch(env: &Env, a: BatchArgs, out: &mut impl Write) -> Result<()> {
    let text = match a.file {
        Some(f) => {
            std::fs::read_to_string(&f).with_context(|| format!("reading {}", f.display()))?
        }
        None => stdin()?,
    };
    let ops = ops::parse_ops(&text)?;
    let total = ops.len();
    let mut s = Session::open(env, "cli")?;
    let mut receipts = Receipts::new(out);
    let mut failed = 0;
    for (n, op) in ops.into_iter().enumerate() {
        match s.apply_parsed(op) {
            Ok(r) => receipts.line(format_args!("✓ {r}")),
            Err(e) => {
                failed += 1;
                receipts.line(format_args!("✗ op {n}: {e:#}"));
            }
        }
    }
    let outcome = if failed > 0 {
        Err(anyhow!(
            "{failed} of {total} operations failed; the rest were applied"
        ))
    } else {
        Ok(())
    };
    receipts.finish(outcome)
}

/// One operation through the write path, and its receipt.
pub(crate) fn apply_one(env: &Env, op: &Op, out: &mut impl Write) -> Result<()> {
    let mut s = Session::open(env, "cli")?;
    writeln!(out, "{}", s.apply(op)?)?;
    Ok(())
}

pub(crate) fn scan(env: &Env, a: ScanArgs, out: &mut impl Write) -> Result<()> {
    let mut receipts = Receipts::new(out);
    let mut total = 0;
    let outcome = a.paths.into_iter().try_for_each(|p| {
        // Relative to `--dir`, for the text read and for the anchor alike.
        let abs = if p.is_absolute() { p } else { env.cwd.join(&p) };
        let text =
            std::fs::read_to_string(&abs).with_context(|| format!("reading {}", abs.display()))?;
        for o in hooks::scan_text(env, &abs.to_string_lossy(), &[text.as_str()], "cli")? {
            total += 1;
            match o {
                ops::RaiseOutcome::New(id) => receipts.line(format_args!("raised {id}")),
                ops::RaiseOutcome::Existing(id) => {
                    receipts.line(format_args!("{id} already raised"));
                }
                ops::RaiseOutcome::Duplicate(id) => {
                    receipts.line(format_args!("linked to open {id}"));
                }
            }
        }
        Ok(())
    });
    if outcome.is_ok() && total == 0 {
        receipts.line(format_args!("nothing to raise"));
    }
    receipts.finish(outcome)
}

#[cfg(test)]
mod tests {
    use super::batch;
    use crate::env::{Config, ConfigSource, Env};
    use crate::ops::read_ledger;
    use crate::{BatchArgs, Stdout, is_closed_stdout};
    use std::io;

    /// A standard output whose reader stops after the first line.
    struct OneLine {
        written: Vec<u8>,
    }

    impl io::Write for OneLine {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.written.contains(&b'\n') {
                return Err(io::Error::from(io::ErrorKind::BrokenPipe));
            }
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_batch_whose_reader_stops_early_still_applies_every_operation() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let env = Env {
            cwd: dir.path().to_path_buf(),
            repo_root: None,
            common_dir: None,
            branch: None,
            head: None,
            worktree: None,
            session: Some("s1".into()),
            config: Config::default(),
            config_source: ConfigSource::Absent,
            store_override: Some(dir.path().join("ledger.sqlite3")),
            // Set, so the test never asks the machine's git for an owner.
            git_owner: Config::default().owner.into(),
        };
        let file = dir.path().join("batch.json");
        std::fs::write(
            &file,
            r#"[{"op":"raise","q":"one?"},{"op":"raise","q":"two?"},{"op":"raise","q":"three?"}]"#,
        )?;
        let mut out = Stdout(OneLine {
            written: Vec::new(),
        });

        let result = batch(&env, BatchArgs { file: Some(file) }, &mut out);

        let stopped = result.expect_err("the closed stdout is reported");
        assert!(is_closed_stdout(&stopped), "{stopped:#}");
        let printed = String::from_utf8(out.0.written)?;
        assert_eq!(printed.lines().count(), 1, "{printed}");
        assert_eq!(read_ledger(&env)?.open().len(), 3);
        Ok(())
    }
}
