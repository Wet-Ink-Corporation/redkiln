//! The ledger store: happenstance's SQLite event store at `<git-common-dir>/weighin/`.
//!
//! Every write is conditioned (DCB): raising an item fails if that item was already raised, and
//! every later event for an item fails if the item changed since the writer read it. Two sessions
//! deciding the same item at once therefore cannot both win; the loser re-reads and sees the
//! decision. This is the property the per-session JSONL design needed a projector conflict state
//! for; the store now enforces it at write time.
//!
//! # Journal mode
//!
//! happenstance opens its stores in WAL, and SQLite's own default is DELETE. Both delete files
//! beside the database: WAL removes its `-wal` and `-shm` on close, DELETE removes its journal
//! at every commit. Cowork's connected folders refuse deletes (`SQLITE_IOERR_DELETE`, 2570) and
//! do not support WAL's shared memory, so the ledger runs under TRUNCATE instead: one journal
//! beside the ledger, emptied at each commit, never deleted. It works on every disk the ledger
//! lands on. The ledger writes a few events per turn and does not need WAL's concurrency.
//!
//! [`SqliteEventStore::open`] would switch a file to WAL before migrating it, so the ledger
//! does not go through it. [`Store::open`] opens its own connection and, for a new file, copies
//! the schema happenstance builds in a template on a local temporary disk. The copy runs inside
//! one `BEGIN IMMEDIATE` transaction, so two processes creating the same ledger cannot both
//! write a schema.
//!
//! # Schema version
//!
//! The ledger's own schema version is `PRAGMA user_version` ([`LEDGER_SCHEMA`]); happenstance
//! keeps its migration counter in `store_meta` and never touches the pragma. A new ledger is
//! stamped in the transaction that copies its schema. Ledgers from 0.1.x and 0.2.x read 0 and are
//! brought up step by step on first open, in one `BEGIN IMMEDIATE`; a ledger stamped by a newer
//! build is refused and left as it is. A schema change in a later happenstance is a new step.

use anyhow::{Context, Result, anyhow, bail};
use happenstance_core::{
    AppendCondition, AppendError, Event, MAX_TAG_LEN, Query, QueryItem, ReadOptions,
    SendEventStore, SequencePosition, Tag, Tags, collect,
};
use happenstance_sqlite::event_store::SqliteEventStore;
use happenstance_sqlite::rusqlite::{self, Connection};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use weighin_core::event::{EVENT_TYPES, EventEnvelope};
use weighin_core::{ItemId, Ledger};

/// The journal mode every ledger connection runs under, as `PRAGMA journal_mode` reports it.
pub(crate) const JOURNAL_MODE: &str = "truncate";

/// The ledger schema this build reads and writes, kept in `PRAGMA user_version`.
///
/// 1. happenstance 0.3.2's event store schema under a TRUNCATE journal. Ledgers written by
///    0.1.x (WAL) and 0.2.x (TRUNCATE) have this schema unstamped, as 0.
pub(crate) const LEDGER_SCHEMA: i64 = 1;

/// How long a connection waits for a lock, and for a WAL ledger to be released to TRUNCATE.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) struct Store {
    inner: SqliteEventStore,
    rt: tokio::runtime::Runtime,
}

