//! Current state as a fold of events. The store's position is the only ordering key; a caller
//! folds events in position order and nothing here trusts timestamps for order.

use crate::card::Card;
use crate::event::{
    Blast, Channel, Decided, DefaultCause, Defaulted, Deferred, DismissReason, EventEnvelope,
    Grounded, LedgerEvent, Linked, Presented, Provenance, Raised, Route, Timing, Triaged,
};
use crate::id::{ItemId, question_key};
use crate::marker::{Authority, Door, Kind};
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BTreeMap;

/// Where an item is in its lifecycle.
///
/// The order is the lifecycle's: a later triage or grounding never moves an open item backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Captured and not yet triaged, or reopened before it had a card.
    Raised,
    /// Classified and routed, with no card yet.
    Triaged,
    /// Has a decision card that has not been shown, or was reopened after it had one.
    Grounded,
    /// Shown to the human and waiting for an answer.
    Presented,
    /// Waiting on a named fact; open, but parked behind the deferral.
    Deferred,
    /// The human chose an option.
    Decided,
    /// An option was taken without a human.
    Defaulted,
    /// Needed no human.
    Dismissed,
    /// Something else answers it.
    Superseded,
}

impl Status {
    /// The wire spelling, such as `raised` or `defaulted`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raised => "raised",
            Self::Triaged => "triaged",
            Self::Grounded => "grounded",
            Self::Presented => "presented",
            Self::Deferred => "deferred",
            Self::Decided => "decided",
            Self::Defaulted => "defaulted",
            Self::Dismissed => "dismissed",
            Self::Superseded => "superseded",
        }
    }
    /// Still waiting on someone (a deferred item waits on its trigger).
    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Self::Raised | Self::Triaged | Self::Grounded | Self::Presented | Self::Deferred
        )
    }
}

/// The human's decision on an item, from its latest `item_decided` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Decision {
    /// The chosen card option id, or the answer itself for an item with no card.
    pub option: String,
    /// Who decided, as `human:<owner>`.
    pub decider: String,
    /// Whether the choice followed the recommendation or was the human's own.
    pub provenance: Provenance,
    /// The human's own words with the choice; empty when none.
    pub note: String,
    /// Where it was decided: `chat`, `board`, `pane` or `cli`.
    pub via: String,
    /// Whether the choice was the card's recommendation; `None` when the item had no card.
    pub matched_recommendation: Option<bool>,
    /// The tree when it was decided; `None` outside git or when git could not compute it.
    pub tree_hash: Option<String>,
    /// Seconds from the latest presentation to the decision; `None` when unknown.
    pub seconds_to_decide: Option<u64>,
    /// When the decision was written, by the writer's clock (RFC 3339 UTC).
    pub ts: String,
    /// When the decision was last reviewed and left standing (`item_acknowledged`), by the
    /// writer's clock; `None` until it is. Absent from the serialised item until then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_ts: Option<String>,
}

/// An option taken without a human, from the item's latest `item_defaulted` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DefaultTaken {
    /// The option taken, or what the agent did for its own two-way call.
    pub option: String,
    /// Why no human chose.
    pub cause: DefaultCause,
    /// The explanation shown in the digest.
    pub reason: String,
    /// When the default was written, by the writer's clock (RFC 3339 UTC).
    pub ts: String,
    /// The human saw it in the digest and left it standing (`item_acknowledged`).
    pub acknowledged: bool,
}

/// A `.kb` atom written for an item, from a `record_written` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    /// Repo-relative path of the atom file.
    pub path: String,
    /// The atom's id, as in its frontmatter.
    pub atom_id: String,
    /// `decision` or `open_question`.
    pub atom_kind: String,
}

/// One item's current state: its first raise plus every later event, folded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Item {
    /// The item's id.
    pub id: ItemId,
    /// What was raised: a question, an assumption, a deferral or a call.
    pub kind: Kind,
    /// The question as first raised.
    pub question: String,
    /// The normalised question, the key duplicates are matched on.
    pub question_key: String,
    /// What the agent did meanwhile; `None` when the raise said nothing.
    pub did: Option<String>,
    /// The flip fact the agent named when raising; `None` when it named none.
    pub flip_hint: Option<String>,
    /// The raise's repo-relative `path:line` anchor; `None` when the item points at no code.
    pub at: Option<String>,
    /// The symbol the raise named; `None` when it named none.
    pub sym: Option<String>,
    /// What the item blocks, as the raise gave it; `None` when it blocks nothing.
    pub blocks: Option<String>,
    /// The agent's reversibility guess at raise time.
    pub door_hint: Door,
    /// The agent's authority-or-judgment guess at raise time.
    pub authority_hint: Authority,
    /// How the item was captured.
    pub channel: Channel,
    /// The raised text exactly as the agent or file had it.
    pub quote: String,
    /// The repo-relative file the item was scanned from; `None` unless it came from a scan.
    pub source_path: Option<String>,
    /// The 1-based line of the quote in `source_path`; `None` when unknown.
    pub source_line: Option<usize>,
    /// The session that first raised it; `None` when that writer had no session.
    pub session_id: Option<String>,
    /// The worktree the first raise was written from; `None` outside git.
    pub worktree: Option<String>,
    /// The branch the first raise was written on; `None` outside git or on a detached head.
    pub branch: Option<String>,
    /// When the first raise was written, by the writer's clock (RFC 3339 UTC).
    pub raised_ts: String,
    /// Store position of the first raise; the age part of the rank key.
    pub raised_position: u64,

    /// Reversibility from the latest triage; `None` until triaged (see [`Item::door`]).
    pub door: Option<Door>,
    /// Blast radius from the latest triage; `None` until triaged.
    pub blast: Option<Blast>,
    /// Authority from the latest triage; `None` until triaged (see [`Item::authority`]).
    pub authority: Option<Authority>,
    /// Route from the latest triage; `None` until triaged.
    pub route: Option<Route>,
    /// The latest triage's decide-now test; `None` until triaged, or when it recorded none.
    pub timing: Option<Timing>,
    /// The latest triage's deadline; `None` until triaged, or when it set none.
    pub resolve_by: Option<String>,
    /// The card from the latest grounding; `None` until grounded.
    pub card: Option<Card>,
    /// Position of the grounding that produced `card`; answers must name it.
    ///
    /// `None` until grounded.
    pub card_rev: Option<u64>,
    /// When the item was last presented; `None` if it never was.
    pub presented_ts: Option<String>,
    /// The tree at the latest presentation; `None` if never presented or no tree was recorded.
    pub presented_tree: Option<String>,
    /// How many times the item was presented.
    pub times_presented: u32,

    /// Where the item is in its lifecycle.
    pub status: Status,
    /// The latest decision; `None` if never decided, or reopened since.
    pub decision: Option<Decision>,
    /// The latest deferral; `None` if never deferred, or cleared since by a decision or reopen.
    pub deferral: Option<Deferred>,
    /// The latest default; `None` if never defaulted, or reopened since.
    pub default: Option<DefaultTaken>,
    /// Why it was dismissed; `None` if never dismissed, or reopened since.
    pub dismissed: Option<DismissReason>,
    /// What superseded it, as the caller gave it; `None` if never superseded.
    pub superseded_by: Option<String>,
    /// Atoms written for the item, oldest first.
    pub records: Vec<Record>,
    /// `(kind, target)` links other than `raised-again`, oldest first.
    pub links: Vec<(String, String)>,
    /// Other sessions that raised the same question, each once, in the order they did.
    pub also_raised_in: Vec<String>,
    /// How many times the item was reopened.
    pub reopened: u32,
    /// Store position of the latest event applied to the item.
    pub last_position: u64,
    /// When the latest event applied to the item was written, by the writer's clock.
    pub last_ts: String,
    /// How many times a standing decision on the item was reviewed and left standing (D-0022).
    /// Not serialised, so an item prints as it did before reviews existed; `stats` counts it.
    #[serde(skip)]
    pub reviews: u32,
    /// How many reopens came after the decision they reopened had been reviewed. Not serialised,
    /// like [`Item::reviews`].
    #[serde(skip)]
    pub reopened_after_review: u32,
}

