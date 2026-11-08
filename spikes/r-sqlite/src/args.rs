//! A subcommand and its `--key value` options, read from the process's arguments.

use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context as _, anyhow, bail};

/// The subcommand and its options.
#[derive(Debug)]
pub struct Args {
    /// The first argument.
    pub command: String,
    options: HashMap<String, String>,
}

impl Args {
    /// Reads the process's arguments; every one must be UTF-8.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut all = std::env::args_os().skip(1).map(|arg| {
            arg.into_string()
                .map_err(|raw| anyhow!("argument {} is not UTF-8", raw.display()))
        });
        let command = all.next().transpose()?.context("no subcommand")?;
        let mut options = HashMap::new();
        while let Some(key) = all.next().transpose()? {
            let Some(name) = key.strip_prefix("--") else {
                bail!("expected an option, found {key:?}");
            };
            let value = all
                .next()
                .transpose()?
                .with_context(|| format!("--{name} needs a value"))?;
            options.insert(name.to_owned(), value);
        }
        Ok(Self { command, options })
    }

    /// The option `name`, parsed.
    pub fn get<T>(&self, name: &str) -> anyhow::Result<T>
    where
        T: FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        let raw = self
            .options
            .get(name)
            .with_context(|| format!("--{name} is required"))?;
        raw.parse()
            .with_context(|| format!("--{name} {raw:?} does not parse"))
    }

    /// The option `name`, parsed, or `default` when absent.
    pub fn get_or<T>(&self, name: &str, default: T) -> anyhow::Result<T>
    where
        T: FromStr,
        T::Err: std::error::Error + Send + Sync + 'static,
    {
        if self.options.contains_key(name) {
            self.get(name)
        } else {
            Ok(default)
        }
    }

    /// The option `name` as a path.
    pub fn path(&self, name: &str) -> anyhow::Result<PathBuf> {
        self.get::<String>(name).map(PathBuf::from)
    }
}