/// What a conditioned append must not find.
#[derive(Debug, Clone)]
pub(crate) enum Guard {
    /// Fail if `item_raised` for this item exists.
    NotYetRaised(ItemId),
    /// Fail if any event for this item was appended after `position`.
    ItemUnchangedSince(ItemId, u64),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Appended {
    At(u64),
    /// The guard matched: someone else wrote first.
    Conflict,
}

fn tag(key: &str, value: &str) -> Option<Tag> {
    let mut v: String = value.chars().filter(|c| !c.is_control()).collect();
    let budget = MAX_TAG_LEN.saturating_sub(key.len() + 1);
    while v.len() > budget {
        v.pop();
    }
    if v.is_empty() {
        None
    } else {
        Tag::key_value(key, &v).ok()
    }
}

fn item_tags(id: &ItemId) -> Result<Tags> {
    Tags::from_pairs([("item", id.as_str())]).map_err(|e| anyhow!("{e}"))
}

impl Store {
    /// Open (creating when absent) the store at `path`.
    ///
    /// # Errors
    /// Fails when the file cannot be opened or migrated.
    pub(crate) fn open(path: &Path) -> Result<Self> {
        if let Some(d) = path.parent()
            && !d.as_os_str().is_empty()
        {
            std::fs::create_dir_all(d).with_context(|| format!("creating {}", d.display()))?;
        }
        let opened = (|| {
            set_aside_wal(path, true)?;
            let conn = connect(path)?;
            // TRUNCATE is in force, so no connection is using a WAL now; one left beside the
            // ledger is stale (its frames were checkpointed when the mode switched) and must not
            // be replayed by a later WAL opener.
            set_aside_wal(path, false)?;
            ensure_schema(&conn)?;
            migrate(&conn)?;
            SqliteEventStore::new(conn).map_err(|e| anyhow!("{e}"))
        })();
        let inner = opened.with_context(|| format!("opening ledger {}", path.display()))?;
        let rt = tokio::runtime::Builder::new_current_thread().build()?;
        Ok(Self { inner, rt })
    }

    /// Every ledger event, in position order.
    ///
    /// # Errors
    /// Fails on a store error or an undecodable payload.
    pub(crate) fn read_all(&self) -> Result<Vec<(u64, EventEnvelope)>> {
        let query = Query::from_item(
            QueryItem::of_types(EVENT_TYPES.iter().copied()).map_err(|e| anyhow!("{e}"))?,
        );
        let events = self
            .rt
            .block_on(collect(self.inner.read(&query, ReadOptions::new())))
            .map_err(|e| anyhow!("reading ledger: {e}"))?;
        let mut out = Vec::with_capacity(events.len());
        for se in events {
            let pos = u64::from(se.position);
            let env = EventEnvelope::from_json(se.event.data())
                .with_context(|| format!("decoding event at position {pos}"))?;
            out.push((pos, env));
        }
        Ok(out)
    }

    /// # Errors
    /// See [`Store::read_all`].
    pub(crate) fn ledger(&self) -> Result<Ledger> {
        let events = self.read_all()?;
        Ok(Ledger::fold(events.iter().map(|(p, e)| (*p, e))))
    }

    /// Append events for one item atomically under `guard`.
    ///
    /// # Errors
    /// Fails on a store error (a guard match is [`Appended::Conflict`], not an error).
    pub(crate) fn append(&self, envs: &[EventEnvelope], guard: &Guard) -> Result<Appended> {
        let mut events = Vec::with_capacity(envs.len());
        for env in envs {
            let tags: Vec<Tag> = env.tags().iter().filter_map(|(k, v)| tag(k, v)).collect();
            let tags = Tags::from_pairs(tags.iter().filter_map(|t| t.key().zip(t.value())))
                .map_err(|e| anyhow!("{e}"))?;
            let ev = Event::new(env.event.event_type(), env.to_json()?)
                .map_err(|e| anyhow!("{e}"))?
                .with_tags(tags);
            events.push(ev);
        }
        let condition = match guard {
            Guard::NotYetRaised(id) => Some(AppendCondition::new(Query::from_item(
                QueryItem::new(["item_raised"], item_tags(id)?).map_err(|e| anyhow!("{e}"))?,
            ))),
            Guard::ItemUnchangedSince(id, pos) => {
                let q = Query::from_item(
                    QueryItem::tagged(item_tags(id)?).map_err(|e| anyhow!("{e}"))?,
                );
                Some(AppendCondition::new(q).after_opt(SequencePosition::new(*pos)))
            }
        };
        match self
            .rt
            .block_on(self.inner.append(&events, condition.as_ref()))
        {
            Ok(p) => Ok(Appended::At(u64::from(p))),
            Err(AppendError::ConditionViolated(_)) => Ok(Appended::Conflict),
            Err(e) => Err(anyhow!("appending to ledger: {e}")),
        }
    }
}

/// `<ledger><suffix>`: the journal, WAL and shared-memory files SQLite keeps beside a database.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Rename a `-wal` beside the ledger aside; renaming is allowed where deleting is not.
///
/// * Before connecting (`only_if_empty`): SQLite deletes a `-wal` it finds beside an empty
///   database, and a failed WAL creation (weighin 0.1.x in a Cowork folder) leaves exactly that.
/// * After switching to TRUNCATE: converting a 0.1.x WAL ledger where deletes are refused
///   leaves its checkpointed `-wal` behind.
fn set_aside_wal(path: &Path, only_if_empty: bool) -> Result<()> {
    let wal = sibling(path, "-wal");
    let empty = std::fs::metadata(path).map_or(true, |m| m.len() == 0);
    if (empty || !only_if_empty) && wal.exists() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let aside = sibling(path, &format!("-wal.orphaned-{stamp}"));
        std::fs::rename(&wal, &aside)
            .with_context(|| format!("setting aside {}", wal.display()))?;
    }
    Ok(())
}