impl Item {
    /// Door as triaged, else as the agent hinted.
    #[must_use]
    pub fn door(&self) -> Door {
        self.door.unwrap_or(self.door_hint)
    }

    /// Authority as triaged, else as the agent hinted.
    #[must_use]
    pub fn authority(&self) -> Authority {
        self.authority.unwrap_or(self.authority_hint)
    }

    /// Open and not parked behind a deferral.
    #[must_use]
    pub fn needs_attention(&self) -> bool {
        self.status.is_open() && self.status != Status::Deferred
    }

    /// When the standing decision was last looked at: its newest review, else when it was made.
    /// `None` when the item has no decision.
    #[must_use]
    pub fn last_reviewed(&self) -> Option<&str> {
        self.decision
            .as_ref()
            .map(|d| d.reviewed_ts.as_deref().unwrap_or(&d.ts))
    }

    /// What would change the answer: the card recommendation's flip, else the flip fact the
    /// raise named. `None` when neither says anything.
    #[must_use]
    pub fn flip_condition(&self) -> Option<&str> {
        let from_card = self.card.as_ref().map(|c| c.recommendation.flip.trim());
        let from_raise = self.flip_hint.as_deref().map(str::trim);
        from_card
            .into_iter()
            .chain(from_raise)
            .find(|f| !f.is_empty())
    }

    /// How many whole days the standing decision has gone unreviewed, when it is due a look at
    /// `now` (D-0022): decided, a two-way door, with a flip condition, and last reviewed at least
    /// `after_days` calendar days (UTC) before `now`. `after_days` 0 turns ageing off.
    ///
    /// `now` is an RFC 3339 UTC stamp or a `YYYY-MM-DD` date from the caller's clock; a `now` or
    /// a stored stamp that does not start with a date makes nothing due.
    #[must_use]
    pub fn due_for_review(&self, now: &str, after_days: u32) -> Option<u64> {
        if after_days == 0
            || self.status != Status::Decided
            || self.door() != Door::TwoWay
            || self.flip_condition().is_none()
        {
            return None;
        }
        let age = day_number(now)?.checked_sub(day_number(self.last_reviewed()?)?)?;
        let age = u64::try_from(age).ok()?;
        (age >= u64::from(after_days)).then_some(age)
    }

    /// Rank key: one-way doors first, then items that block others, then authority-bound, then
    /// oldest first ("worst first" within a tier, then age).
    #[must_use]
    pub fn rank_key(&self) -> (u8, u8, u8, u64) {
        let door = match self.door() {
            Door::OneWay => 0,
            Door::Unsure => 1,
            Door::TwoWay => 2,
        };
        let blocks = u8::from(self.blocks.is_none());
        let auth = u8::from(self.authority() == Authority::Judgment);
        (door, blocks, auth, self.raised_position)
    }

    /// A new item from its first `item_raised` event, written at `position`.
    fn from_raised(position: u64, env: &EventEnvelope, r: &Raised) -> Self {
        Self {
            id: r.item_id.clone(),
            kind: r.kind,
            question: r.question.clone(),
            question_key: question_key(&r.question),
            did: r.did.clone(),
            flip_hint: r.flip.clone(),
            at: r.at.clone(),
            sym: r.sym.clone(),
            blocks: r.blocks.clone(),
            door_hint: r.door_hint,
            authority_hint: r.authority_hint,
            channel: r.channel,
            quote: r.quote.clone(),
            source_path: r.source_path.clone(),
            source_line: r.source_line,
            session_id: env.session_id.clone(),
            worktree: env.worktree.clone(),
            branch: env.branch.clone(),
            raised_ts: env.ts.clone(),
            raised_position: position,
            door: None,
            blast: None,
            authority: None,
            route: None,
            timing: None,
            resolve_by: None,
            card: None,
            card_rev: None,
            presented_ts: None,
            presented_tree: None,
            times_presented: 0,
            status: Status::Raised,
            decision: None,
            deferral: None,
            default: None,
            dismissed: None,
            superseded_by: None,
            records: Vec::new(),
            links: Vec::new(),
            also_raised_in: Vec::new(),
            reopened: 0,
            last_position: position,
            last_ts: env.ts.clone(),
            reviews: 0,
            reopened_after_review: 0,
        }
    }

    /// Fold one later event, written at `position` with timestamp `ts`, into this item.
    fn change(&mut self, position: u64, ts: &str, event: &LedgerEvent) {
        match event {
            // `Ledger::apply` turns a raise into a new item and never passes one here.
            LedgerEvent::ItemRaised(_) => {}
            LedgerEvent::ItemTriaged(t) => self.triaged(t),
            LedgerEvent::ItemGrounded(g) => self.grounded(position, g),
            LedgerEvent::ItemPresented(p) => self.presented(ts, p),
            LedgerEvent::ItemDecided(d) => self.decided(ts, d),
            LedgerEvent::ItemDeferred(d) => {
                self.status = Status::Deferred;
                self.deferral = Some(d.clone());
            }
            LedgerEvent::ItemDefaulted(d) => self.defaulted(ts, d),
            LedgerEvent::ItemDismissed(d) => {
                self.status = Status::Dismissed;
                self.dismissed = Some(d.why.clone());
            }
            LedgerEvent::ItemReopened(_) => self.reopen(),
            LedgerEvent::ItemSuperseded(s) => {
                self.status = Status::Superseded;
                self.superseded_by = Some(s.by.clone());
            }
            LedgerEvent::ItemLinked(l) => self.linked(l),
            LedgerEvent::RecordWritten(r) => {
                self.records.push(Record {
                    path: r.path.clone(),
                    atom_id: r.atom_id.clone(),
                    atom_kind: r.atom_kind.clone(),
                });
            }
            LedgerEvent::ItemAcknowledged(_) => self.acknowledged(ts),
        }
    }

