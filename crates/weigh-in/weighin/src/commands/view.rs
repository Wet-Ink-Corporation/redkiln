//! The commands that read items as they stand: `open`, `show`, `context` and `trailer`.

use anyhow::{Result, anyhow};
use std::io::Write;
use weighin_core::marker::Door;
use weighin_core::{Item, Ledger};

use super::write_line;
use crate::context;
use crate::env::Env;
use crate::ops::{self, read_ledger};
use crate::{ItemArgs, OpenArgs, OpenFrom, TrailerArgs};

pub(crate) fn open(env: &Env, a: &OpenArgs, out: &mut impl Write) -> Result<()> {
    let ledger = read_ledger(env)?;
    let from = a.scope.scope();
    let scope_session = match from {
        OpenFrom::Session => env.session.as_deref(),
        OpenFrom::Repo | OpenFrom::All => None,
    };
    let items: Vec<&Item> = match (from, scope_session) {
        (OpenFrom::All, _) => ledger.items.values().collect(),
        (OpenFrom::Session, Some(id)) => ledger.open_in_session(id),
        (OpenFrom::Session | OpenFrom::Repo, _) => ledger.open(),
    };
    if a.json {
        writeln!(
            out,
            "{}",
            serde_json::to_string(&items.iter().take(a.top).collect::<Vec<_>>())?
        )?;
        return Ok(());
    }
    open_summary(&ledger, scope_session, out)?;
    if items.is_empty() {
        writeln!(out, "No decisions need you.")?;
    }
    for i in items.iter().take(a.top) {
        write_line(out, i)?;
    }
    if items.len() > a.top {
        writeln!(out, "… {} more (--top)", items.len() - a.top)?;
    }
    if a.with_context {
        for i in items
            .iter()
            .take(a.top)
            .filter(|i| i.needs_attention() && i.at.is_some())
        {
            writeln!(
                out,
                "{}",
                context::render(&context::build(env, i), env.config.caps.context_chars)
            )?;
        }
    }
    Ok(())
}

/// The counts line `open` starts with.
fn open_summary(ledger: &Ledger, session: Option<&str>, out: &mut impl Write) -> Result<()> {
    let repo_open = ledger.open();
    let one_way = repo_open
        .iter()
        .filter(|i| i.door() == Door::OneWay)
        .count();
    let defaults = ledger.unacknowledged_defaults().len();
    let here = session.map_or_else(String::new, |id| {
        format!(
            "{} open in this session · ",
            ledger.open_in_session(id).len()
        )
    });
    writeln!(
        out,
        "{here}{} open in repo ({one_way} one-way) · {defaults} default(s) to review",
        repo_open.len()
    )?;
    Ok(())
}

pub(crate) fn show(env: &Env, a: &ItemArgs, out: &mut impl Write) -> Result<()> {
    let ledger = read_ledger(env)?;
    let item = ledger
        .resolve(&a.id)
        .ok_or_else(|| anyhow!("no single item matches `{}`", a.id))?;
    if a.json {
        writeln!(out, "{}", serde_json::to_string_pretty(item)?)?;
        return Ok(());
    }
    write_line(out, item)?;
    if let Some(c) = &item.card {
        writeln!(out, "  card: {}", serde_json::to_string(c)?)?;
    }
    if let Some(d) = &item.decision {
        writeln!(
            out,
            "  decided {} by {} ({}) via {} · {}",
            d.option,
            d.decider,
            d.provenance.as_str(),
            d.via,
            d.ts
        )?;
    }
    for r in &item.records {
        writeln!(out, "  record: {} ({})", r.path, r.atom_id)?;
    }
    Ok(())
}

pub(crate) fn context_pack(env: &Env, a: &ItemArgs, out: &mut impl Write) -> Result<()> {
    let ledger = read_ledger(env)?;
    let item = ledger
        .resolve(&a.id)
        .ok_or_else(|| anyhow!("no single item matches `{}`", a.id))?;
    let pack = context::build(env, item);
    if a.json {
        writeln!(out, "{}", serde_json::to_string(&pack)?)?;
    } else {
        writeln!(
            out,
            "{}",
            context::render(&pack, env.config.caps.context_chars)
        )?;
    }
    Ok(())
}

pub(crate) fn trailer(env: &Env, a: &TrailerArgs, out: &mut impl Write) -> Result<()> {
    let ledger = read_ledger(env)?;
    let item = ledger
        .resolve(&a.id)
        .ok_or_else(|| anyhow!("no single item matches `{}`", a.id))?;
    let d = item
        .decision
        .as_ref()
        .ok_or_else(|| anyhow!("{} is not decided", item.id))?;
    writeln!(
        out,
        "{}",
        ops::Trailer {
            id: &item.id,
            option: &d.option
        }
    )?;
    Ok(())
}