/// Open (creating when absent) a connection under [`JOURNAL_MODE`].
fn connect(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    set_journal_mode(&conn)?;
    // FULL, not NORMAL: under a rollback journal NORMAL can lose the last commit on power loss.
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

/// Switch to [`JOURNAL_MODE`]. A ledger an older build left in WAL converts on first open; the
/// conversion needs every other connection gone, and SQLite reports that as busy without calling
/// the busy handler, so it is retried until [`BUSY_TIMEOUT`].
fn set_journal_mode(conn: &Connection) -> Result<()> {
    let deadline = Instant::now() + BUSY_TIMEOUT;
    let sql = format!("PRAGMA journal_mode = {JOURNAL_MODE}");
    loop {
        let result = conn.query_row(&sql, [], |row| row.get::<_, String>(0));
        let busy = match &result {
            Ok(mode) => !mode.eq_ignore_ascii_case(JOURNAL_MODE),
            Err(rusqlite::Error::SqliteFailure(f, _)) => matches!(
                f.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ),
            Err(_) => false,
        };
        match result {
            Ok(mode) if mode.eq_ignore_ascii_case(JOURNAL_MODE) => return Ok(()),
            _ if busy && Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(mode) => {
                bail!("SQLite kept journal mode {mode}; another process may hold the ledger open")
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn has_schema(conn: &Connection) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT count(*) FROM main.sqlite_master WHERE type = 'table' AND name = 'store_meta'",
        [],
        |row| row.get(0),
    )?;
    Ok(n > 0)
}

/// Give a new ledger happenstance's schema and a fresh store identity without letting
/// happenstance open it (see the module documentation).
fn ensure_schema(conn: &Connection) -> Result<()> {
    if has_schema(conn)? {
        return Ok(());
    }
    let tmp = tempfile::tempdir().context("creating a temporary directory")?;
    let template = tmp.path().join("template.sqlite3");
    drop(SqliteEventStore::open(&template).map_err(|e| anyhow!("building the template: {e}"))?);
    conn.execute(
        "ATTACH DATABASE ?1 AS tpl",
        [template.to_string_lossy().as_ref()],
    )?;
    let copied = copy_schema(conn);
    let detached = conn.execute_batch("DETACH DATABASE tpl");
    match (copied, detached) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), Ok(())) => Err(e),
        (Err(e), Err(d)) => Err(e.context(format!("detaching the template also failed: {d}"))),
        (Ok(()), Err(d)) => Err(anyhow::Error::from(d).context("detaching the template database")),
    }
}

fn copy_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let copied = (|| {
        // Another process may have created the schema while the template was built.
        if has_schema(conn)? {
            return Ok(());
        }
        let mut stmt = conn.prepare(
            "SELECT type, name, sql FROM tpl.sqlite_master
             WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
             ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, rowid",
        )?;
        let objects = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        // Unqualified DDL creates its object in `main`.
        for (_, _, sql) in &objects {
            conn.execute_batch(sql)?;
        }
        for (_, name, _) in objects.iter().filter(|(ty, _, _)| ty == "table") {
            let name = name.replace('"', "\"\"");
            conn.execute(
                &format!("INSERT INTO main.\"{name}\" SELECT * FROM tpl.\"{name}\""),
                [],
            )?;
        }
        set_user_version(conn, LEDGER_SCHEMA)
    })();
    finish_transaction(conn, copied, "committing the ledger schema")
}