    /// A default seen and left standing, or a standing decision reviewed and left standing
    /// (D-0022). A default is marked as a 1.1.x build marked it.
    fn acknowledged(&mut self, ts: &str) {
        if let Some(d) = &mut self.default {
            d.acknowledged = true;
        }
        if self.status == Status::Decided
            && let Some(d) = &mut self.decision
        {
            d.reviewed_ts = Some(ts.to_owned());
            self.reviews = self.reviews.saturating_add(1);
        }
    }

    fn triaged(&mut self, t: &Triaged) {
        self.door = Some(t.door);
        self.blast = Some(t.blast);
        self.authority = Some(t.authority);
        self.route = Some(t.route);
        self.timing.clone_from(&t.timing);
        self.resolve_by.clone_from(&t.resolve_by);
        if self.status.is_open() && self.status < Status::Triaged {
            self.status = Status::Triaged;
        }
    }

    fn grounded(&mut self, position: u64, g: &Grounded) {
        self.card = Some(g.card.clone());
        self.card_rev = Some(position);
        if self.status.is_open() && self.status < Status::Grounded {
            self.status = Status::Grounded;
        }
    }

    fn presented(&mut self, ts: &str, p: &Presented) {
        self.presented_ts = Some(ts.to_owned());
        self.presented_tree.clone_from(&p.tree_hash);
        self.times_presented += 1;
        if self.status.is_open() && self.status != Status::Deferred {
            self.status = Status::Presented;
        }
    }

    fn decided(&mut self, ts: &str, d: &Decided) {
        self.status = Status::Decided;
        self.deferral = None;
        self.decision = Some(Decision {
            option: d.option.clone(),
            decider: d.decider.clone(),
            provenance: d.provenance,
            note: d.note.clone(),
            via: d.via.clone(),
            matched_recommendation: d.matched_recommendation,
            tree_hash: d.tree_hash.clone(),
            seconds_to_decide: d.seconds_to_decide,
            ts: ts.to_owned(),
            reviewed_ts: None,
        });
    }

    fn defaulted(&mut self, ts: &str, d: &Defaulted) {
        self.status = Status::Defaulted;
        self.default = Some(DefaultTaken {
            option: d.option.clone(),
            cause: d.cause,
            reason: d.reason.clone(),
            ts: ts.to_owned(),
            acknowledged: false,
        });
    }

    fn reopen(&mut self) {
        self.reopened += 1;
        if self
            .decision
            .as_ref()
            .is_some_and(|d| d.reviewed_ts.is_some())
        {
            self.reopened_after_review = self.reopened_after_review.saturating_add(1);
        }
        self.decision = None;
        self.default = None;
        self.dismissed = None;
        self.deferral = None;
        self.status = if self.card.is_some() {
            Status::Grounded
        } else {
            Status::Raised
        };
    }

    fn linked(&mut self, l: &Linked) {
        if l.kind == "raised-again" {
            if !self.also_raised_in.contains(&l.target) {
                self.also_raised_in.push(l.target.clone());
            }
        } else {
            self.links.push((l.kind.clone(), l.target.clone()));
        }
    }
}

/// The folded ledger.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Ledger {
    /// Every item ever raised, by id.
    pub items: BTreeMap<ItemId, Item>,
    /// The highest store position folded so far; 0 for an empty ledger.
    pub head: u64,
    /// Events the fold could not apply (an event for an item never raised). Surfaced, not dropped.
    pub orphans: Vec<(u64, String)>,
}

impl Ledger {
    /// Apply one event at `position`. Positions must be supplied in ascending order.
    pub fn apply(&mut self, position: u64, env: &EventEnvelope) {
        self.head = self.head.max(position);
        if let LedgerEvent::ItemRaised(r) = &env.event {
            self.raise(position, env, r);
            return;
        }
        let id = env.event.item_id();
        let Some(item) = self.items.get_mut(id) else {
            self.orphans.push((
                position,
                format!("{} for unknown {id}", env.event.event_type()),
            ));
            return;
        };
        item.last_position = position;
        item.last_ts.clone_from(&env.ts);
        item.change(position, &env.ts, &env.event);
    }

    /// Fold an `item_raised` event: a new item, or a note that another session raised it again.
    fn raise(&mut self, position: u64, env: &EventEnvelope, r: &Raised) {
        if let Some(existing) = self.items.get_mut(&r.item_id) {
            // Same id again: the writer's idempotency guard should prevent this; keep the first.
            if let Some(s) = &env.session_id
                && existing.session_id.as_deref() != Some(s.as_str())
                && !existing.also_raised_in.contains(s)
            {
                existing.also_raised_in.push(s.clone());
            }
            return;
        }
        self.items
            .insert(r.item_id.clone(), Item::from_raised(position, env, r));
    }

    /// Fold a whole ordered stream.
    ///
    /// # Examples
    ///
    /// ```
    /// use weighin_core::{EventEnvelope, ItemId, Ledger, Status};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let raised = EventEnvelope::from_json(
    ///         br#"{"type": "item_raised", "item_id": "wi-930e15", "kind": "question",
    ///              "question": "Which region hosts the backup?", "door_hint": "one-way",
    ///              "authority_hint": "authority", "channel": "marker",
    ///              "quote": "Which region hosts the backup?", "actor": "cli",
    ///              "ts": "2026-10-02T21:40:02Z"}"#,
    ///     )?;
    ///     let dismissed = EventEnvelope::from_json(
    ///         br#"{"type": "item_dismissed", "item_id": "wi-930e15", "why": {"kind": "moot"},
    ///              "reason": "the backup moved to the provider", "actor": "cli",
    ///              "ts": "2026-10-02T21:45:00Z"}"#,
    ///     )?;
    ///
    ///     let open = Ledger::fold([(1, &raised)]);
    ///     assert_eq!(open.open().len(), 1);
    ///
    ///     let ledger = Ledger::fold([(1, &raised), (2, &dismissed)]);
    ///     let id = ItemId::parse("wi-930e15").ok_or("not an item id")?;
    ///     let item = ledger.get(&id).ok_or("the raise was not folded")?;
    ///     assert_eq!(item.status, Status::Dismissed);
    ///     assert!(ledger.open().is_empty());
    ///     assert_eq!(ledger.head, 2);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn fold<'a>(events: impl IntoIterator<Item = (u64, &'a EventEnvelope)>) -> Self {
        let mut l = Self::default();
        for (p, e) in events {
            l.apply(p, e);
        }
        l
    }

    /// Open items, ranked.
    #[must_use]
    pub fn open(&self) -> Vec<&Item> {
        let mut v: Vec<&Item> = self.items.values().filter(|i| i.status.is_open()).collect();
        v.sort_by_key(|i| i.rank_key());
        v
    }

    /// Open items raised in `session` (or raised again there).
    #[must_use]
    pub fn open_in_session(&self, session: &str) -> Vec<&Item> {
        self.open()
            .into_iter()
            .filter(|i| {
                i.session_id.as_deref() == Some(session)
                    || i.also_raised_in.iter().any(|s| s == session)
            })
            .collect()
    }

    /// An open item already asking this question, in any session.
    #[must_use]
    pub fn open_with_question(&self, question: &str) -> Option<&Item> {
        let k = question_key(question);
        self.items
            .values()
            .find(|i| i.status.is_open() && i.question_key == k)
    }

    /// Any item, open or closed, asking this question (a settled question is never re-raised).
    #[must_use]
    pub fn any_with_question(&self, question: &str) -> Option<&Item> {
        let k = question_key(question);
        self.items
            .values()
            .filter(|i| i.question_key == k)
            .max_by_key(|i| i.last_position)
    }

    /// Defaults nobody has looked at yet (the digest).
    #[must_use]
    pub fn unacknowledged_defaults(&self) -> Vec<&Item> {
        let mut v: Vec<&Item> = self
            .items
            .values()
            .filter(|i| {
                i.status == Status::Defaulted && i.default.as_ref().is_some_and(|d| !d.acknowledged)
            })
            .collect();
        v.sort_by_key(|i| i.rank_key());
        v
    }

    /// Standing decisions due a look at `now`, each with its days since the last look, oldest
    /// first, then in rank order (see [`Item::due_for_review`]).
    #[must_use]
    pub fn due_for_review(&self, now: &str, after_days: u32) -> Vec<(&Item, u64)> {
        let mut v: Vec<(&Item, u64)> = self
            .items
            .values()
            .filter_map(|i| Some((i, i.due_for_review(now, after_days)?)))
            .collect();
        v.sort_by_key(|(i, age)| (Reverse(*age), i.rank_key()));
        v
    }

    /// The item with exactly this id, open or closed.
    #[must_use]
    pub fn get(&self, id: &ItemId) -> Option<&Item> {
        self.items.get(id)
    }

    /// Resolve a possibly abbreviated id (`3fa9`, `wi-3fa9`) to exactly one item.
    #[must_use]
    pub fn resolve(&self, needle: &str) -> Option<&Item> {
        let n = needle.trim();
        let n = if n.starts_with("wi-") {
            n.to_owned()
        } else {
            format!("wi-{n}")
        };
        if let Some(exact) = self.items.values().find(|i| i.id.as_str() == n) {
            return Some(exact);
        }
        let mut hits = self
            .items
            .values()
            .filter(|i| i.id.as_str().starts_with(&n));
        let first = hits.next()?;
        if hits.next().is_some() {
            None
        } else {
            Some(first)
        }
    }
}

