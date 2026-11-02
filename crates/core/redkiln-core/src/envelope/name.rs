//! Event type names (SPEC EV-03): a registered namespace, a dot, and a name.

use std::fmt;
use std::str::FromStr;

/// The longest event type, in bytes (EV-03).
pub const MAX_TYPE_BYTES: usize = 64;

/// A namespace of the closed registry (EV-03). Adding one is a SPEC minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    /// Redkiln Core: imports, approvals, capacity, calibration.
    Core,
    /// Weigh-In.
    Weighin,
    /// Bench.
    Bench,
    /// Redkiln Rust.
    Rust,
    /// Review.
    Review,
    /// Loop.
    Loop,
    /// Knowledge.
    Knowledge,
    /// Backlog.
    Backlog,
}

impl Namespace {
    /// Every registered namespace.
    pub const ALL: [Self; 8] = [
        Self::Core,
        Self::Weighin,
        Self::Bench,
        Self::Rust,
        Self::Review,
        Self::Loop,
        Self::Knowledge,
        Self::Backlog,
    ];

    /// The namespace as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Weighin => "weighin",
            Self::Bench => "bench",
            Self::Rust => "rust",
            Self::Review => "review",
            Self::Loop => "loop",
            Self::Knowledge => "knowledge",
            Self::Backlog => "backlog",
        }
    }
}

impl FromStr for Namespace {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|namespace| namespace.as_str() == text)
            .ok_or(NameError::Namespace)
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why a type name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    /// The part before the first dot is not a registered namespace.
    #[error("the type's namespace is not in the registry")]
    Namespace,
    /// The name does not match `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*`.
    #[error("the type's name does not match [a-z][a-z0-9_]*(.[a-z][a-z0-9_]*)*")]
    Grammar,
    /// The whole type is longer than [`MAX_TYPE_BYTES`].
    #[error("the type is {len} bytes, more than {MAX_TYPE_BYTES}")]
    TooLong {
        /// Its length in bytes.
        len: usize,
    },
}

/// An event type, `<namespace>.<name>` (EV-03): a registered namespace and a name in the
/// grammar, at most [`MAX_TYPE_BYTES`] bytes in all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeName {
    namespace: Namespace,
    text: String,
}

impl TypeName {
    /// Reads `text` as a type.
    ///
    /// # Errors
    /// [`NameError::TooLong`], [`NameError::Namespace`] or [`NameError::Grammar`], checked in
    /// that order.
    pub fn parse(text: &str) -> Result<Self, NameError> {
        if text.len() > MAX_TYPE_BYTES {
            return Err(NameError::TooLong { len: text.len() });
        }
        let (namespace, name) = text.split_once('.').ok_or(NameError::Namespace)?;
        let namespace = namespace.parse()?;
        if !is_dotted_name(name) {
            return Err(NameError::Grammar);
        }
        Ok(Self {
            namespace,
            text: text.to_owned(),
        })
    }

    /// The namespace.
    #[must_use]
    pub const fn namespace(&self) -> Namespace {
        self.namespace
    }

    /// The name after the namespace's dot.
    #[must_use]
    pub fn name(&self) -> &str {
        self.text.split_once('.').map_or("", |(_, name)| name)
    }

    /// The whole type as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The namespace of `text` when it fits EV-03's type grammar, registered or not:
/// `<namespace>.<name>` of at most [`MAX_TYPE_BYTES`], the namespace one `[a-z][a-z0-9_]*`
/// segment. What a journal reader keeps whether or not this build knows the type (JR-10): a
/// later SPEC minor may register the namespace.
pub(crate) fn namespace_of(text: &str) -> Option<&str> {
    let (namespace, name) = text.split_once('.')?;
    (text.len() <= MAX_TYPE_BYTES && is_dotted_name(namespace) && is_dotted_name(name))
        .then_some(namespace)
}

/// `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*`: EV-03's names, and the part of a namespaced tag key
/// after its namespace (EV-12).
pub(crate) fn is_dotted_name(text: &str) -> bool {
    text.split('.').all(|segment| {
        let mut bytes = segment.bytes();
        bytes.next().is_some_and(|b| b.is_ascii_lowercase())
            && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_is_a_registered_namespace_and_a_name() {
        let name = TypeName::parse("weighin.item_decided").expect("valid");
        assert_eq!(name.namespace(), Namespace::Weighin);
        assert_eq!(name.name(), "item_decided");
        assert_eq!(name.as_str(), "weighin.item_decided");
    }

    #[test]
    fn dotted_names_keep_their_shape() {
        let name = TypeName::parse("loop.migrate.wave.started").expect("anneal's shape");
        assert_eq!(name.namespace(), Namespace::Loop);
        assert_eq!(name.name(), "migrate.wave.started");
    }

    #[test]
    fn every_registered_namespace_reads_back() {
        for namespace in Namespace::ALL {
            assert_eq!(namespace.as_str().parse(), Ok(namespace));
        }
        assert_eq!("anneal".parse::<Namespace>(), Err(NameError::Namespace));
    }

    #[test]
    fn an_unregistered_namespace_is_refused() {
        assert_eq!(
            TypeName::parse("anneal.ledger_opened"),
            Err(NameError::Namespace)
        );
        assert_eq!(
            TypeName::parse("item_decided"),
            Err(NameError::Namespace),
            "no namespace"
        );
    }

    #[test]
    fn names_outside_the_grammar_are_refused() {
        for text in [
            "weighin.",
            "weighin.Item",
            "weighin.1item",
            "weighin.item-decided",
            "weighin.item..decided",
            "weighin.item.",
            "weighin._item",
            "weighin.item decided",
        ] {
            assert_eq!(TypeName::parse(text), Err(NameError::Grammar), "{text:?}");
        }
    }

    #[test]
    fn a_type_over_64_bytes_is_refused() {
        let fits = format!("core.{}", "a".repeat(MAX_TYPE_BYTES - "core.".len()));
        let name = TypeName::parse(&fits).expect("exactly 64 bytes fits");
        assert_eq!(name.as_str().len(), MAX_TYPE_BYTES);
        assert_eq!(
            TypeName::parse(&format!("{fits}a")),
            Err(NameError::TooLong {
                len: MAX_TYPE_BYTES + 1
            })
        );
    }
}
