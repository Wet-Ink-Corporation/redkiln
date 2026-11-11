//! CLI-12's common keys of `bench status`: where the store is and how Bench is doing. They
//! come from the location, the open store (if any), the suite config and the findings, and
//! `status` creates nothing while reading them.

use std::collections::BTreeMap;
use std::path::{MAIN_SEPARATOR, Path};

use bench_contract::status::{Health, RepoClass, StoreSource};
use redkiln_core::suite::{self, Suite};

use super::Gathered;
use crate::envelope::{Finding, Severity};
use crate::error_chain::reason;
use crate::path_text::slashed;
use crate::paths::{self, GIT_TIMEOUT, PathError, StoreLocation};
use crate::store::Store;

/// What the open store says of itself: its schema and the events of Bench's types this build
/// skipped, by type; `None` and empty without a store.
#[derive(Debug, Default)]
pub(super) struct StoreFacts {
    /// `<major>.<minor>` of its stamp.
    pub(super) schema: Option<String>,
    /// Unusable Bench events, by stored type (EV-10).
    pub(super) unreadable: BTreeMap<String, u64>,
}

/// The facts of `store`, reading every Bench event once; a read that fails is a
/// `status/unreadable:unreadable` finding and leaves the count empty, and events skipped are
/// one `ledger/unusable` warning.
pub(super) fn store_facts(store: &Store, gathered: &mut Gathered) -> StoreFacts {
    let unreadable = match store.unusable_by_type() {
        Ok(by_type) => by_type,
        Err(error) => {
            gathered.unreadable("unreadable", &reason(&error));
            BTreeMap::new()
        }
    };
    let skipped = unreadable
        .values()
        .fold(0_u64, |sum, n| sum.saturating_add(*n));
    if skipped > 0 {
        gathered.findings.push(
            Finding::new(
                "ledger/unusable",
                format!(
                    "{skipped} event(s) in the ledger are of a form this bench cannot read, so \
                     every section skips them; a newer bench wrote them"
                ),
            )
            .with_severity(Severity::Warning),
        );
    }
    StoreFacts {
        schema: Some(store.schema()),
        unreadable,
    }
}

/// The store's path as text, with `/` separators on every platform.
pub(super) fn store_path(location: &StoreLocation) -> String {
    slashed(&location.path.to_string_lossy(), MAIN_SEPARATOR).into_owned()
}

/// ST-03's step as the contract spells it.
pub(super) const fn store_source(source: paths::StoreSource) -> StoreSource {
    match source {
        paths::StoreSource::Flag => StoreSource::Flag,
        paths::StoreSource::Env => StoreSource::Env,
        paths::StoreSource::Config => StoreSource::Config,
        paths::StoreSource::GitCommonDir => StoreSource::GitCommonDir,
        paths::StoreSource::Folder => StoreSource::Folder,
    }
}

/// The repository class (ST-13): the suite config's that locating read, else (`--store` or a
/// variable chose first) the config of the worktree at `cwd`, or of `cwd` outside git. git
/// failing otherwise is a `status/unreadable:repo_class` finding and the class is ST-13's
/// default, `client`.
pub(super) fn repo_class(
    location: &StoreLocation,
    cwd: &Path,
    gathered: &mut Gathered,
) -> RepoClass {
    let class = match location.repo_class {
        Some(class) => class,
        None => match paths::worktree_root_within(cwd, GIT_TIMEOUT) {
            Ok(root) => Suite::load(Path::new(&root)).repo_class(),
            Err(PathError::NotARepository { .. }) => Suite::load(cwd).repo_class(),
            Err(error) => {
                gathered.unreadable("repo_class", &reason(&error));
                suite::RepoClass::Client
            }
        },
    };
    match class {
        suite::RepoClass::Own => RepoClass::Own,
        suite::RepoClass::Client => RepoClass::Client,
    }
}

/// `off` with an error finding, `degraded` with a warning, else `ok`: from the findings alone,
/// so the two cannot disagree.
pub(super) fn health(findings: &[Finding]) -> Health {
    let has = |severity| findings.iter().any(|f| f.severity == Some(severity));
    if has(Severity::Error) {
        Health::Off
    } else if has(Severity::Warning) {
        Health::Degraded
    } else {
        Health::Ok
    }
}
