//! Finding a NaN or an infinity in a payload before it is written (EV-05).
//!
//! JSON has no NaN or infinity, and `serde_json` writes either as `null` without an error, so
//! an event holding one would be stored and then read back unusable. Its formatter cannot tell
//! that `null` from a `None`'s, so the payload is walked once by [`Finder`], a serializer that
//! writes nothing and looks only at floats.

use std::fmt;

use serde::Serialize;
use serde::ser;

/// What the walk found, as a serde error so that it stops the walk.
#[derive(Debug)]
pub(super) enum Found {
    /// A NaN or an infinite float.
    NonFinite,
    /// The payload's own `Serialize` failed.
    Custom(String),
}

impl fmt::Display for Found {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => f.write_str("a NaN or infinite float"),
            Self::Custom(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Found {}

impl ser::Error for Found {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

/// `Ok` when every float in `value` is finite.
pub(super) fn check<T: Serialize + ?Sized>(value: &T) -> Result<(), Found> {
    value.serialize(Finder)
}

/// A serializer that writes nothing and refuses a non-finite float.
#[derive(Debug, Clone, Copy)]
struct Finder;

fn finite(finite: bool) -> Result<(), Found> {
    if finite {
        Ok(())
    } else {
        Err(Found::NonFinite)
    }
}

impl ser::Serializer for Finder {
    type Ok = ();
    type Error = Found;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_f32(self, v: f32) -> Result<(), Found> {
        finite(v.is_finite())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Found> {
        finite(v.is_finite())
    }

    fn serialize_bool(self, _: bool) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_i8(self, _: i8) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_i16(self, _: i16) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_i32(self, _: i32) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_i64(self, _: i64) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_i128(self, _: i128) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_u8(self, _: u8) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_u16(self, _: u16) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_u32(self, _: u32) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_u64(self, _: u64) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_u128(self, _: u128) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_char(self, _: char) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_str(self, _: &str) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_bytes(self, _: &[u8]) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), Found> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(self)
    }

    fn serialize_seq(self, _: Option<usize>) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_tuple(self, _: usize) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_map(self, _: Option<usize>) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Found> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Found> {
        Ok(self)
    }
}

impl ser::SerializeSeq for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTuple for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTupleVariant for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeMap for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Found> {
        key.serialize(*self)
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeStruct for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeStructVariant for Finder {
    type Ok = ();
    type Error = Found;

    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(*self)
    }

    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn floats_are_found_in_every_shape() {
        assert!(matches!(check(&1.5_f64), Ok(())));
        assert!(matches!(check(&f64::NAN), Err(Found::NonFinite)));
        assert!(matches!(
            check(&[1.0, f64::INFINITY]),
            Err(Found::NonFinite)
        ));
        assert!(matches!(
            check(&(1, Some(f32::NEG_INFINITY))),
            Err(Found::NonFinite)
        ));
        let map: BTreeMap<&str, Vec<f64>> = [("a", vec![0.0]), ("b", vec![f64::NAN])].into();
        assert!(matches!(check(&map), Err(Found::NonFinite)));
        let whole: BTreeMap<&str, i128> = [("a", i128::MAX)].into();
        assert!(
            matches!(check(&whole), Ok(())),
            "128-bit integers are not refused"
        );
    }
}
