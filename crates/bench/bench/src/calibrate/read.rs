//! Reading the decision log `bench decide` writes, through `redkiln_calibrate::rows` (anneal's
//! v1 rows), with Clef's [`Point`] and [`Question`]: the old folder's files first, then the new
//! one's, each oldest first. A row naming a point or question this build does not know is a
//! skipped row.

use redkiln_calibrate::rows::{self, ReadError};
use redkiln_calibrate::score::{Probability, Sample};

use crate::baseline::BaselineError;
use crate::calibrate::CalibrateError;
use crate::decide::log::DecisionLog;
use crate::decide::point::{Point, Question};

#[cfg(test)]
pub(crate) use redkiln_calibrate::rows::{HEADROOM_ROWS, Label, MAX_LOG_FILE_BYTES, ROW_BYTES};

/// An answered decision of a Clef point.
pub(crate) type Answered = rows::Answered<Point, Question>;

/// What the log holds, as calibration needs it.
pub(crate) type LogRows = rows::LogRows<Point, Question>;

/// `decision` as a scored sample whose predicted event happened when `positive`; `None` only
/// for a probability outside 0..=1, which [`read_log`] never yields (it skips such a row).
pub(crate) fn sample(decision: &Answered, positive: bool) -> Option<Sample> {
    Probability::try_from(decision.probability)
        .ok()
        .map(|probability| Sample::of(probability, positive))
}

/// Reads every file of `log`: the old folder's, then the new one's.
///
/// # Errors
/// [`CalibrateError::Read`] when a file cannot be read or is not a regular file;
/// [`CalibrateError::TooLarge`] when one is over [`rows::MAX_LOG_FILE_BYTES`].
pub(crate) fn read_log(log: &DecisionLog) -> Result<LogRows, CalibrateError> {
    rows::read(log.logs()).map_err(|error| match error {
        ReadError::Io { path, source } => CalibrateError::Read(BaselineError::Io {
            op: "reading",
            path,
            source,
        }),
        ReadError::NotAFile { path } => CalibrateError::Read(BaselineError::NotAFile { path }),
        ReadError::TooLarge { path, cap } => CalibrateError::TooLarge { path, cap },
    })
}

#[cfg(test)]
mod tests;
