//! The sections `bench status` reads from the anneal home: `clef`, from `decide`'s
//! configuration and its answered-decision counts (`decisions/counts.json`), and `context`,
//! the fresher of the latest snapshots `bench statusline` (`context-usage.json`) and the mod
//! (`context-usage-mod.json`) saved.

use std::ffi::OsString;
use std::path::Path;

use bench_contract::status::{self, Clef, ClefMode, Context, DecisionPoint, PointStatus};

use super::Unreadable;
use crate::context::usage::{Level, Window, WindowSource};
use crate::context::{self, Fresher, Snapshot};
use crate::decide::config::Mode;
use crate::decide::log::DecisionLog;
use crate::decide::modes::Modes;
use crate::decide::point::Point;

/// The point as the section names it.
const fn point(point: Point) -> DecisionPoint {
    match point {
        Point::D1 => DecisionPoint::D1,
        Point::D2 => DecisionPoint::D2,
        Point::D3 => DecisionPoint::D3,
        Point::D4 => DecisionPoint::D4,
        Point::D5 => DecisionPoint::D5,
        Point::D6 => DecisionPoint::D6,
        Point::D7 => DecisionPoint::D7,
    }
}

/// The mode as the section names it.
const fn mode(mode: Mode) -> ClefMode {
    match mode {
        Mode::Off => ClefMode::Off,
        Mode::Shadow => ClefMode::Shadow,
        Mode::Enforce => ClefMode::Enforce,
    }
}

/// `clef`, or `None` when Clef has no account and no decision was ever answered. Each point's
/// `mode` is the one in effect: `off` without an account, `shadow` while a configured
/// `enforce` has fewer than 50 answered decisions, else the configured one.
pub(super) fn clef(
    home: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
) -> Result<Option<Clef>, Unreadable> {
    let modes = Modes::read(&DecisionLog::in_home(home), lookup)?;
    let configured = modes.is_configured();
    if !configured && !modes.any_answered() {
        return Ok(None);
    }
    let points = Point::ALL
        .into_iter()
        .map(|each| PointStatus {
            point: point(each),
            answered: modes.answered(each),
            configured_mode: mode(modes.configured(each)),
            mode: mode(modes.effective(each)),
        })
        .collect();
    Ok(Some(Clef { configured, points }))
}

/// `context` as read: the section, and why each usage file that could not be read could not.
pub(super) struct ContextRead {
    /// The section, or `None` when no readable usage file has a reading.
    pub(super) context: Option<Context>,
    /// Why each unreadable usage file could not be read, the status line's first.
    pub(super) unreadable: Vec<Unreadable>,
}

/// `context`: the fresher of the status line's and the mod's newest snapshots
/// ([`context::Sources::newest`]); a usage file that cannot be read leaves the other's shown.
pub(super) fn context(home: &Path) -> ContextRead {
    let Fresher { saved, unreadable } = context::Sources::load(home).newest();
    ContextRead {
        context: saved.map(|saved| section(saved.reading)),
        unreadable: unreadable.into_iter().map(Unreadable::Sessions).collect(),
    }
}

/// The section for `snapshot`.
fn section(snapshot: Snapshot) -> Context {
    let window = Window {
        tokens: snapshot.window,
        source: snapshot.window_source,
    };
    let percent = snapshot
        .used_tokens
        .map(|used| (used / f64::from(snapshot.window.max(1)) * 1000.0).round() / 10.0);
    let window_source = match snapshot.window_source {
        WindowSource::Env => status::WindowSource::Env,
        WindowSource::Model => status::WindowSource::Model,
        WindowSource::Default => status::WindowSource::Default,
    };
    let level = snapshot
        .used_tokens
        .and_then(|used| window.level(used))
        .map(|level| match level {
            Level::Warn => status::Level::Warn,
            Level::Stop => status::Level::Stop,
        });
    Context {
        used_tokens: snapshot.used_tokens,
        window: snapshot.window,
        window_source,
        percent,
        level,
    }
}
