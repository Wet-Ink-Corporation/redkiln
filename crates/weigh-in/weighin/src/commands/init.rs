//! `init`: write the folder's config.

use anyhow::Result;
use std::io::Write;

use crate::InitArgs;
use crate::env::Env;

pub(crate) fn init(env: &mut Env, a: InitArgs, out: &mut impl Write) -> Result<()> {
    // A config's owner stands unless `--owner` is given; a first config takes git's (B-31).
    let owner = match a.owner {
        Some(o) => o,
        None => env.owner().to_owned(),
    };
    let c = &mut env.config;
    c.owner = owner;
    // Only what was given changes: a later `init --ledger` keeps a client repo's local board.
    match (a.class, a.board) {
        (class, Some(board)) => {
            if let Some(class) = class {
                c.repo_class = class;
            }
            c.board.mode = board;
        }
        (Some(class), None) => {
            c.board.mode = class.default_board();
            c.repo_class = class;
        }
        (None, None) => {}
    }
    if a.board_url.is_some() {
        c.board.url = a.board_url;
    }
    if let Some(l) = a.ledger {
        c.store = Some(l).filter(|l| !l.trim().is_empty());
    }
    let p = env.config.save(&env.root())?;
    writeln!(
        out,
        "wrote {} · class {} · board {} · ledger {}",
        env.relative(&p.to_string_lossy()),
        env.config.repo_class,
        env.config.board.mode,
        env.store_path().display()
    )?;
    Ok(())
}