/// Days since 1970-01-01 of the `YYYY-MM-DD` date that `stamp` starts with (proleptic
/// Gregorian, Howard Hinnant's `days_from_civil`); `None` when it does not start with one.
fn day_number(stamp: &str) -> Option<i64> {
    let date = stamp.get(..10)?;
    let mut parts = date.split('-');
    let mut field = |len: usize| {
        parts
            .next()
            .filter(|p| p.len() == len && p.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|p| p.parse::<i64>().ok())
    };
    let (year, month, day) = (field(4)?, field(2)?, field(2)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // A year from March, so the leap day is the last day of the year it belongs to.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let month_from_march = (month + 9).rem_euclid(12);
    let day_of_year = (153 * month_from_march + 2).div_euclid(5) + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era.div_euclid(4) - year_of_era.div_euclid(100) + day_of_year;
    Some(era * 146_097 + day_of_era - 719_468)
}

/// Health numbers over presented items, as the board shows them.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Health {
    /// Items presented at least once.
    pub presented: usize,
    /// Presented items decided, deferred with a passing decide-now test, or dismissed as
    /// someone else's call.
    pub acted: usize,
    /// Presented items dismissed as not useful.
    pub not_useful: usize,
    /// Presented items decided against the card's recommendation.
    pub chosen_against_recommendation: usize,
    /// Presented items decided while the card had a recommendation to compare with.
    pub chosen_with_recommendation_known: usize,
    /// The lower median of the decision times that were recorded; `None` when none were.
    pub median_seconds_to_decide: Option<u64>,
    /// Presented items deferred without a passing decide-now test.
    pub deferred_unqualified: usize,
    /// Never-presented items defaulted and not yet acknowledged.
    pub defaults_unacknowledged: usize,
}

