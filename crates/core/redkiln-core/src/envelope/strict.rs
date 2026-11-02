//! JSON read with no key named twice in any object (EV-10, OUT-05).
//!
//! `serde_json` reads a repeated key last-wins, so an envelope with two `trust` objects would
//! read as whichever came last, which is not what a reader that keeps the first would see. A
//! stored envelope is read through [`Strict`] instead, which refuses the repeat, and the event
//! is unusable rather than read either way.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::map::Entry;
use serde_json::{Map, Number, Value};

/// A JSON value whose objects, at every depth, name each key once.
#[derive(Debug)]
pub(super) struct Strict(pub(super) Value);

impl<'de> Deserialize<'de> for Strict {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(Self)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value with no repeated key")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        // JSON text never holds a non-finite number, so this refuses nothing serde_json reads.
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("a non-finite number"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::String(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::String(v))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        Strict::deserialize(deserializer).map(|Strict(value)| value)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(Strict(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            match object.entry(key) {
                Entry::Occupied(seen) => {
                    return Err(de::Error::custom(format_args!(
                        "duplicate key `{}`",
                        seen.key()
                    )));
                }
                Entry::Vacant(slot) => {
                    let Strict(value) = map.next_value()?;
                    slot.insert(value);
                }
            }
        }
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> Result<Value, serde_json::Error> {
        serde_json::from_str::<Strict>(text).map(|Strict(value)| value)
    }

    #[test]
    fn it_reads_what_serde_json_reads() {
        let text = r#"{"a":[1,-2,3.5,true,null,"x",{"b":{}}],"c":18446744073709551615}"#;
        let plain: Value = serde_json::from_str(text).expect("json");
        assert_eq!(read(text).expect("no repeated key"), plain);
    }

    #[test]
    fn a_repeated_key_is_refused_at_any_depth() {
        for text in [r#"{"a":1,"a":2}"#, r#"{"a":[{"b":1,"c":2,"b":3}]}"#] {
            let error = read(text).expect_err("a repeated key");
            assert!(
                error.to_string().contains("duplicate key"),
                "{text}: {error}"
            );
        }
    }
}
