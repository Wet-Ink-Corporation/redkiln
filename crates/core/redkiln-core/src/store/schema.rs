//! A store's schema as its stamp gives it (SPEC ST-06): `major * 1000 + minor`.

use std::fmt;

/// A store's schema, `<major>.<minor>`, as the stamp in its file gives it (ST-06): what
/// `status --json` reports as `store_schema` (CLI-12). Built only from a stamp in range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StoreSchema {
    major: u16,
    minor: u16,
}

/// A stamp that is no store schema: below 1000 (major 0, an old plugin ledger's) or above
/// 65,535,999 (a major past `u16`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the stamp {0} is not a store schema (1000 to 65535999)")]
pub struct StampOutOfRange(pub i64);

impl StoreSchema {
    /// The major version: a store of another major is refused (ST-07).
    #[must_use]
    pub const fn major(self) -> u16 {
        self.major
    }

    /// The minor version, 0 to 999.
    #[must_use]
    pub const fn minor(self) -> u16 {
        self.minor
    }

    /// The stamp, `major * 1000 + minor`.
    #[must_use]
    pub fn stamp(self) -> i64 {
        i64::from(self.major) * 1000 + i64::from(self.minor)
    }
}

impl TryFrom<i64> for StoreSchema {
    type Error = StampOutOfRange;

    /// # Errors
    /// [`StampOutOfRange`] for a stamp below 1000 or above 65,535,999.
    fn try_from(stamp: i64) -> Result<Self, Self::Error> {
        // A minor is 0 to 999 by `rem_euclid`; a major is 1 to `u16::MAX`.
        match (
            u16::try_from(stamp.div_euclid(1000)),
            u16::try_from(stamp.rem_euclid(1000)),
        ) {
            (Ok(major), Ok(minor)) if major != 0 => Ok(Self { major, minor }),
            _ => Err(StampOutOfRange(stamp)),
        }
    }
}

impl fmt::Display for StoreSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[cfg(test)]
mod tests {
    use super::{StampOutOfRange, StoreSchema};

    #[test]
    fn a_stamp_is_major_times_1000_plus_minor() {
        let schema = StoreSchema::try_from(1003).expect("a store schema");
        assert_eq!((schema.major(), schema.minor()), (1, 3));
        assert_eq!(schema.to_string(), "1.3");
        assert_eq!(schema.stamp(), 1003);
        assert_eq!(
            StoreSchema::try_from(1000).map(|s| s.to_string()),
            Ok("1.0".to_owned())
        );
    }

    #[test]
    fn stamps_outside_the_range_are_refused() {
        assert_eq!(StoreSchema::try_from(999), Err(StampOutOfRange(999)));
        assert_eq!(StoreSchema::try_from(0), Err(StampOutOfRange(0)));
        assert_eq!(StoreSchema::try_from(-1), Err(StampOutOfRange(-1)));
        let top = StoreSchema::try_from(65_535_999).expect("the largest stamp");
        assert_eq!(top.to_string(), "65535.999");
        assert_eq!(
            StoreSchema::try_from(65_536_000),
            Err(StampOutOfRange(65_536_000))
        );
    }
}