impl Health {
    /// Count the health numbers over every item in `ledger`.
    #[must_use]
    pub fn of(ledger: &Ledger) -> Self {
        let mut h = Self::default();
        let mut secs = Vec::new();
        for i in ledger.items.values() {
            if i.times_presented == 0 {
                if i.status == Status::Defaulted
                    && i.default.as_ref().is_some_and(|d| !d.acknowledged)
                {
                    h.defaults_unacknowledged += 1;
                }
                continue;
            }
            h.presented += 1;
            match i.status {
                Status::Decided => {
                    h.acted += 1;
                    if let Some(d) = &i.decision {
                        if let Some(m) = d.matched_recommendation {
                            h.chosen_with_recommendation_known += 1;
                            if !m {
                                h.chosen_against_recommendation += 1;
                            }
                        }
                        if let Some(s) = d.seconds_to_decide {
                            secs.push(s);
                        }
                    }
                }
                Status::Deferred => {
                    if i.deferral.as_ref().is_some_and(|d| !d.unqualified) {
                        h.acted += 1;
                    } else {
                        h.deferred_unqualified += 1;
                    }
                }
                Status::Dismissed => match &i.dismissed {
                    Some(DismissReason::NotUseful) => h.not_useful += 1,
                    Some(DismissReason::NotMyCall(_)) => h.acted += 1,
                    _ => {}
                },
                _ => {}
            }
        }
        secs.sort_unstable();
        h.median_seconds_to_decide = secs
            .get(secs.len().saturating_sub(1) / 2)
            .copied()
            .filter(|_| !secs.is_empty());
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::*;

    fn env(event: LedgerEvent, session: &str) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: "test".into(),
            session_id: Some(session.into()),
            worktree: None,
            branch: None,
            head: None,
            ts: "2026-09-26T00:00:00Z".into(),
        }
    }

    fn raised(id: &ItemId, q: &str, door: Door) -> LedgerEvent {
        LedgerEvent::ItemRaised(Raised {
            item_id: id.clone(),
            kind: Kind::Question,
            question: q.into(),
            did: None,
            flip: None,
            at: None,
            sym: None,
            blocks: None,
            door_hint: door,
            authority_hint: Authority::Authority,
            channel: Channel::Marker,
            quote: q.into(),
            source_path: None,
            source_line: None,
        })
    }

    #[test]
    fn lifecycle_and_ranking() {
        let a = ItemId::mint("s1", "two-way thing?");
        let b = ItemId::mint("s1", "one-way thing?");
        let evs = [
            env(raised(&a, "two-way thing?", Door::TwoWay), "s1"),
            env(raised(&b, "one-way thing?", Door::OneWay), "s1"),
            env(
                LedgerEvent::ItemPresented(Presented {
                    item_id: b.clone(),
                    surface: "chat".into(),
                    tree_hash: Some("t1".into()),
                    reason: "sitting".into(),
                }),
                "s1",
            ),
            env(
                LedgerEvent::ItemDecided(Decided {
                    item_id: b.clone(),
                    option: "A".into(),
                    decider: "human:ryan".into(),
                    provenance: Provenance::UserApproved,
                    note: String::new(),
                    tree_hash: Some("t1".into()),
                    matched_recommendation: Some(true),
                    via: "chat".into(),
                    seconds_to_decide: Some(40),
                    reason: "picked the recommendation".into(),
                }),
                "s1",
            ),
        ];
        let l = Ledger::fold((1u64..).zip(evs.iter()));
        assert_eq!(l.get(&b).unwrap().status, Status::Decided);
        let open = l.open();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, a);
        let h = Health::of(&l);
        assert_eq!(h.presented, 1);
        assert_eq!(h.acted, 1);
        assert_eq!(h.median_seconds_to_decide, Some(40));
    }

    #[test]
    fn one_way_ranks_first_and_reopen_restores_grounded() {
        let a = ItemId::mint("s", "a?");
        let b = ItemId::mint("s", "b?");
        let mut l = Ledger::default();
        l.apply(1, &env(raised(&a, "a?", Door::TwoWay), "s"));
        l.apply(2, &env(raised(&b, "b?", Door::OneWay), "s"));
        assert_eq!(l.open()[0].id, b);
        l.apply(
            3,
            &env(
                LedgerEvent::ItemDefaulted(Defaulted {
                    item_id: a.clone(),
                    option: "A".into(),
                    cause: DefaultCause::TwoWay,
                    reason: "agent's call".into(),
                }),
                "s",
            ),
        );
        assert_eq!(l.unacknowledged_defaults().len(), 1);
        l.apply(
            4,
            &env(
                LedgerEvent::ItemReopened(Reopened {
                    item_id: a.clone(),
                    note: "wrong call".into(),
                    reason: "reopened from digest".into(),
                }),
                "s",
            ),
        );
        assert_eq!(l.get(&a).unwrap().status, Status::Raised);
        assert!(l.unacknowledged_defaults().is_empty());
    }

    #[test]
    fn orphans_are_kept_and_duplicates_found() {
        let a = ItemId::mint("s", "Use Postgres?");
        let mut l = Ledger::default();
        l.apply(
            1,
            &env(
                LedgerEvent::ItemAcknowledged(Acknowledged {
                    item_id: a.clone(),
                    reason: "x".into(),
                }),
                "s",
            ),
        );
        assert_eq!(l.orphans.len(), 1);
        l.apply(2, &env(raised(&a, "Use Postgres?", Door::Unsure), "s"));
        assert!(l.open_with_question("use postgres").is_some());
        assert_eq!(l.resolve(a.as_str().get(3..7).unwrap()).unwrap().id, a);
    }

    fn card() -> Card {
        Card {
            options: vec![
                crate::card::CardOption {
                    id: "A".into(),
                    title: "a".into(),
                    ..Default::default()
                },
                crate::card::CardOption {
                    id: "B".into(),
                    title: "b".into(),
                    ..Default::default()
                },
            ],
            recommendation: crate::card::Recommendation {
                option: "A".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn triage(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemTriaged(Triaged {
            item_id: id.clone(),
            door: Door::OneWay,
            blast: Blast::System,
            authority: Authority::Authority,
            route: Route::Present,
            timing: Some(Timing {
                verdict: "defer".into(),
                flip: "f".into(),
                obtainable_now: false,
                arrives_after_expiry: false,
                cost_of_waiting: "none".into(),
                options_close: false,
            }),
            resolve_by: Some("before-release".into()),
            reason: "r".into(),
        })
    }

    fn ground(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemGrounded(Grounded {
            item_id: id.clone(),
            card: card(),
            reason: "r".into(),
        })
    }

    fn present(id: &ItemId, tree: &str) -> LedgerEvent {
        LedgerEvent::ItemPresented(Presented {
            item_id: id.clone(),
            surface: "board".into(),
            tree_hash: Some(tree.into()),
            reason: "r".into(),
        })
    }

    fn defer(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemDeferred(Deferred {
            item_id: id.clone(),
            flip: "f".into(),
            trigger: "next sitting".into(),
            resolve_by: "2026-10-09".into(),
            default_option: "A".into(),
            owner: "ryan".into(),
            unqualified: false,
            reason: "r".into(),
        })
    }

    fn decide(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemDecided(Decided {
            item_id: id.clone(),
            option: "B".into(),
            decider: "human:ryan".into(),
            provenance: Provenance::UserDirected,
            note: "n".into(),
            tree_hash: Some("t2".into()),
            matched_recommendation: Some(false),
            via: "board".into(),
            seconds_to_decide: Some(7),
            reason: "r".into(),
        })
    }

    fn record(id: &ItemId) -> LedgerEvent {
        LedgerEvent::RecordWritten(RecordWritten {
            item_id: id.clone(),
            path: ".kb/_intake/x.md".into(),
            atom_id: "kb-x".into(),
            atom_kind: "decision".into(),
            reason: "r".into(),
        })
    }

    fn link(id: &ItemId, kind: &str, target: &str) -> LedgerEvent {
        LedgerEvent::ItemLinked(Linked {
            item_id: id.clone(),
            kind: kind.into(),
            target: target.into(),
            reason: "r".into(),
        })
    }

    fn default_taken(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemDefaulted(Defaulted {
            item_id: id.clone(),
            option: "kept it".into(),
            cause: DefaultCause::TwoWay,
            reason: "r".into(),
        })
    }

    fn acknowledge(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemAcknowledged(Acknowledged {
            item_id: id.clone(),
            reason: "r".into(),
        })
    }

    fn dismiss(id: &ItemId, why: DismissReason) -> LedgerEvent {
        LedgerEvent::ItemDismissed(Dismissed {
            item_id: id.clone(),
            why,
            note: String::new(),
            reason: "r".into(),
        })
    }

    fn reopen(id: &ItemId) -> LedgerEvent {
        LedgerEvent::ItemReopened(Reopened {
            item_id: id.clone(),
            note: String::new(),
            reason: "r".into(),
        })
    }

    fn supersede(id: &ItemId, by: &str) -> LedgerEvent {
        LedgerEvent::ItemSuperseded(Superseded {
            item_id: id.clone(),
            by: by.into(),
            reason: "r".into(),
        })
    }

    /// Raised twice, triaged, grounded, presented, deferred, presented again, decided, recorded
    /// and linked (a raised-again link twice).
    fn decided_item_events(a: &ItemId) -> Vec<EventEnvelope> {
        vec![
            env(raised(a, "a?", Door::Unsure), "s1"),
            env(raised(a, "a again?", Door::OneWay), "s2"),
            env(triage(a), "s1"),
            env(ground(a), "s1"),
            env(present(a, "t1"), "s1"),
            env(defer(a), "s1"),
            env(present(a, "t2"), "s1"),
            env(decide(a), "s1"),
            env(record(a), "s1"),
            env(link(a, "commit", "abc123"), "s1"),
            env(link(a, "raised-again", "s3"), "s1"),
            env(link(a, "raised-again", "s3"), "s1"),
        ]
    }

    /// Raised, defaulted, acknowledged, reopened without a card, dismissed as not useful.
    fn defaulted_item_events(b: &ItemId) -> Vec<EventEnvelope> {
        vec![
            env(raised(b, "b?", Door::TwoWay), "s1"),
            env(default_taken(b), "s1"),
            env(acknowledge(b), "s1"),
            env(reopen(b), "s1"),
            env(dismiss(b, DismissReason::NotUseful), "s1"),
        ]
    }

    /// Raised, grounded, dismissed, reopened with a card, superseded, then triaged and presented
    /// while closed.
    fn superseded_item_events(c: &ItemId) -> Vec<EventEnvelope> {
        vec![
            env(raised(c, "c?", Door::OneWay), "s1"),
            env(ground(c), "s1"),
            env(dismiss(c, DismissReason::NotMyCall("legal".into())), "s1"),
            env(reopen(c), "s1"),
            env(supersede(c, "wi-aaaaaa"), "s1"),
            env(triage(c), "s1"),
            env(present(c, "t3"), "s1"),
        ]
    }

    fn at(event: LedgerEvent, ts: &str) -> EventEnvelope {
        EventEnvelope {
            ts: ts.into(),
            ..env(event, "s1")
        }
    }

    /// A card whose recommendation names `flip` as what would change the answer.
    fn ground_with_flip(id: &ItemId, flip: &str) -> LedgerEvent {
        let mut c = card();
        flip.clone_into(&mut c.recommendation.flip);
        LedgerEvent::ItemGrounded(Grounded {
            item_id: id.clone(),
            card: c,
            reason: "r".into(),
        })
    }

    /// `q` raised behind `door`, grounded on a card with `flip`, and decided at `decided_ts`.
    fn decided_at(id: &ItemId, door: Door, flip: &str, decided_ts: &str) -> Vec<EventEnvelope> {
        vec![
            at(raised(id, "q?", door), "2026-01-01T09:00:00Z"),
            at(ground_with_flip(id, flip), "2026-01-01T09:00:00Z"),
            at(decide(id), decided_ts),
        ]
    }

    fn fold_all(evs: &[EventEnvelope]) -> Ledger {
        Ledger::fold((1u64..).zip(evs))
    }

    fn due_ids(l: &Ledger, now: &str, after_days: u32) -> Vec<(ItemId, u64)> {
        l.due_for_review(now, after_days)
            .into_iter()
            .map(|(i, age)| (i.id.clone(), age))
            .collect()
    }

    #[test]
    fn a_two_way_decision_with_a_flip_comes_due_after_the_review_age() {
        let a = ItemId::mint("s1", "q?");
        let l = fold_all(&decided_at(
            &a,
            Door::TwoWay,
            "a tenant asks",
            "2026-02-01T10:00:00Z",
        ));

        assert_eq!(due_ids(&l, "2026-05-02T00:00:00Z", 90), [(a.clone(), 90)]);
        assert_eq!(due_ids(&l, "2026-05-01T23:59:59Z", 90), []);
        assert_eq!(
            l.get(&a).and_then(Item::flip_condition),
            Some("a tenant asks")
        );
    }

    #[test]
    fn one_way_and_unflippable_decisions_never_come_due() {
        let one_way = ItemId::mint("s1", "one?");
        let no_flip = ItemId::mint("s1", "none?");
        let mut evs = decided_at(
            &one_way,
            Door::OneWay,
            "a tenant asks",
            "2025-01-01T00:00:00Z",
        );
        evs.extend(decided_at(
            &no_flip,
            Door::TwoWay,
            "  ",
            "2025-01-01T00:00:00Z",
        ));
        let l = fold_all(&evs);

        assert_eq!(due_ids(&l, "2026-11-01", 90), []);
    }

    #[test]
    fn a_review_age_of_zero_turns_ageing_off() {
        let a = ItemId::mint("s1", "q?");
        let l = fold_all(&decided_at(&a, Door::TwoWay, "f", "2020-01-01T00:00:00Z"));

        assert_eq!(due_ids(&l, "2026-11-01", 0), []);
        assert_eq!(due_ids(&l, "2026-11-01", 1).len(), 1);
    }

    #[test]
    fn an_unreadable_now_or_stamp_makes_nothing_due() {
        let a = ItemId::mint("s1", "q?");
        let l = fold_all(&decided_at(&a, Door::TwoWay, "f", "yesterday"));
        let ok = fold_all(&decided_at(&a, Door::TwoWay, "f", "2020-01-01T00:00:00Z"));

        assert_eq!(due_ids(&l, "2026-11-01", 1), []);
        assert_eq!(due_ids(&ok, "", 1), []);
        assert_eq!(due_ids(&ok, "2026-13-01", 1), []);
    }

    #[test]
    fn a_review_restarts_the_clock_and_is_counted() {
        let a = ItemId::mint("s1", "q?");
        let mut evs = decided_at(&a, Door::TwoWay, "f", "2026-01-01T00:00:00Z");
        evs.push(at(acknowledge(&a), "2026-10-01T00:00:00Z"));
        let l = fold_all(&evs);
        let item = l.get(&a).unwrap();

        assert_eq!(item.last_reviewed(), Some("2026-10-01T00:00:00Z"));
        assert_eq!(item.status, Status::Decided);
        assert_eq!(item.reviews, 1);
        assert_eq!(due_ids(&l, "2026-11-01", 90), []);
        assert_eq!(due_ids(&l, "2026-12-30", 90), [(a, 90)]);
    }

    #[test]
    fn a_reopen_removes_a_due_decision_and_counts_when_it_follows_a_review() {
        let a = ItemId::mint("s1", "q?");
        let mut evs = decided_at(&a, Door::TwoWay, "f", "2026-01-01T00:00:00Z");
        evs.push(at(acknowledge(&a), "2026-04-01T00:00:00Z"));
        evs.push(at(reopen(&a), "2026-08-01T00:00:00Z"));
        let l = fold_all(&evs);
        let item = l.get(&a).unwrap();

        assert_eq!(due_ids(&l, "2026-11-01", 90), []);
        assert_eq!(item.last_reviewed(), None);
        assert_eq!((item.reviews, item.reopened_after_review), (1, 1));
    }

    #[test]
    fn a_reopen_without_a_review_is_not_counted_as_following_one() {
        let a = ItemId::mint("s1", "q?");
        let mut evs = decided_at(&a, Door::TwoWay, "f", "2026-01-01T00:00:00Z");
        evs.push(at(reopen(&a), "2026-08-01T00:00:00Z"));
        evs.push(at(decide(&a), "2026-08-02T00:00:00Z"));
        evs.push(at(reopen(&a), "2026-08-03T00:00:00Z"));
        let item = fold_all(&evs).get(&a).cloned().unwrap();

        assert_eq!((item.reviews, item.reopened_after_review), (0, 0));
    }

    #[test]
    fn a_new_decision_starts_its_own_clock() {
        let a = ItemId::mint("s1", "q?");
        let mut evs = decided_at(&a, Door::TwoWay, "f", "2026-01-01T00:00:00Z");
        evs.push(at(acknowledge(&a), "2026-03-01T00:00:00Z"));
        evs.push(at(decide(&a), "2026-06-01T00:00:00Z"));
        let item = fold_all(&evs).get(&a).cloned().unwrap();

        assert_eq!(item.last_reviewed(), Some("2026-06-01T00:00:00Z"));
    }

    #[test]
    fn an_acknowledged_default_folds_as_a_1_1_build_folded_it() {
        let b = ItemId::parse("wi-bbbbbb").unwrap();
        let evs = [
            env(raised(&b, "b?", Door::TwoWay), "s1"),
            env(default_taken(&b), "s1"),
            env(acknowledge(&b), "s1"),
        ];
        let l = fold_all(&evs);
        let item = l.get(&b).unwrap();

        assert!(item.default.as_ref().is_some_and(|d| d.acknowledged));
        assert_eq!(item.status, Status::Defaulted);
        assert_eq!(item.reviews, 0);
        assert!(l.unacknowledged_defaults().is_empty());
        assert_eq!(due_ids(&l, "2030-01-01", 1), []);
        let json = serde_json::to_value(item).unwrap();
        assert!(
            json.get("reviews").is_none() && json.get("reopened_after_review").is_none(),
            "{json}"
        );
    }

    /// Events as weighin 1.1.x stored them: a default acknowledged after the digest, and a
    /// decision. A review is the same `item_acknowledged` payload (D-0022), so it is listed too.
    const LEDGER_1_1: [&str; 6] = [
        r#"{"type":"item_raised","item_id":"wi-bee566","kind":"call","question":"Keep the retry at three attempts?","did":"kept three","flip":null,"at":null,"sym":null,"blocks":null,"door_hint":"two-way","authority_hint":"judgment","channel":"marker","quote":"Keep the retry at three attempts?","source_path":null,"source_line":null,"actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:02Z"}"#,
        r#"{"type":"item_defaulted","item_id":"wi-bee566","option":"kept three","cause":"two-way","reason":"the agent's own two-way call","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:03Z"}"#,
        r#"{"type":"item_acknowledged","item_id":"wi-bee566","reason":"left as defaulted after the digest","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-03T08:00:00Z"}"#,
        r#"{"type":"item_raised","item_id":"wi-930e15","kind":"question","question":"Name the helper apply or run?","did":null,"flip":"a second caller","at":null,"sym":null,"blocks":null,"door_hint":"two-way","authority_hint":"judgment","channel":"marker","quote":"Name the helper apply or run?","source_path":null,"source_line":null,"actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:40:02Z"}"#,
        r#"{"type":"item_decided","item_id":"wi-930e15","option":"apply","decider":"human:ryan","provenance":"user-directed","note":"","tree_hash":null,"matched_recommendation":null,"via":"chat","seconds_to_decide":null,"reason":"answered","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2026-10-02T21:41:00Z"}"#,
        r#"{"type":"item_acknowledged","item_id":"wi-930e15","reason":"left standing after the digest","actor":"cli","session_id":"s1","worktree":null,"branch":null,"head":null,"ts":"2027-01-10T08:00:00Z"}"#,
    ];

    /// The current fold reads 1.1.x events as before, and a review uses the 1.1.x payload. This
    /// cannot run a 1.1.x fold: that it ignores a review rests on its `item_acknowledged` arm,
    /// which sets `acknowledged` only on an item with a default (D-0022).
    #[test]
    fn a_1_1_ledger_folds_its_acknowledged_default_as_before_and_reads_a_review() {
        let evs: Vec<EventEnvelope> = LEDGER_1_1
            .iter()
            .map(|p| EventEnvelope::from_json(p.as_bytes()).unwrap())
            .collect();
        let l = fold_all(&evs);
        let default = l.get(&ItemId::parse("wi-bee566").unwrap()).unwrap();
        let decision = l.get(&ItemId::parse("wi-930e15").unwrap()).unwrap();

        assert_eq!(
            serde_json::to_value(&default.default).unwrap(),
            serde_json::json!({
                "option": "kept three", "cause": "two-way",
                "reason": "the agent's own two-way call", "ts": "2026-10-02T21:40:03Z",
                "acknowledged": true
            })
        );
        assert_eq!((default.status, default.reviews), (Status::Defaulted, 0));
        assert_eq!(default.last_ts, "2026-10-03T08:00:00Z");
        assert!(l.unacknowledged_defaults().is_empty());
        assert_eq!(decision.last_reviewed(), Some("2027-01-10T08:00:00Z"));
        assert_eq!(decision.flip_condition(), Some("a second caller"));
        let written = env(acknowledge(&decision.id), "s1").to_json().unwrap();
        let keys: Vec<String> = serde_json::from_slice::<serde_json::Map<_, _>>(&written)
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys,
            [
                "actor",
                "branch",
                "head",
                "item_id",
                "reason",
                "session_id",
                "ts",
                "type",
                "worktree"
            ],
            "a review is written in the 1.1.x acknowledgement's shape"
        );
    }

    #[test]
    fn a_reviewed_decision_serialises_its_review_time() {
        let a = ItemId::mint("s1", "q?");
        let mut evs = decided_at(&a, Door::TwoWay, "f", "2026-01-01T00:00:00Z");
        let unreviewed = serde_json::to_value(fold_all(&evs).get(&a).unwrap()).unwrap();
        evs.push(at(acknowledge(&a), "2026-04-01T00:00:00Z"));
        let reviewed = serde_json::to_value(fold_all(&evs).get(&a).unwrap()).unwrap();

        assert!(
            unreviewed["decision"].get("reviewed_ts").is_none(),
            "{unreviewed}"
        );
        assert_eq!(
            reviewed["decision"]["reviewed_ts"],
            serde_json::json!("2026-04-01T00:00:00Z")
        );
    }

    #[test]
    fn day_numbers_count_calendar_days_across_leap_years() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(day_number("2000-03-01T00:00:00Z"), Some(11_017));
        assert_eq!(
            day_number("2024-03-01"),
            day_number("2024-02-28").map(|d| d + 2)
        );
        assert_eq!(
            day_number("2023-03-01"),
            day_number("2023-02-28").map(|d| d + 1)
        );
        assert_eq!(day_number("2026-11-01"), Some(20_758));
        for bad in [
            "",
            "2026-1-01",
            "2026-00-10",
            "2026-02-32",
            "20x6-01-01",
            "2026/01/01",
        ] {
            assert_eq!(day_number(bad), None, "{bad:?}");
        }
    }

    /// Every event kind through the fold, pinned as the serialised ledger it produces.
    #[test]
    fn every_event_kind_folds_to_the_pinned_ledger() {
        let a = ItemId::parse("wi-aaaaaa").unwrap();
        let b = ItemId::parse("wi-bbbbbb").unwrap();
        let c = ItemId::parse("wi-cccccc").unwrap();
        let ghost = ItemId::parse("wi-dddddd").unwrap();
        let mut evs = decided_item_events(&a);
        evs.extend(defaulted_item_events(&b));
        evs.extend(superseded_item_events(&c));
        evs.push(env(triage(&ghost), "s1"));
        let l = Ledger::fold((1u64..).zip(evs.iter()));
        let got = serde_json::to_string_pretty(&l).unwrap();
        assert_eq!(got, PINNED_LEDGER, "the fold's output changed");
    }

    const PINNED_LEDGER: &str = r#"{
  "items": {
    "wi-aaaaaa": {
      "id": "wi-aaaaaa",
      "kind": "question",
      "question": "a?",
      "question_key": "ca978112ca1bbdca",
      "did": null,
      "flip_hint": null,
      "at": null,
      "sym": null,
      "blocks": null,
      "door_hint": "unsure",
      "authority_hint": "authority",
      "channel": "marker",
      "quote": "a?",
      "source_path": null,
      "source_line": null,
      "session_id": "s1",
      "worktree": null,
      "branch": null,
      "raised_ts": "2026-09-26T00:00:00Z",
      "raised_position": 1,
      "door": "one-way",
      "blast": "system",
      "authority": "authority",
      "route": "present",
      "timing": {
        "verdict": "defer",
        "flip": "f",
        "obtainable_now": false,
        "arrives_after_expiry": false,
        "cost_of_waiting": "none",
        "options_close": false
      },
      "resolve_by": "before-release",
      "card": {
        "why": "",
        "timing": "",
        "evidence": [],
        "options": [
          {
            "id": "A",
            "title": "a",
            "summary": "",
            "holds_if": "",
            "if_wrong": ""
          },
          {
            "id": "B",
            "title": "b",
            "summary": "",
            "holds_if": "",
            "if_wrong": ""
          }
        ],
        "criteria": [],
        "recommendation": {
          "option": "A",
          "flip": "",
          "tradeoff": "",
          "why_wrong": ""
        }
      },
      "card_rev": 4,
      "presented_ts": "2026-09-26T00:00:00Z",
      "presented_tree": "t2",
      "times_presented": 2,
      "status": "decided",
      "decision": {
        "option": "B",
        "decider": "human:ryan",
        "provenance": "user-directed",
        "note": "n",
        "via": "board",
        "matched_recommendation": false,
        "tree_hash": "t2",
        "seconds_to_decide": 7,
        "ts": "2026-09-26T00:00:00Z"
      },
      "deferral": null,
      "default": null,
      "dismissed": null,
      "superseded_by": null,
      "records": [
        {
          "path": ".kb/_intake/x.md",
          "atom_id": "kb-x",
          "atom_kind": "decision"
        }
      ],
      "links": [
        [
          "commit",
          "abc123"
        ]
      ],
      "also_raised_in": [
        "s2",
        "s3"
      ],
      "reopened": 0,
      "last_position": 12,
      "last_ts": "2026-09-26T00:00:00Z"
    },
    "wi-bbbbbb": {
      "id": "wi-bbbbbb",
      "kind": "question",
      "question": "b?",
      "question_key": "3e23e8160039594a",
      "did": null,
      "flip_hint": null,
      "at": null,
      "sym": null,
      "blocks": null,
      "door_hint": "two-way",
      "authority_hint": "authority",
      "channel": "marker",
      "quote": "b?",
      "source_path": null,
      "source_line": null,
      "session_id": "s1",
      "worktree": null,
      "branch": null,
      "raised_ts": "2026-09-26T00:00:00Z",
      "raised_position": 13,
      "door": null,
      "blast": null,
      "authority": null,
      "route": null,
      "timing": null,
      "resolve_by": null,
      "card": null,
      "card_rev": null,
      "presented_ts": null,
      "presented_tree": null,
      "times_presented": 0,
      "status": "dismissed",
      "decision": null,
      "deferral": null,
      "default": null,
      "dismissed": {
        "kind": "not-useful"
      },
      "superseded_by": null,
      "records": [],
      "links": [],
      "also_raised_in": [],
      "reopened": 1,
      "last_position": 17,
      "last_ts": "2026-09-26T00:00:00Z"
    },
    "wi-cccccc": {
      "id": "wi-cccccc",
      "kind": "question",
      "question": "c?",
      "question_key": "2e7d2c03a9507ae2",
      "did": null,
      "flip_hint": null,
      "at": null,
      "sym": null,
      "blocks": null,
      "door_hint": "one-way",
      "authority_hint": "authority",
      "channel": "marker",
      "quote": "c?",
      "source_path": null,
      "source_line": null,
      "session_id": "s1",
      "worktree": null,
      "branch": null,
      "raised_ts": "2026-09-26T00:00:00Z",
      "raised_position": 18,
      "door": "one-way",
      "blast": "system",
      "authority": "authority",
      "route": "present",
      "timing": {
        "verdict": "defer",
        "flip": "f",
        "obtainable_now": false,
        "arrives_after_expiry": false,
        "cost_of_waiting": "none",
        "options_close": false
      },
      "resolve_by": "before-release",
      "card": {
        "why": "",
        "timing": "",
        "evidence": [],
        "options": [
          {
            "id": "A",
            "title": "a",
            "summary": "",
            "holds_if": "",
            "if_wrong": ""
          },
          {
            "id": "B",
            "title": "b",
            "summary": "",
            "holds_if": "",
            "if_wrong": ""
          }
        ],
        "criteria": [],
        "recommendation": {
          "option": "A",
          "flip": "",
          "tradeoff": "",
          "why_wrong": ""
        }
      },
      "card_rev": 19,
      "presented_ts": "2026-09-26T00:00:00Z",
      "presented_tree": "t3",
      "times_presented": 1,
      "status": "superseded",
      "decision": null,
      "deferral": null,
      "default": null,
      "dismissed": null,
      "superseded_by": "wi-aaaaaa",
      "records": [],
      "links": [],
      "also_raised_in": [],
      "reopened": 1,
      "last_position": 24,
      "last_ts": "2026-09-26T00:00:00Z"
    }
  },
  "head": 25,
  "orphans": [
    [
      25,
      "item_triaged for unknown wi-dddddd"
    ]
  ]
}"#;
}

