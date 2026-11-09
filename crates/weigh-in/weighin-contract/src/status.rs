//! The `data` of `weighin status --json`, schema `weighin.status/1` (SPEC CLI-12).
//!
//! CLI-12's common keys come first, then weighin's own (`hook_errors`, `hooks`); field order is
//! wire order. Every `Option` is written as `null`, never left out, since CLI-12 lists every key
//! as present. Unknown keys are accepted when read: CLI-04 lets a version grow additively, so a
//! reader takes what a later weighin adds. The suite's status view and the mod read it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The schema name and version the envelope carries.
pub const SCHEMA: &str = "weighin.status/1";

/// Weigh-In's install id, `status`'s `plugin`.
pub const PLUGIN: &str = "rk-weigh-in";

/// A plugin's health and where its store is (CLI-12), with weighin's hook health.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// Always [`PLUGIN`].
    pub plugin: String,
    /// The binary's version.
    pub version: String,
    /// The SPEC version the binary conforms to.
    pub spec: String,
    /// The store's path; `None` when it cannot be located.
    pub store: Option<String>,
    /// The step that chose the store (ST-03); `None` when it cannot be located.
    pub store_source: Option<StoreSource>,
    /// The store's schema as `<major>.<minor>` (ST-06); `None` when there is no store yet, or it
    /// was refused or cannot be located.
    pub store_schema: Option<String>,
    /// The repository class (ST-13).
    pub repo_class: RepoClass,
    /// Weigh-In events this build skipped, by type (EV-10).
    pub unreadable: BTreeMap<String, u64>,
    /// Telemetry events dropped on a busy lock (ST-09): always 0, since weighin records none.
    pub dropped: u64,
    /// `ok`, `degraded` or `off`, from the envelope's findings alone.
    pub health: Health,
    /// Lines of the hook error log the next `SessionStart` would report. A lower bound when the
    /// unreported part of the log is past the read cap (256 KiB): the lines before it are not
    /// counted, and the finding says "at least".
    pub hook_errors: u64,
    /// Each registered hook event's last run, by event name; an event with no record is absent.
    pub hooks: BTreeMap<String, HookRun>,
}

/// Why [`Status::from_envelope_data`] refused a document.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// The document is not this status's shape.
    #[error("not the data of {SCHEMA}")]
    Shape(#[from] serde_json::Error),
}

impl Status {
    /// The status in an envelope's `data`. The caller checks the envelope's `schema` is
    /// [`SCHEMA`] first.
    ///
    /// # Errors
    /// [`ReadError::Shape`] when `data` lacks a key, or a key holds another type or value.
    pub fn from_envelope_data(data: &serde_json::Value) -> Result<Self, ReadError> {
        Ok(Self::deserialize(data)?)
    }
}

/// The step of ST-03 that chose the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreSource {
    /// `--store`.
    Flag,
    /// `$REDKILN_STORE`.
    Env,
    /// `store` in `.redkiln/suite.toml`.
    Config,
    /// `<git-common-dir>/redkiln/`.
    GitCommonDir,
    /// `<folder>/.redkiln/`, outside git.
    Folder,
}

/// The repository class (ST-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepoClass {
    /// The owner's own repository.
    Own,
    /// A client's repository.
    Client,
}

/// How the plugin is doing: `off` with an error finding, `degraded` with a warning, else `ok`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    /// No error or warning.
    Ok,
    /// A warning: it works, with something to look at.
    Degraded,
    /// An error: capture is off.
    Off,
}

/// A hook event's last run, as its heartbeat slot records it (HK-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookRun {
    /// How it stands.
    pub state: HookState,
    /// Its last sign of life, RFC 3339: its last beat, or when it finished.
    pub at: String,
}

/// How a hook's last run stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookState {
    /// Running now.
    Running,
    /// Done.
    Finished,
    /// Killed by its harness timeout.
    TimedOut,
    /// Ended before its deadline without finishing.
    Interrupted,
}

#[cfg(test)]
mod tests {
    use super::{Health, HookState, ReadError, Status, StoreSource};
    use serde_json::{Value, json};

    const OK: &str = include_str!("../../../../spec/fixtures/status/valid/weighin-ok.json");
    const OFF: &str = include_str!("../../../../spec/fixtures/status/valid/weighin-off.json");

    /// The golden envelope's `data`, read through the contract.
    fn read(golden: &str) -> Status {
        let envelope: Value = serde_json::from_str(golden).expect("the golden is JSON");
        Status::from_envelope_data(&envelope["data"]).expect("the golden reads")
    }

    /// The bytes of the golden's `data`, as written: between `"data":` and `,"findings":`.
    fn data_bytes(golden: &str) -> &str {
        let (_, data) = golden.split_once("\"data\":").expect("a data key");
        let (data, _) = data.rsplit_once(",\"findings\":").expect("a findings key");
        data
    }

    #[test]
    fn the_golden_reads_and_writes_back_byte_for_byte() {
        for golden in [OK, OFF] {
            let status = read(golden);
            assert_eq!(
                serde_json::to_string(&status).expect("encoded"),
                data_bytes(golden),
                "field order is wire order"
            );
        }
        let ok = read(OK);
        assert_eq!(ok.health, Health::Ok);
        assert_eq!(ok.store_source, Some(StoreSource::GitCommonDir));
        assert_eq!(ok.hooks["Stop"].state, HookState::Finished);
    }

    #[test]
    fn a_later_key_is_accepted() {
        let envelope: Value = serde_json::from_str(OK).expect("JSON");
        let mut data = envelope["data"].clone();
        data["added_in_a_later_build"] = json!({"any": [1, 2]});
        let status = Status::from_envelope_data(&data).expect("CLI-04: additive");
        assert_eq!(status.plugin, "rk-weigh-in");
    }

    #[test]
    fn null_options_stay_null() {
        let off = read(OFF);
        assert_eq!(
            (&off.store, off.store_source, &off.store_schema),
            (&None, None, &None)
        );
        let written = serde_json::to_string(&off).expect("encoded");
        assert!(
            written.contains("\"store\":null,\"store_source\":null,\"store_schema\":null"),
            "never omitted: {written}"
        );
    }

    #[test]
    fn health_and_hook_states_spell_as_the_schema_says() {
        let spelled = |v: Value| v.as_str().map(str::to_owned);
        let health = [Health::Ok, Health::Degraded, Health::Off]
            .map(|h| spelled(serde_json::to_value(h).expect("encoded")));
        assert_eq!(
            health,
            ["ok", "degraded", "off"].map(|s| Some(s.to_owned()))
        );
        let states = [
            HookState::Running,
            HookState::Finished,
            HookState::TimedOut,
            HookState::Interrupted,
        ]
        .map(|s| spelled(serde_json::to_value(s).expect("encoded")));
        assert_eq!(
            states,
            ["running", "finished", "timed-out", "interrupted"].map(|s| Some(s.to_owned()))
        );
        let sources = [
            StoreSource::Flag,
            StoreSource::Env,
            StoreSource::Config,
            StoreSource::GitCommonDir,
            StoreSource::Folder,
        ]
        .map(|s| spelled(serde_json::to_value(s).expect("encoded")));
        assert_eq!(
            sources,
            ["flag", "env", "config", "git_common_dir", "folder"].map(|s| Some(s.to_owned()))
        );
    }

    #[test]
    fn another_shape_is_refused() {
        let refused = Status::from_envelope_data(&json!({"plugin": "rk-weigh-in"}));
        assert!(matches!(refused, Err(ReadError::Shape(_))), "{refused:?}");
    }
}
