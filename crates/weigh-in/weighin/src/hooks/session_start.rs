//! `SessionStart`: the prime for the model's context, and the one message a session start shows
//! the person (a broken config, a store that cannot be used, what the imports found, hook errors
//! and hooks that ran out of time since the last session).

use anyhow::Result;
use std::fmt::Write as _;
use std::path::PathBuf;
use weighin_core::Ledger;
use weighin_core::marker::Door;

use super::errors::{new_errors, refused_given_store};
use super::late::new_late;
use super::{Failures, Hook, HookOutput};
use crate::env::{self, Config, ConfigSource, Env};
use crate::ops::{self, read_ledger};
use crate::shell_bin;
use crate::text::{Report, clip};

/// `SessionStart`: the prime for Claude's context and, when hooks have failed since the last
/// session, the config is broken or the import has `journal` notes, a one-line message for the
/// user. A prime that fails is logged (in `failures` too) and reported with the rest, and Claude
/// still gets the convention. A broken config is reported here, once a session, and never
/// logged.
pub(super) fn session_start(
    hook: &Hook,
    failures: &mut Failures,
    source: &str,
    journal: Vec<String>,
) -> HookOutput {
    let env = &hook.env;
    let broken = broken_config(env);
    let (primed, unusable) = match ops::loaded(env) {
        Ok(loaded) => (
            prime_from(env, source, &loaded.ledger),
            loaded.unusable.total(),
        ),
        Err(e) => (Err(e), 0),
    };
    // A given store Core refuses is told to the user here, every session until it is fixed, and
    // not logged: the log would only repeat it.
    let mut refused = None;
    let mut context = primed.unwrap_or_else(|e| {
        if refused_given_store(env, &e) {
            refused = Some(format!(
                "Weigh-In cannot open its store, so hooks capture nothing until that is fixed: {}.",
                clip(&format!("{e:#}"), 400)
            ));
        } else {
            hook.log(failures, &e);
        }
        CONVENTION.to_owned()
    });
    if let Some(copy) = shell_copy(hook, failures) {
        context.push_str("Weigh-In binary for the shell: ");
        context.push_str(&copy.display().to_string());
        context.push('\n');
    }
    let parts: Vec<String> = broken
        .into_iter()
        .chain(refused)
        .chain(store_notes(env, unusable))
        .chain(journal)
        .chain(new_errors(env))
        .chain(new_late(env))
        .collect();
    HookOutput {
        context: Some(context),
        message: (!parts.is_empty()).then(|| parts.join(" ")),
    }
}

/// What the user should hear about the store, once a session: that it cannot be located (hooks
/// then write nothing, CLI-09), a session id the store refuses (D10), what locating it found (a
/// 2.x fallback, a suite config problem), and events a newer weighin wrote, which are skipped
/// (EV-10).
fn store_notes(env: &Env, unusable: u64) -> Vec<String> {
    let mut notes = Vec::new();
    if let Err(e) = &env.store {
        notes.push(format!(
            "Weigh-In cannot locate its store, so hooks capture nothing until that is fixed: {}.",
            Report(e)
        ));
    }
    if let Some(e) = &env.session_error {
        notes.push(format!(
            "Weigh-In: {e}, so this session's items are recorded with no session."
        ));
    }
    notes.extend(env.notices.iter().map(|n| format!("Weigh-In: {n}.")));
    if unusable > 0 {
        notes.push(format!(
            "Weigh-In: {unusable} event(s) in the store are of a form this weighin cannot read, so they are skipped; a newer weighin wrote them."
        ));
    }
    notes
}

/// On Windows inside a git repository, a copy of this binary in the repository's git dir, where
/// the skill's shell can reach it when it cannot see the plugin folder (#127, `shell_bin`). Nowhere
/// else: other platforms reach the plugin folder, and outside git the state dir would be a
/// `.weighin/` created in the user's folder. A failed copy is logged, and `SessionStart` goes on.
fn shell_copy(hook: &Hook, failures: &mut Failures) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    // The default store's state dir in the repository's git dir, not the state dir, which follows
    // a configured `store` to a folder the shell may not see. Under `redkiln/`, so nothing is
    // added beside a 1.x ledger in `<common>/weighin/`, which stays as it was, imported or not.
    let dir = hook.env.default_state_dir()?;
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            hook.log(failures, &anyhow::Error::from(e));
            return None;
        }
    };
    match shell_bin::place(&exe, &dir, env!("CARGO_PKG_VERSION")) {
        Ok(copy) => Some(copy),
        Err(unplaced) => {
            hook.log(failures, &unplaced.error);
            unplaced.existing
        }
    }
}

/// When the folder's config cannot be used, the sentence that leads the user's message. It is
/// not logged: `new_errors` would read it straight back as a hook failure, and every other
/// command reports it itself. Hooks never fail a session, so they carry on with the defaults,
/// which drop the config's settings (a `store` elsewhere among them) until the file is fixed.
fn broken_config(env: &Env) -> Option<String> {
    let ConfigSource::Broken(e) = &env.config_source else {
        return None;
    };
    // The fault only: the sentence names the file already, and a deep folder's absolute path
    // would push the fault past the clip.
    let fault = std::error::Error::source(e)
        .map_or_else(|| e.to_string(), |cause| Report(cause).to_string());
    Some(format!(
        "Weigh-In: {} is broken ({}). Until it is fixed, hooks run on the defaults, so items captured now go to the default ledger, not a `store` the config names, and other weighin commands stop.",
        Config::PATH,
        clip(&fault, 200)
    ))
}

/// The convention, the session key and the counts, in at most ~150 tokens.
///
/// # Errors
/// Fails when the ledger cannot be read.
pub(crate) fn prime(env: &Env, source: &str) -> Result<String> {
    // Read-only: a session that raises nothing must not leave a store behind.
    prime_from(env, source, &read_ledger(env)?)
}

/// [`prime`] of a ledger already read.
fn prime_from(env: &Env, source: &str, ledger: &Ledger) -> Result<String> {
    let open = ledger.open();
    let here = env
        .session
        .as_deref()
        .map_or(0, |id| ledger.open_in_session(id).len());
    let one_way = open.iter().filter(|i| i.door() == Door::OneWay).count();
    let defaults = ledger.unacknowledged_defaults().len();
    let due = match ledger
        .due_for_review(&env::now(), env.config.caps.review_age_days)
        .len()
    {
        0 => String::new(),
        n => format!(" · {n} decision(s) due a look"),
    };
    let mut out = String::new();
    writeln!(
        out,
        "Weigh-In ({source}): session {} · {here} open here · {} open in repo ({one_way} one-way) · {defaults} default(s) to review{due}.",
        env.session.as_deref().unwrap_or("?"),
        open.len()
    )?;
    out.push_str(CONVENTION);
    Ok(out)
}

#[cfg(test)]
mod tests;

pub(super) const COWORK_PRIME: &str = "Weigh-In (cowork): the ledger lives in the user's connected folder, not in this workspace; /weigh-in records there through the computer's shell.\n";

pub(super) const CONVENTION: &str = "When you assume, defer, make a call the user did not examine, or cannot decide, end your message with:\n```weigh-in\nkind: question|assumption|deferral|call\ndoor: one-way|two-way|unsure\nauthority: authority|judgment\nq: <the full question?>\ndid: <what you did meanwhile, or blocked>\nflip: <the fact that would change the answer>\nat: <path:line>\n```\nA list of calls for the user (\"Decisions for you\", \"Your calls\") gets one block per call. No TODO/FIXME/\"revisit later\" without a block. If you cannot ask the user, take no one-way path: `did: blocked`, then continue with independent work.\n";
