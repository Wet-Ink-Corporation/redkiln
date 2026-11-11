//! The owner from git's configuration, ST-12's fallbacks below `[suite] owner`.

use super::Owner;

impl Owner {
    /// The owner git's configuration names (ST-12, below `[suite] owner`): the last `user.name`
    /// as a slug, else the local part of the last `user.email` as one; `None` when neither gives
    /// a slug. `listed` is the output of `git config --get-regexp '^user\.(name|email)$'`, one
    /// `<key> <value>` per line, which each binary runs itself (Core reads no process state; an
    /// exit 1 there means neither key is set, so `""`).
    ///
    /// A slug is lower-case ASCII words joined with `-`: any other ASCII character separates
    /// words, and characters outside ASCII are dropped (`Ryan Britton` is `ryan-britton`).
    #[must_use]
    pub fn from_git_config(listed: &str) -> Option<Self> {
        let last = |key: &str| {
            listed
                .lines()
                .filter_map(|l| l.split_once(' '))
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v)
                .next_back()
        };
        last("user.name")
            .and_then(slug)
            .or_else(|| {
                last("user.email")
                    .and_then(|e| e.split('@').next())
                    .and_then(slug)
            })
            // A slug is non-empty lower-case ASCII words joined by single dashes: `Owner`'s
            // grammar, so it needs no second check.
            .map(Self)
    }
}

/// `name` as lower-case ASCII words joined with `-`: any other ASCII character separates words,
/// and characters outside ASCII are dropped. `None` when no word is left.
fn slug(name: &str) -> Option<String> {
    let mut out = String::new();
    let mut gap = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if gap && !out.is_empty() {
                out.push('-');
            }
            gap = false;
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii() {
            gap = true;
        }
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The owner `listed` names, as weighin's `owner_in` returned it before it moved here.
    fn owner_in(listed: &str) -> Option<String> {
        Owner::from_git_config(listed).map(|owner| owner.as_str().to_owned())
    }

    #[test]
    fn a_name_becomes_lower_case_ascii_words_joined_by_dashes() {
        let slugs = [
            "Ana Lima",
            "  O'Brien,  Seán ",
            "J.R.R. Tolkien",
            "日本",
            "--",
        ]
        .map(slug);

        assert_eq!(
            slugs,
            [
                Some("ana-lima".to_owned()),
                Some("o-brien-sen".to_owned()),
                Some("j-r-r-tolkien".to_owned()),
                None,
                None,
            ]
        );
    }

    #[test]
    fn the_owner_is_the_git_user_name_else_the_email_local_part() {
        let both = "user.email ana@example.com\nuser.name Ana Lima\n";
        let email = "user.email j.doe@example.com\n";
        let neither = "user.name 日本\nuser.email @example.com\n";

        let owners = [both, email, neither, ""].map(owner_in);

        assert_eq!(
            owners,
            [
                Some("ana-lima".to_owned()),
                Some("j-doe".to_owned()),
                None,
                None
            ]
        );
    }

    #[test]
    fn the_last_git_user_name_wins_as_git_reads_it() {
        let listed = "user.name Global Name\nuser.name Repo Name\n";

        assert_eq!(owner_in(listed), Some("repo-name".to_owned()));
    }

    #[test]
    fn ryan_britton_is_ryan_britton_and_every_slug_is_an_owner() {
        let listed = "user.name Ryan Britton\n";

        let owner = Owner::from_git_config(listed).expect("an owner");

        assert_eq!(owner.as_str(), "ryan-britton");
        assert_eq!(owner.as_str().parse::<Owner>(), Ok(owner));
    }
}
