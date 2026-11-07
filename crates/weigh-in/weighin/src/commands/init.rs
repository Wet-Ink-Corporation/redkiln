//! `init`: write the folder's config, and the repository class to `.redkiln/suite.toml`.

use anyhow::{Context as _, Result, bail};
use redkiln_core::suite::{ClassSource, LEGACY_CONFIG, SUITE_FILE};
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::InitArgs;
use crate::env::{Env, RepoClass};

pub(crate) fn init(env: &mut Env, a: InitArgs, out: &mut impl Write) -> Result<()> {
    let root = env.root();
    let suite_path = root.join(SUITE_FILE);
    let suite_exists = suite_path
        .try_exists()
        .with_context(|| format!("checking for {}", suite_path.display()))?;
    // Every refusal comes before any write.
    if let (Some(given), Some(set)) = (a.owner.as_deref(), env.suite_owner())
        && given != set.as_str()
    {
        bail!(
            "{SUITE_FILE} sets owner `{}`, which wins over config.json's; edit it by hand to \
             change the owner (`init` does not rewrite it)",
            set.as_str()
        );
    }
    let class_to_write = class_to_write(env, a.class, suite_exists)?;
    // A config's owner stands unless `--owner` is given; a first config takes git's (B-31).
    // suite.toml's owner is not copied in: it is read where it is.
    let owner = match a.owner {
        Some(o) => o,
        None => env.config_owner().to_owned(),
    };
    let c = &mut env.config;
    c.owner = owner;
    // Only what was given changes: a later `init --ledger` keeps a client repo's local board.
    if let Some(board) = a.board {
        c.board.mode = board;
    } else if let Some(class) = a.class {
        c.board.mode = class.default_board();
    }
    if a.board_url.is_some() {
        c.board.url = a.board_url;
    }
    if let Some(l) = a.ledger {
        c.store = Some(l).filter(|l| !l.trim().is_empty());
    }
    // suite.toml goes first. If config.json's save then fails, a re-run is safe: it finds
    // suite.toml, writes nothing to it, accepts the same `--class` and saves config.json.
    if let Some(class) = class_to_write {
        write_suite(&suite_path, class)?;
    }
    let p = env.config.save(&root)?;
    // The receipt names the store the next command uses, which `--ledger` may have moved, and
    // the class suite.toml now gives.
    env.relocate();
    let class_line = if class_to_write.is_some() {
        format!("wrote {SUITE_FILE}")
    } else {
        env.class_origin()
    };
    writeln!(
        out,
        "wrote {} · class {} · board {} · ledger {}\nclass {}: {class_line}",
        env.relative(&p.to_string_lossy()),
        env.repo_class(),
        env.config.board.mode,
        env.store_path()?.display(),
        env.repo_class(),
    )?;
    Ok(())
}

/// The class `init` writes to a new suite.toml, or `None` when one exists (never rewritten), in
/// which case a `given` class other than the repository's is refused.
fn class_to_write(
    env: &Env,
    given: Option<RepoClass>,
    suite_exists: bool,
) -> Result<Option<RepoClass>> {
    if suite_exists {
        if let Some(given) = given {
            refuse_another_class(env, given)?;
        }
        return Ok(None);
    }
    match given {
        Some(class) => Ok(Some(class)),
        None => first_class(env).map(Some),
    }
}

/// The class a suite.toml written with no `--class` gets: weigh-in 1.x's config.json's, which
/// it moves into suite.toml. With no class anywhere it refuses, before any write, rather than
/// guess (Ryan, #36 F1 A): a client's folder taken for `own` would publish its decisions.
fn first_class(env: &Env) -> Result<RepoClass> {
    match env.class_source() {
        ClassSource::Suite | ClassSource::Legacy => Ok(env.repo_class()),
        ClassSource::Default => bail!(
            "no repository class is set ({SUITE_FILE} and {LEGACY_CONFIG} set no repo_class): \
             pass `--class own` or `--class client`; `init` writes nothing without one"
        ),
    }
}

/// Refuses `given` when the suite.toml that is there makes the repository another class: `init`
/// never rewrites a file the user may have edited.
fn refuse_another_class(env: &Env, given: RepoClass) -> Result<()> {
    let current = env.repo_class();
    if given == current {
        return Ok(());
    }
    let why = match env.class_source() {
        ClassSource::Suite => format!("{SUITE_FILE} sets repo_class `{current}`"),
        ClassSource::Legacy => format!(
            "{SUITE_FILE} sets no repo_class, so the class `{current}` comes from {LEGACY_CONFIG}"
        ),
        ClassSource::Default => {
            format!("{SUITE_FILE} sets no usable repo_class, so the repository is `{current}`")
        }
    };
    bail!(
        "{why}; set `repo_class = \"{given}\"` in its `[suite]` table by hand to change the \
         class (`init` does not rewrite it)"
    )
}

/// Writes a new suite.toml at `path` holding only `[suite] repo_class` (ST-12): fixed text, as
/// the class is one of two names. Never over a file another writer made meanwhile.
fn write_suite(path: &Path, class: RepoClass) -> Result<()> {
    let context = || format!("writing {}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut file = match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            bail!("{} appeared while `init` ran; run it again", path.display())
        }
        Err(e) => return Err(e).with_context(context),
    };
    file.write_all(format!("[suite]\nrepo_class = \"{class}\"\n").as_bytes())
        .with_context(context)?;
    file.flush().with_context(context)
}
