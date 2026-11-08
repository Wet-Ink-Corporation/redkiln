//! The spike's event types, built through Core's envelope as a plugin would build its own.

use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context as _;
use redkiln_core::envelope::{
    Actor, Author, Class, Envelope, Namespace, Payload, Timestamp, Writer,
};
use redkiln_core::store::{Condition, NewEvent, Tag, TelemetryEvent};
use redkiln_core::trust::Trust;
use serde::{Deserialize, Serialize};

/// The tag key naming the writer.
pub const WRITER_KEY: &str = "loop.spike_writer";
/// The tag key naming the writer's sequence number.
pub const SEQ_KEY: &str = "loop.spike_seq";
/// The tag key naming a claimed subject.
pub const CLAIM_KEY: &str = "loop.spike_claim";
/// The type of a claim, which two racers condition on.
const CLAIMED: &str = "loop.spike_claimed";

/// A durable append by one writer.
#[derive(Debug, Serialize, Deserialize)]
struct Appended {
    writer: String,
    seq: u64,
}

impl Payload for Appended {
    const TYPE: &'static str = "loop.spike_appended";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// A first-writer-wins claim of one subject.
#[derive(Debug, Serialize, Deserialize)]
struct Claimed {
    writer: String,
    claim: u64,
}

impl Payload for Claimed {
    const TYPE: &'static str = CLAIMED;
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// A hook's timing, as temper records one on every tool call.
#[derive(Debug, Serialize, Deserialize)]
struct Timed {
    micros: u64,
}

impl Payload for Timed {
    const TYPE: &'static str = "loop.spike_timed";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Telemetry;
}

fn author(actor: Actor) -> anyhow::Result<Author> {
    Ok(Author {
        actor,
        trust: Trust::asserted(),
        writer: Writer::new("r-sqlite", "0.0.0").context("writer")?,
    })
}

/// The timestamp is advisory (EV-15); the spike writes its start date.
fn ts() -> anyhow::Result<Timestamp> {
    Timestamp::parse("2026-11-08T00:00:00Z").context("timestamp")
}

fn tag(key: &str, value: &str) -> anyhow::Result<Tag> {
    let name = key.strip_prefix("loop.").context("a loop. key")?;
    Tag::namespaced(Namespace::Loop, name, value).with_context(|| format!("tag {key}"))
}

/// Writer `writer`'s append number `seq`, tagged with both.
pub fn appended(actor: Actor, writer: &str, seq: u64) -> anyhow::Result<NewEvent> {
    let payload = Appended {
        writer: writer.to_owned(),
        seq,
    };
    Envelope::new(ts()?, author(actor)?, payload)
        .to_event([tag(WRITER_KEY, writer)?, tag(SEQ_KEY, &seq.to_string())?])
        .context("building an append")
}

/// A claim of subject `claim`, and the condition that makes the first claim win.
pub fn claimed(writer: &str, claim: u64) -> anyhow::Result<(NewEvent, Condition)> {
    let subject = tag(CLAIM_KEY, &claim.to_string())?;
    let payload = Claimed {
        writer: writer.to_owned(),
        claim,
    };
    let event = Envelope::new(ts()?, author(Actor::Cli)?, payload)
        .to_event([subject.clone(), tag(WRITER_KEY, writer)?])
        .context("building a claim")?;
    let condition = Condition::not_yet(CLAIMED, subject).context("claim condition")?;
    Ok((event, condition))
}

/// A hook's timing event.
pub fn timed(micros: u64) -> anyhow::Result<TelemetryEvent> {
    Envelope::new(ts()?, author(Actor::Hook)?, Timed { micros })
        .to_telemetry([])
        .context("building telemetry")
}

/// Milliseconds since the Unix epoch: the clock processes share for barriers and deadlines.
pub fn unix_millis() -> anyhow::Result<u64> {
    let since = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("clock before 1970")?;
    u64::try_from(since.as_millis()).context("clock past u64 milliseconds")
}