#[cfg(test)]
mod collision_tests {
    use super::*;
    use crate::event::*;

    #[test]
    fn an_exact_id_wins_over_its_extended_sibling() {
        let short = ItemId::parse("wi-dc4d29").unwrap();
        let long = ItemId::parse("wi-dc4d29f0").unwrap();
        let mut l = Ledger::default();
        for (p, id, q) in [(1, &short, "first?"), (2, &long, "second?")] {
            l.apply(
                p,
                &EventEnvelope {
                    event: LedgerEvent::ItemRaised(Raised {
                        item_id: id.clone(),
                        kind: Kind::Question,
                        question: q.into(),
                        did: None,
                        flip: None,
                        at: None,
                        sym: None,
                        blocks: None,
                        door_hint: Door::Unsure,
                        authority_hint: Authority::Authority,
                        channel: Channel::Marker,
                        quote: q.into(),
                        source_path: None,
                        source_line: None,
                    }),
                    actor: "t".into(),
                    session_id: None,
                    worktree: None,
                    branch: None,
                    head: None,
                    ts: "2026-09-27T00:00:00Z".into(),
                },
            );
        }
        assert_eq!(l.resolve("wi-dc4d29").unwrap().id, short);
        assert_eq!(l.resolve("dc4d29f").unwrap().id, long);
    }
}