/// End the transaction a `BEGIN IMMEDIATE` opened: commit when `body` succeeded, and roll back
/// when it failed or when the commit itself failed, since a failed `COMMIT` (a deferred
/// constraint, a busy lock) leaves the transaction open on the connection.
///
/// # Errors
///
/// The failure of `body`, or of the commit with `committing` as its context; a rollback that
/// also fails is added to that error as context.
fn finish_transaction(conn: &Connection, body: Result<()>, committing: &str) -> Result<()> {
    let failure = match body {
        Ok(()) => match conn.execute_batch("COMMIT") {
            Ok(()) => return Ok(()),
            Err(e) => anyhow::Error::from(e).context(committing.to_owned()),
        },
        Err(e) => e,
    };
    // SQLite ends the transaction itself on some errors (SQLITE_FULL, IOERR, NOMEM); a ROLLBACK
    // then would only add a misleading "no transaction is active".
    if conn.is_autocommit() {
        return Err(failure);
    }
    match conn.execute_batch("ROLLBACK") {
        Ok(()) => Err(failure),
        Err(rollback) => Err(failure.context(format!("rolling back also failed: {rollback}"))),
    }
}

fn user_version(conn: &Connection) -> Result<i64> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn set_user_version(conn: &Connection, version: i64) -> Result<()> {
    conn.execute_batch(&format!("PRAGMA user_version = {version}"))?;
    Ok(())
}

/// Bring the ledger to [`LEDGER_SCHEMA`]. A ledger already there is not written to; one from a
/// newer build is refused untouched.
fn migrate(conn: &Connection) -> Result<()> {
    let check = |version: i64| {
        if version > LEDGER_SCHEMA {
            bail!(
                "the ledger has schema {version}, newer than this weighin reads ({LEDGER_SCHEMA}); upgrade weighin"
            );
        }
        Ok(version == LEDGER_SCHEMA)
    };
    if check(user_version(conn)?)? {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let migrated = (|| {
        // Another process may have migrated it since the check above.
        let mut version = user_version(conn)?;
        if check(version)? {
            return Ok(());
        }
        while version < LEDGER_SCHEMA {
            match version {
                // 0.1.x and 0.2.x: happenstance 0.3.2's schema, unstamped. Nothing to change.
                0 => {}
                v => bail!("no migration from ledger schema {v}"),
            }
            version += 1;
        }
        set_user_version(conn, LEDGER_SCHEMA)
    })();
    finish_transaction(
        conn,
        migrated.context("migrating the ledger schema"),
        "committing the ledger schema migration",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use weighin_core::event::{Channel, DismissReason, Dismissed, LedgerEvent, Raised};
    use weighin_core::marker::{Authority, Door, Kind};

    fn env(event: LedgerEvent) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: "t".into(),
            session_id: Some("s1".into()),
            worktree: Some("wt".into()),
            branch: None,
            head: None,
            ts: "2026-09-26T00:00:00Z".into(),
        }
    }

    fn raise(id: &ItemId) -> EventEnvelope {
        env(LedgerEvent::ItemRaised(Raised {
            item_id: id.clone(),
            kind: Kind::Question,
            question: "q?".into(),
            did: None,
            flip: None,
            at: None,
            sym: None,
            blocks: None,
            door_hint: Door::OneWay,
            authority_hint: Authority::Authority,
            channel: Channel::Marker,
            quote: "q?".into(),
            source_path: None,
            source_line: None,
        }))
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn guards_make_writes_idempotent_and_detect_races() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("weighin/ledger.sqlite3")).unwrap();
        let id = ItemId::mint("s1", "q?");
        assert!(matches!(
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap(),
            Appended::At(1)
        ));
        assert_eq!(
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap(),
            Appended::Conflict,
            "second raise is a no-op"
        );

        let dismiss = env(LedgerEvent::ItemDismissed(Dismissed {
            item_id: id.clone(),
            why: DismissReason::Moot,
            note: String::new(),
            reason: "r".into(),
        }));
        // A writer that read at position 1 wins; a second writer that also read at 1 loses.
        assert!(matches!(
            store
                .append(
                    std::slice::from_ref(&dismiss),
                    &Guard::ItemUnchangedSince(id.clone(), 1)
                )
                .unwrap(),
            Appended::At(2)
        ));
        assert_eq!(
            store
                .append(&[dismiss], &Guard::ItemUnchangedSince(id.clone(), 1))
                .unwrap(),
            Appended::Conflict
        );

        let ledger = store.ledger().unwrap();
        assert_eq!(
            ledger.get(&id).unwrap().status,
            weighin_core::Status::Dismissed
        );
        assert_eq!(ledger.head, 2);
    }

    fn assert_truncate_layout(path: &Path) {
        let journal = sibling(path, "-journal");
        assert!(
            journal.exists(),
            "TRUNCATE keeps the journal instead of deleting it"
        );
        assert_eq!(std::fs::metadata(&journal).unwrap().len(), 0);
        assert!(!sibling(path, "-wal").exists(), "no WAL");
        assert!(!sibling(path, "-shm").exists(), "no shared memory");
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn ledger_runs_under_truncate_and_never_leaves_wal_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".weighin/ledger.sqlite3");
        let id = ItemId::mint("s1", "q?");
        {
            let store = Store::open(&path).unwrap();
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap();
            let mode: String = store
                .inner
                .settings()
                .map(|s| format!("{s:?}"))
                .unwrap_or_default();
            assert!(mode.contains("truncate"), "{mode}");
        }
        assert_truncate_layout(&path);
        // Reopening keeps the events and the identity.
        let store = Store::open(&path).unwrap();
        assert_eq!(store.ledger().unwrap().head, 1);
        drop(store);
        assert_truncate_layout(&path);
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn empty_leftovers_from_a_failed_creation_are_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        std::fs::write(&path, b"").unwrap();
        std::fs::write(sibling(&path, "-wal"), b"not a wal").unwrap();
        let store = Store::open(&path).unwrap();
        let id = ItemId::mint("s1", "q?");
        assert!(matches!(
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap(),
            Appended::At(1)
        ));
        drop(store);
        assert_truncate_layout(&path);
        let orphans = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().contains("-wal.orphaned-"))
            .count();
        assert_eq!(orphans, 1, "the orphan WAL is renamed aside, not deleted");
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_stale_wal_beside_a_truncate_ledger_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        let id = ItemId::mint("s1", "q?");
        {
            let store = Store::open(&path).unwrap();
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap();
        }
        std::fs::write(sibling(&path, "-wal"), b"stale frames").unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(store.ledger().unwrap().head, 1);
        drop(store);
        assert_truncate_layout(&path);
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_wal_ledger_from_an_older_build_converts_and_keeps_its_events() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        let id = ItemId::mint("s1", "q?");
        {
            // What weighin 0.1.x did: happenstance's own open, which leaves the file in WAL.
            let old = SqliteEventStore::open(&path).unwrap();
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let store = Store { inner: old, rt };
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap();
        }
        let store = Store::open(&path).unwrap();
        assert_eq!(store.ledger().unwrap().head, 1);
        assert_eq!(
            store
                .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                .unwrap(),
            Appended::Conflict
        );
        drop(store);
        assert_truncate_layout(&path);
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn concurrent_creators_share_one_schema_and_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        let handles: Vec<_> = (0..6)
            .map(|n| {
                let path = path.clone();
                std::thread::Builder::new()
                    .name(format!("creator-{n}"))
                    .spawn(move || {
                        let store = Store::open(&path).unwrap();
                        let id = ItemId::mint(&format!("s{n}"), "q?");
                        store
                            .append(&[raise(&id)], &Guard::NotYetRaised(id.clone()))
                            .unwrap()
                    })
                    .unwrap()
            })
            .collect();
        for h in handles {
            assert!(matches!(h.join().unwrap(), Appended::At(_)));
        }
        assert_eq!(Store::open(&path).unwrap().ledger().unwrap().head, 6);
    }

    /// A connection under the ledger's own journal mode, so the test does not delete the journal.
    fn raw(path: &Path) -> Connection {
        connect(path).unwrap()
    }

    fn store_id(conn: &Connection) -> Vec<u8> {
        conn.query_row("SELECT v FROM store_meta WHERE k = 'store_id'", [], |r| {
            r.get(0)
        })
        .unwrap()
    }

    /// `user_version` read from the database header (bytes 60..64), without opening a
    /// connection that would convert the file.
    fn header_user_version(path: &Path) -> i64 {
        let header = std::fs::read(path).unwrap();
        i64::from(i32::from_be_bytes(header[60..64].try_into().unwrap()))
    }

    /// SQLite's file change counter (header bytes 24..28), bumped by every write transaction.
    fn change_counter(path: &Path) -> [u8; 4] {
        std::fs::read(path).unwrap()[24..28].try_into().unwrap()
    }

    /// A connection inside a transaction holding a row that breaks a deferred foreign key, so
    /// its `COMMIT` fails and leaves the transaction open.
    fn transaction_whose_commit_fails() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (parent INTEGER REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED);
             BEGIN IMMEDIATE;
             INSERT INTO child (parent) VALUES (1);",
        )
        .unwrap();
        conn
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_failed_commit_is_rolled_back_and_reported() {
        let conn = transaction_whose_commit_fails();

        let err = finish_transaction(&conn, Ok(()), "committing the test").unwrap_err();

        assert_eq!(err.to_string(), "committing the test");
        assert!(
            format!("{err:#}").contains("FOREIGN KEY"),
            "the cause is the deferred foreign key: {err:#}"
        );
        assert!(
            conn.is_autocommit(),
            "the failed commit's transaction is rolled back"
        );
        let children: i64 = conn
            .query_row("SELECT count(*) FROM child", [], |r| r.get(0))
            .unwrap();
        assert_eq!(children, 0, "the rolled-back row is gone");
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_failed_body_is_rolled_back_and_its_error_returned() {
        let conn = transaction_whose_commit_fails();

        let err = finish_transaction(&conn, Err(anyhow!("body failed")), "committing the test")
            .unwrap_err();

        assert_eq!(err.to_string(), "body failed");
        assert!(conn.is_autocommit(), "the transaction is rolled back");
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_new_ledger_is_stamped_and_reopening_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        drop(Store::open(&path).unwrap());
        assert_eq!(user_version(&raw(&path)).unwrap(), LEDGER_SCHEMA);
        let before = change_counter(&path);
        drop(Store::open(&path).unwrap());
        assert_eq!(
            change_counter(&path),
            before,
            "a current ledger is not written on open"
        );
        assert_truncate_layout(&path);
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_wal_ledger_from_0_1_is_stamped_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        let (a, b) = (ItemId::mint("s1", "a?"), ItemId::mint("s1", "b?"));
        {
            // What weighin 0.1.x did: happenstance's own open, which leaves the file in WAL.
            let old = SqliteEventStore::open(&path).unwrap();
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let store = Store { inner: old, rt };
            for id in [&a, &b] {
                store
                    .append(&[raise(id)], &Guard::NotYetRaised(id.clone()))
                    .unwrap();
            }
            let dismiss = env(LedgerEvent::ItemDismissed(Dismissed {
                item_id: a.clone(),
                why: DismissReason::Moot,
                note: String::new(),
                reason: "r".into(),
            }));
            store
                .append(&[dismiss], &Guard::ItemUnchangedSince(a.clone(), 2))
                .unwrap();
        }
        assert_eq!(header_user_version(&path), 0, "0.1.x did not stamp");
        // A plain connection leaves a WAL file in WAL; raw() would convert it.
        let identity = store_id(&Connection::open(&path).unwrap());
        assert_eq!(
            std::fs::read(&path).unwrap()[18],
            2,
            "the fixture is in WAL"
        );

        let store = Store::open(&path).unwrap();
        let ledger = store.ledger().unwrap();
        assert_eq!(ledger.head, 3);
        assert_eq!(
            ledger.get(&a).unwrap().status,
            weighin_core::Status::Dismissed
        );
        assert!(ledger.get(&b).unwrap().status.is_open());
        drop(store);
        assert_eq!(user_version(&raw(&path)).unwrap(), LEDGER_SCHEMA);
        assert_eq!(
            store_id(&raw(&path)),
            identity,
            "the store keeps its identity"
        );
        assert_truncate_layout(&path);
    }

    /// Three events exactly as weighin 0.2.2 wrote them (payloads and tags copied from a ledger
    /// that release made), so a change to the envelope's wire format fails here.
    /// An event as stored: its type, its payload and its tags.
    type StoredEvent = (
        &'static str,
        &'static str,
        &'static [(&'static str, &'static str)],
    );

    const LEDGER_0_2_2: [StoredEvent; 3] = [
        (
            "item_raised",
            r#"{"type":"item_raised","item_id":"wi-bee566","kind":"question","question":"Keep the retry at three attempts?","did":null,"flip":null,"at":null,"sym":null,"blocks":null,"door_hint":"two-way","authority_hint":"judgment","channel":"sweep","quote":"Keep the retry at three attempts?","source_path":null,"source_line":null,"actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:02Z"}"#,
            &[
                ("channel", "sweep"),
                ("door", "two-way"),
                ("item", "wi-bee566"),
                ("kind", "question"),
                ("session", "s1"),
            ],
        ),
        (
            "item_raised",
            r#"{"type":"item_raised","item_id":"wi-930e15","kind":"question","question":"Which region hosts the backup?","did":null,"flip":null,"at":"src/backup.rs:12","sym":null,"blocks":null,"door_hint":"one-way","authority_hint":"authority","channel":"marker","quote":"Which region hosts the backup?","source_path":null,"source_line":null,"actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:02Z"}"#,
            &[
                ("channel", "marker"),
                ("door", "one-way"),
                ("item", "wi-930e15"),
                ("kind", "question"),
                ("session", "s1"),
            ],
        ),
        (
            "item_dismissed",
            r#"{"type":"item_dismissed","item_id":"wi-bee566","why":{"kind":"recoverable"},"note":"","reason":"the config already says three","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:02Z"}"#,
            &[("item", "wi-bee566"), ("session", "s1")],
        ),
    ];

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_truncate_ledger_from_0_2_2_is_stamped_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        {
            // What weighin 0.2.2 did: its own TRUNCATE connection and happenstance's schema,
            // with no schema version.
            let conn = connect(&path).unwrap();
            ensure_schema(&conn).unwrap();
            set_user_version(&conn, 0).unwrap();
            let inner = SqliteEventStore::new(conn).unwrap();
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            for (ty, data, tags) in LEDGER_0_2_2 {
                let ev = Event::new(ty, data.as_bytes().to_vec())
                    .unwrap()
                    .with_tags(Tags::from_pairs(tags.iter().copied()).unwrap());
                rt.block_on(inner.append(&[ev], None)).unwrap();
            }
        }
        assert_eq!(header_user_version(&path), 0);
        let identity = store_id(&raw(&path));

        let store = Store::open(&path).unwrap();
        let ledger = store.ledger().unwrap();
        assert_eq!(ledger.head, 3);
        let retry = ItemId::parse("wi-bee566").unwrap();
        let backup = ItemId::parse("wi-930e15").unwrap();
        assert_eq!(
            ledger.get(&retry).unwrap().status,
            weighin_core::Status::Dismissed
        );
        let open = ledger.get(&backup).unwrap();
        assert!(open.status.is_open());
        assert_eq!(open.door(), Door::OneWay);
        // The conditions still see the old events: raising the same item again is a no-op.
        assert_eq!(
            store
                .append(&[raise(&backup)], &Guard::NotYetRaised(backup.clone()))
                .unwrap(),
            Appended::Conflict
        );
        drop(store);
        assert_eq!(user_version(&raw(&path)).unwrap(), LEDGER_SCHEMA);
        assert_eq!(store_id(&raw(&path)), identity);
        assert_truncate_layout(&path);
    }

    #[test]
    #[cfg_attr(miri, ignore = "opens SQLite through FFI")]
    fn a_ledger_from_a_newer_build_is_refused_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ledger.sqlite3");
        drop(Store::open(&path).unwrap());
        set_user_version(&raw(&path), LEDGER_SCHEMA + 1).unwrap();
        let before = std::fs::read(&path).unwrap();
        let err = Store::open(&path).err().expect("a newer ledger is refused");
        assert!(format!("{err:#}").contains("upgrade weighin"), "{err:#}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the ledger is left as it is"
        );
        assert_eq!(user_version(&raw(&path)).unwrap(), LEDGER_SCHEMA + 1);
        assert_truncate_layout(&path);
    }
}
